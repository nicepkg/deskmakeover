/**
 * The five /engine/ three.js scenes, as one module. Each factory matches the
 * SceneInit contract exactly: it owns a private WebGLRenderer on the supplied
 * canvas, returns a SceneHandle, and lets the wrappers own all surrounding DOM.
 *
 * v3 — cinematic upgrade:
 *   - ZERO CLIPPING, by maths not eyeballing. Each scene reports a Box3 that
 *     bounds every model, shadow, floating layer and particle across its whole
 *     timeline; the Stage fits the camera to that volume for the actual canvas
 *     aspect and rotation range (full-orbit → sphere fit; clamped → swept fit),
 *     with a ≥12% margin, so no frame of spin / breathe / explode ever touches
 *     the edge.
 *   - PHYSICAL MATERIALS + LIGHTING. Glass cards are MeshPhysicalMaterial
 *     (clearcoat, low roughness, ior) lit by a PMREM-baked RoomEnvironment plus
 *     a key + fill light; reflections roll across their bevels as you orbit.
 *     Icon pixel planes stay unlit (pixel-honest, tone-map-exempt).
 *   - PIXEL PARTICLES. The engine's real pixels ARE the particles: InstancedMesh
 *     voxels coloured per-instance from the real ImageData — shatter (cut),
 *     converge (rescue), burst (hero), scan-dust (read). No sparkles/glow.
 *   - INTERACTION. Drag-orbit + inertia + double-click replay; hover lifts the
 *     card stack a touch. reduceMotion renders the end state, no particles.
 *
 * Transparent, dual-theme canvas: env intensity is restrained and the glass is
 * opacity-blended (not transmission — transmission samples a black backdrop on a
 * transparent canvas and reads dirty in light theme), so it stays clean on both.
 */
import * as THREE from "three";
import { RoomEnvironment } from "three/examples/jsm/environments/RoomEnvironment.js";
import type {
  CutAssets,
  HeroAssets,
  LabelPoint,
  PromiseAssets,
  ReadAssets,
  RescueAssets,
  SceneCommonOpts,
  SceneHandle,
  SceneInit,
} from "./contract";

// ── palette ─────────────────────────────────────────────────────────
const COLOR_CORAL = "#ff6f5e";
const COLOR_INK = "#16181d";
const COLOR_TEAL = "#128577";

// ── easing / math ───────────────────────────────────────────────────
const clamp = (x: number, a: number, b: number): number => (x < a ? a : x > b ? b : x);
const clamp01 = (x: number): number => (x < 0 ? 0 : x > 1 ? 1 : x);
const lerp = (a: number, b: number, t: number): number => a + (b - a) * t;
const smooth = (e0: number, e1: number, x: number): number => {
  const t = clamp01((x - e0) / (e1 - e0));
  return t * t * (3 - 2 * t);
};
const easeOutExpo = (x: number): number => (x >= 1 ? 1 : 1 - Math.pow(2, -10 * x));
const easeOutCubic = (x: number): number => 1 - Math.pow(1 - x, 3);
const easeInOutCubic = (x: number): number =>
  x < 0.5 ? 4 * x * x * x : 1 - Math.pow(-2 * x + 2, 3) / 2;
/** overshoot-limited settle; s≈0.9 peaks ~3% over 1 before resting */
const easeOutBack = (x: number, s: number): number => {
  const c3 = s + 1;
  const y = x - 1;
  return 1 + c3 * y * y * y + s * y * y;
};

// ── resource tracking ───────────────────────────────────────────────
class ResourcePool {
  private items: { dispose(): void }[] = [];
  track<T extends { dispose(): void }>(x: T): T {
    this.items.push(x);
    return x;
  }
  disposeAll(): void {
    for (const i of this.items) i.dispose();
    this.items = [];
  }
}

// ── shared geometry helpers ─────────────────────────────────────────
function roundedRectShape(w: number, h: number, r: number): THREE.Shape {
  const s = new THREE.Shape();
  const x = -w / 2;
  const y = -h / 2;
  s.moveTo(x + r, y);
  s.lineTo(x + w - r, y);
  s.quadraticCurveTo(x + w, y, x + w, y + r);
  s.lineTo(x + w, y + h - r);
  s.quadraticCurveTo(x + w, y + h, x + w - r, y + h);
  s.lineTo(x + r, y + h);
  s.quadraticCurveTo(x, y + h, x, y + h - r);
  s.lineTo(x, y + r);
  s.quadraticCurveTo(x, y, x + r, y);
  return s;
}

/** Frosted glass card: a thin bevelled slab of MeshPhysicalMaterial. Env + key
 *  light give it clearcoat highlights that roll along the bevel as you orbit —
 *  same-colour layers stay legible because each card's edge catches its own
 *  glint. Facing +Z, co-planar with the artwork it backs. */
function buildGlassCard(pool: ResourcePool, w: number, h: number, r: number): THREE.Mesh {
  const shape = roundedRectShape(w, h, r);
  const geo = pool.track(
    new THREE.ExtrudeGeometry(shape, {
      depth: 0.03,
      bevelEnabled: true,
      bevelThickness: 0.009,
      bevelSize: 0.009,
      bevelSegments: 2,
      curveSegments: 14,
    }),
  );
  geo.translate(0, 0, -0.015);
  const mat = pool.track(
    new THREE.MeshPhysicalMaterial({
      color: 0xf3f5f8,
      metalness: 0,
      roughness: 0.16,
      clearcoat: 1,
      clearcoatRoughness: 0.18,
      ior: 1.45,
      reflectivity: 0.55,
      transparent: true,
      opacity: 0.38,
      envMapIntensity: 1.25,
      side: THREE.DoubleSide,
      depthWrite: false,
    }),
  );
  return new THREE.Mesh(geo, mat);
}

/** Flat unlit colour swatch (rounded). Kept unlit so the seed colour reads true
 *  — a physical material would tint it with the environment. */
interface Chip {
  group: THREE.Group;
  fillMat: THREE.MeshBasicMaterial;
}
function buildChip(pool: ResourcePool, w: number, h: number, r: number, color: THREE.ColorRepresentation): Chip {
  const shape = roundedRectShape(w, h, r);
  const fillMat = pool.track(
    new THREE.MeshBasicMaterial({
      color: new THREE.Color(color),
      transparent: true,
      opacity: 1,
      depthWrite: false,
      side: THREE.DoubleSide,
      toneMapped: false,
    }),
  );
  const fill = new THREE.Mesh(pool.track(new THREE.ShapeGeometry(shape)), fillMat);
  const group = new THREE.Group();
  group.add(fill);
  return { group, fillMat };
}

/** soft radial contact shadow, lying flat on the ground plane */
function buildShadow(pool: ResourcePool, size: number): THREE.Mesh {
  const c = document.createElement("canvas");
  c.width = c.height = 128;
  const g = c.getContext("2d");
  if (g) {
    const grad = g.createRadialGradient(64, 64, 4, 64, 64, 62);
    grad.addColorStop(0, "rgba(22,24,29,0.42)");
    grad.addColorStop(0.6, "rgba(22,24,29,0.14)");
    grad.addColorStop(1, "rgba(22,24,29,0)");
    g.fillStyle = grad;
    g.fillRect(0, 0, 128, 128);
  }
  const tex = pool.track(new THREE.CanvasTexture(c));
  tex.colorSpace = THREE.SRGBColorSpace;
  const mat = pool.track(
    new THREE.MeshBasicMaterial({ map: tex, transparent: true, depthWrite: false, toneMapped: false }),
  );
  const mesh = new THREE.Mesh(pool.track(new THREE.PlaneGeometry(size, size)), mat);
  mesh.rotation.x = -Math.PI / 2;
  mesh.scale.set(1.2, 1, 1);
  mesh.renderOrder = -10;
  return mesh;
}

// ── pixel particle field ────────────────────────────────────────────
// Real pixels as instanced voxels: sample the ImageData on a grid, one lit cube
// per opaque cell coloured by that pixel. The scene composes each instance's
// matrix per frame (converge / shatter / burst / scan-dust). Lit + tone-mapped
// so they read as little dimensional blocks, not flat squares.
interface PixelField {
  mesh: THREE.InstancedMesh;
  mat: THREE.MeshStandardMaterial;
  count: number;
  bx: Float32Array; // base local x (pixel position on a `size` plane)
  by: Float32Array; // base local y
  rnd: Float32Array; // 3 randoms per instance
  dummy: THREE.Object3D;
  commit(): void;
}
function createPixelField(
  pool: ResourcePool,
  img: ImageData,
  o: { grid: number; size: number; cube: number; cap: number; tint?: THREE.Color; tintAmount?: number },
): PixelField {
  const { grid, size, data } = { grid: o.grid, size: o.size, data: img.data };
  const bxs: number[] = [];
  const bys: number[] = [];
  const cols: THREE.Color[] = [];
  const src = new THREE.Color();
  for (let gy = 0; gy < grid; gy++) {
    for (let gx = 0; gx < grid; gx++) {
      const px = Math.min(img.width - 1, Math.floor((gx + 0.5) / grid * img.width));
      const py = Math.min(img.height - 1, Math.floor((gy + 0.5) / grid * img.height));
      const idx = (py * img.width + px) * 4;
      if (data[idx + 3] < 45) continue;
      src.setRGB(data[idx] / 255, data[idx + 1] / 255, data[idx + 2] / 255, THREE.SRGBColorSpace);
      if (o.tint && o.tintAmount) src.lerp(o.tint, o.tintAmount);
      bxs.push((gx + 0.5) / grid * size - size / 2);
      bys.push(-((gy + 0.5) / grid * size - size / 2));
      cols.push(src.clone());
    }
  }
  // even subsample down to the cap so the shape stays representative
  let count = bxs.length;
  const stride = count > o.cap ? count / o.cap : 1;
  const bx = new Float32Array(Math.min(count, o.cap));
  const by = new Float32Array(bx.length);
  const rnd = new Float32Array(bx.length * 3);
  const geo = pool.track(new THREE.BoxGeometry(o.cube, o.cube, o.cube));
  const mat = pool.track(
    new THREE.MeshStandardMaterial({ roughness: 0.5, metalness: 0, transparent: true, depthWrite: false }),
  );
  const mesh = new THREE.InstancedMesh(geo, mat, bx.length);
  mesh.frustumCulled = false;
  const c = new THREE.Color();
  for (let i = 0; i < bx.length; i++) {
    const s = Math.floor(i * stride);
    bx[i] = bxs[s];
    by[i] = bys[s];
    rnd[i * 3] = Math.random();
    rnd[i * 3 + 1] = Math.random();
    rnd[i * 3 + 2] = Math.random();
    c.copy(cols[s]);
    mesh.setColorAt(i, c);
  }
  if (mesh.instanceColor) mesh.instanceColor.needsUpdate = true;
  count = bx.length;
  pool.track(mesh);
  return {
    mesh,
    mat,
    count,
    bx,
    by,
    rnd,
    dummy: new THREE.Object3D(),
    commit() {
      mesh.instanceMatrix.needsUpdate = true;
    },
  };
}

function setRenderOrder(obj: THREE.Object3D, ro: number): void {
  obj.traverse((o) => {
    o.renderOrder = ro;
  });
}

// ── camera fit: smallest distance that keeps every extent point inside the
// frustum across the rotation range, for the current aspect. Sphere fit is the
// full-yaw special case; a clamped yaw range fits tighter (wide rows stay big).
function fitDistance(
  pts: THREE.Vector3[],
  target: THREE.Vector3,
  fovDeg: number,
  aspect: number,
  yawA: number,
  yawB: number,
  minPitch: number,
  maxPitch: number,
  margin: number,
): number {
  const vHalf = (fovDeg * Math.PI) / 360;
  const tanV = Math.tan(vHalf);
  const tanH = Math.tan(Math.atan(tanV * aspect));
  const up = new THREE.Vector3(0, 1, 0);
  const dir = new THREE.Vector3();
  const forward = new THREE.Vector3();
  const right = new THREE.Vector3();
  const camUp = new THREE.Vector3();
  const rel = new THREE.Vector3();
  let best = 0;
  const YS = 18;
  const PS = 3;
  for (let yi = 0; yi <= YS; yi++) {
    const yaw = lerp(yawA, yawB, yi / YS);
    for (let pi = 0; pi <= PS; pi++) {
      const pitch = lerp(minPitch, maxPitch, pi / PS);
      const cp = Math.cos(pitch);
      dir.set(cp * Math.sin(yaw), Math.sin(pitch), cp * Math.cos(yaw));
      forward.copy(dir).negate();
      right.crossVectors(forward, up);
      if (right.lengthSq() < 1e-6) right.set(1, 0, 0);
      right.normalize();
      camUp.crossVectors(right, forward).normalize();
      for (const p of pts) {
        rel.copy(p).sub(target);
        const along = rel.dot(dir);
        const dH = along + Math.abs(rel.dot(right)) / tanH;
        const dV = along + Math.abs(rel.dot(camUp)) / tanV;
        if (dH > best) best = dH;
        if (dV > best) best = dV;
      }
    }
  }
  return best * margin;
}

// ── the shared Stage ────────────────────────────────────────────────
interface BuildCtx {
  content: THREE.Group;
  pool: ResourcePool;
  reduceMotion: boolean;
  /** unit (1×1) plane facing +Z; scale per mesh */
  unitPlane: THREE.PlaneGeometry;
  tex(img: ImageData): THREE.CanvasTexture;
  /** MeshBasicMaterial pre-wired for a transparent, unlit artwork plane */
  basicTex(img: ImageData): THREE.MeshBasicMaterial;
  /** upright plane wearing `mat`, wrapped in an unscaled group (safe for anchors) */
  plane(mat: THREE.Material, size: number, order: number): { group: THREE.Group; mesh: THREE.Mesh };
  card(w: number, h: number, r: number): THREE.Mesh;
  chip(w: number, h: number, r: number, color: THREE.ColorRepresentation): Chip;
  shadow(size: number): THREE.Mesh;
  pixels(img: ImageData, o: { grid: number; size: number; cube: number; cap: number; tint?: THREE.Color; tintAmount?: number }): PixelField;
}

interface LabelSpec {
  id: string;
  anchor: THREE.Object3D;
  visible?: () => boolean;
}
interface SceneLogic {
  update(clock: number, dt: number): void;
  onReplay(): void;
  onState?(name: string, clock: number): void;
  labels: LabelSpec[];
  /** world-space (content-local) bounds of everything across the timeline */
  extent: THREE.Box3;
}

interface StageConfig {
  fov: number;
  minPitch: number;
  maxPitch: number;
  initialYaw: number;
  initialPitch: number;
  autoYaw: number;
  breatheAmp: number;
  orbitScale: number;
  enableDblReplay: boolean;
  /** if set, yaw (drag + idle) is limited to ±this, and the fit uses that range */
  yawClamp?: number;
  marginFactor?: number;
}

function createStageHandle(
  canvas: HTMLCanvasElement,
  common: SceneCommonOpts,
  cfg: StageConfig,
  build: (ctx: BuildCtx) => SceneLogic,
): SceneHandle {
  const { reduceMotion, onLabel } = common;

  const renderer = new THREE.WebGLRenderer({ canvas, alpha: true, antialias: true });
  renderer.setPixelRatio(Math.min(2, window.devicePixelRatio || 1));
  renderer.outputColorSpace = THREE.SRGBColorSpace;
  renderer.toneMapping = THREE.ACESFilmicToneMapping;
  renderer.toneMappingExposure = 1.02;
  renderer.setClearColor(0x000000, 0);

  const scene = new THREE.Scene();
  const camera = new THREE.PerspectiveCamera(cfg.fov, 1, 0.1, 200);

  // studio IBL (zero external assets) + a key/fill so glass reflections have
  // something bright to roll and particles read as lit dimensional blocks
  const pmrem = new THREE.PMREMGenerator(renderer);
  const envScene = new RoomEnvironment();
  const envRT = pmrem.fromScene(envScene, 0.04);
  scene.environment = envRT.texture;
  envScene.traverse((o) => {
    const m = o as THREE.Mesh;
    if (m.isMesh) {
      m.geometry.dispose();
      const mm = m.material;
      if (Array.isArray(mm)) for (const x of mm) x.dispose();
      else mm.dispose();
    }
  });
  pmrem.dispose();

  const key = new THREE.DirectionalLight(0xffffff, 1.35);
  key.position.set(-3, 4.5, 5);
  const fill = new THREE.DirectionalLight(0xdfe6f0, 0.4);
  fill.position.set(4, 1.5, 2);
  const ambient = new THREE.HemisphereLight(0xffffff, 0x2a2d33, 0.35);
  scene.add(key, fill, ambient);

  const content = new THREE.Group();
  scene.add(content);

  const pool = new ResourcePool();
  const unitPlane = pool.track(new THREE.PlaneGeometry(1, 1));

  const tex = (img: ImageData): THREE.CanvasTexture => {
    const c = document.createElement("canvas");
    c.width = img.width;
    c.height = img.height;
    const g = c.getContext("2d");
    if (g) g.putImageData(img, 0, 0);
    const t = new THREE.CanvasTexture(c);
    t.colorSpace = THREE.SRGBColorSpace;
    t.anisotropy = Math.min(8, renderer.capabilities.getMaxAnisotropy());
    t.minFilter = THREE.LinearMipmapLinearFilter;
    return pool.track(t);
  };
  const basicTex = (img: ImageData): THREE.MeshBasicMaterial =>
    pool.track(new THREE.MeshBasicMaterial({
      map: tex(img), transparent: true, depthWrite: false, toneMapped: false, side: THREE.DoubleSide,
    }));
  const plane = (mat: THREE.Material, size: number, order: number) => {
    const group = new THREE.Group();
    const mesh = new THREE.Mesh(unitPlane, mat);
    mesh.scale.set(size, size, 1);
    mesh.renderOrder = order;
    group.add(mesh);
    return { group, mesh };
  };
  const ctx: BuildCtx = {
    content, pool, reduceMotion, unitPlane, tex, basicTex, plane,
    card: (w, h, r) => buildGlassCard(pool, w, h, r),
    chip: (w, h, r, color) => buildChip(pool, w, h, r, color),
    shadow: (size) => buildShadow(pool, size),
    pixels: (img, o) => createPixelField(pool, img, o),
  };
  const logic = build(ctx);

  // ── camera fit ────────────────────────────────────────────────────
  const box = logic.extent.clone().expandByScalar(cfg.breatheAmp + 0.07);
  const target = box.getCenter(new THREE.Vector3());
  const corners: THREE.Vector3[] = [];
  for (let i = 0; i < 8; i++) {
    corners.push(new THREE.Vector3(
      i & 1 ? box.max.x : box.min.x,
      i & 2 ? box.max.y : box.min.y,
      i & 4 ? box.max.z : box.min.z,
    ));
  }
  const yawA = cfg.yawClamp !== undefined ? -cfg.yawClamp : 0;
  const yawB = cfg.yawClamp !== undefined ? cfg.yawClamp : Math.PI * 2;
  let distance = 6;

  // ── orbit state ───────────────────────────────────────────────────
  let yaw = cfg.initialYaw;
  let pitch = cfg.initialPitch;
  let yawVel = 0;
  let pitchVel = 0;
  let dragging = false;
  let hover = 0;
  let hoverTarget = 0;
  let lastX = 0;
  let lastY = 0;
  let lastMoveT = 0;
  const sens = 0.0075 * cfg.orbitScale;
  const yawLimit = cfg.yawClamp;

  // visual-QA hook: ?engfreeze=<seconds> renders one deterministic frame of the
  // timeline (no loop, no idle motion) so screenshots hit exact phases.
  const freezeParam = new URLSearchParams(window.location.search).get("engfreeze");
  const freeze = freezeParam !== null && Number.isFinite(parseFloat(freezeParam)) ? parseFloat(freezeParam) : null;

  let cssW = 1;
  let cssH = 1;
  let clock = 0;
  let lastT = 0;
  let raf = 0;
  let running = false;
  let visible = true;
  let pageVisible = !document.hidden;

  const _cp = new THREE.Vector3();
  const _wp = new THREE.Vector3();
  const _view = new THREE.Vector3();

  const applyCamera = (): void => {
    const cp = Math.cos(pitch);
    _cp.set(cp * Math.sin(yaw), Math.sin(pitch), cp * Math.cos(yaw)).multiplyScalar(distance).add(target);
    camera.position.copy(_cp);
    camera.up.set(0, 1, 0);
    camera.lookAt(target);
  };

  const projectLabels = (): void => {
    if (!onLabel) return;
    for (const spec of logic.labels) {
      spec.anchor.getWorldPosition(_wp);
      _view.copy(_wp).applyMatrix4(camera.matrixWorldInverse);
      const front = _view.z < 0;
      _wp.project(camera);
      const rawX = (_wp.x * 0.5 + 0.5) * cssW;
      const rawY = (-_wp.y * 0.5 + 0.5) * cssH;
      const onCanvas = rawX >= -cssW * 0.12 && rawX <= cssW * 1.12 && rawY >= -cssH * 0.12 && rawY <= cssH * 1.12;
      const x = clamp(rawX, 10, Math.max(10, cssW - 84));
      const y = clamp(rawY, 10, Math.max(10, cssH - 26));
      const specVisible = spec.visible ? spec.visible() : true;
      const pt: LabelPoint = { x, y, visible: front && onCanvas && specVisible };
      onLabel(spec.id, pt);
    }
  };

  const orbitActive = (): boolean =>
    dragging || Math.abs(yawVel) > 0.02 || Math.abs(pitchVel) > 0.02 || Math.abs(hover - hoverTarget) > 0.01;

  const renderFrame = (dt: number): void => {
    if (freeze !== null) {
      content.position.set(0, 0, 0);
      logic.update(freeze, 0);
      applyCamera();
      renderer.render(scene, camera);
      projectLabels();
      return;
    }
    clock += dt;
    if (!dragging) {
      yaw += yawVel * dt;
      pitch = clamp(pitch + pitchVel * dt, cfg.minPitch, cfg.maxPitch);
      const decay = Math.exp(-3.2 * dt);
      yawVel *= decay;
      pitchVel *= decay;
      if (Math.abs(yawVel) < 0.02) yawVel = 0;
      if (Math.abs(pitchVel) < 0.02) pitchVel = 0;
      if (yawLimit !== undefined) {
        yaw = clamp(yaw, -yawLimit, yawLimit);
        if (yawVel === 0 && !reduceMotion) {
          const swayTarget = Math.sin(clock * 0.3) * yawLimit * 0.5;
          yaw += (swayTarget - yaw) * (1 - Math.exp(-1.6 * dt));
        }
      } else if (!reduceMotion && yawVel === 0) {
        yaw += cfg.autoYaw * (1 + hover * 0.6) * dt;
      }
    }
    hover += (hoverTarget - hover) * (1 - Math.exp(-8 * dt));
    content.position.y = reduceMotion ? 0 : Math.sin(clock * 0.5) * cfg.breatheAmp;
    content.position.z = hover * 0.07;
    logic.update(clock, dt);
    applyCamera();
    renderer.render(scene, camera);
    projectLabels();
  };

  const frame = (now: number): void => {
    const dt = lastT === 0 ? 0 : Math.min(0.05, (now - lastT) / 1000);
    lastT = now;
    renderFrame(dt);
    if (!visible || !pageVisible) {
      running = false;
      return;
    }
    if (reduceMotion && !orbitActive()) {
      running = false;
      return;
    }
    raf = requestAnimationFrame(frame);
  };

  const wake = (): void => {
    if (running || !visible || !pageVisible) return;
    running = true;
    lastT = 0;
    raf = requestAnimationFrame(frame);
  };

  const resize = (): void => {
    const w = canvas.clientWidth || 1;
    const h = canvas.clientHeight || 1;
    cssW = w;
    cssH = h;
    renderer.setSize(w, h, false);
    camera.aspect = w / h;
    camera.updateProjectionMatrix();
    distance = fitDistance(corners, target, cfg.fov, w / h, yawA, yawB, cfg.minPitch, cfg.maxPitch, cfg.marginFactor ?? 1.12);
    if (!running) renderFrame(0);
  };

  // ── pointer / orbit input ────────────────────────────────────────
  const onDown = (e: PointerEvent): void => {
    dragging = true;
    lastX = e.clientX;
    lastY = e.clientY;
    lastMoveT = performance.now();
    yawVel = 0;
    pitchVel = 0;
    try { canvas.setPointerCapture(e.pointerId); } catch { /* ignore */ }
    wake();
  };
  const onMove = (e: PointerEvent): void => {
    if (!dragging) return;
    const now = performance.now();
    const mdt = Math.max((now - lastMoveT) / 1000, 1 / 240);
    const dx = e.clientX - lastX;
    const dy = e.clientY - lastY;
    lastX = e.clientX;
    lastY = e.clientY;
    lastMoveT = now;
    const dYaw = -dx * sens;
    const dPitch = -dy * sens;
    yaw += dYaw;
    if (yawLimit !== undefined) yaw = clamp(yaw, -yawLimit, yawLimit);
    pitch = clamp(pitch + dPitch, cfg.minPitch, cfg.maxPitch);
    yawVel = dYaw / mdt;
    pitchVel = dPitch / mdt;
    if (!running) renderFrame(0);
    wake();
  };
  const onUp = (e: PointerEvent): void => {
    if (!dragging) return;
    dragging = false;
    try { canvas.releasePointerCapture(e.pointerId); } catch { /* ignore */ }
    wake();
  };
  const onEnter = (): void => {
    hoverTarget = 1;
    wake();
  };
  const onLeave = (): void => {
    hoverTarget = 0;
    wake();
  };

  const dispose = (): void => {
    running = false;
    cancelAnimationFrame(raf);
    io.disconnect();
    ro.disconnect();
    document.removeEventListener("visibilitychange", onVis);
    canvas.removeEventListener("pointerdown", onDown);
    canvas.removeEventListener("pointermove", onMove);
    window.removeEventListener("pointerup", onUp);
    canvas.removeEventListener("pointerenter", onEnter);
    canvas.removeEventListener("pointerleave", onLeave);
    if (cfg.enableDblReplay) canvas.removeEventListener("dblclick", onDbl);
    envRT.dispose();
    pool.disposeAll();
    renderer.dispose();
  };
  const replay = (): void => {
    clock = 0;
    logic.onReplay();
    if (!running) renderFrame(0);
    wake();
  };
  const setState = (name: string): void => {
    logic.onState?.(name, clock);
    if (!running) renderFrame(0);
    wake();
  };
  const onDbl = (): void => replay();

  canvas.style.touchAction = "none";
  canvas.addEventListener("pointerdown", onDown);
  canvas.addEventListener("pointermove", onMove);
  window.addEventListener("pointerup", onUp);
  canvas.addEventListener("pointerenter", onEnter);
  canvas.addEventListener("pointerleave", onLeave);
  if (cfg.enableDblReplay) canvas.addEventListener("dblclick", onDbl);

  const ro = new ResizeObserver(resize);
  ro.observe(canvas);
  const io = new IntersectionObserver(
    (records) => {
      visible = records.some((r) => r.isIntersecting);
      if (visible) wake();
      else {
        running = false;
        cancelAnimationFrame(raf);
      }
    },
    { threshold: 0.05 },
  );
  io.observe(canvas);
  const onVis = (): void => {
    pageVisible = !document.hidden;
    if (pageVisible) wake();
    else {
      running = false;
      cancelAnimationFrame(raf);
    }
  };
  document.addEventListener("visibilitychange", onVis);

  resize();
  renderFrame(0);
  if (!reduceMotion && freeze === null) wake();

  return { dispose, replay, setState };
}

// ── shared layer primitive (glass card + upright artwork + right anchor) ──
interface Layer {
  group: THREE.Group;
  mesh: THREE.Mesh;
  mat: THREE.MeshBasicMaterial;
  right: THREE.Object3D;
}
function makeLayer(ctx: BuildCtx, img: ImageData, size: number, order: number): Layer {
  const group = new THREE.Group();
  const card = ctx.card(size + 0.22, size + 0.22, 0.16);
  card.position.z = -0.05;
  card.renderOrder = order;
  group.add(card);
  const mat = ctx.basicTex(img);
  const { group: mg, mesh } = ctx.plane(mat, size, order + 1);
  group.add(mg);
  const right = new THREE.Object3D();
  right.position.set(size / 2 + 0.14, 0, 0);
  group.add(right);
  return { group, mesh, mat, right };
}

// ════════════════════════════════════════════════════════════════════
// 1. HERO — the engine exploded diagram, with a burst of pixel dust
// ════════════════════════════════════════════════════════════════════
export const createHeroScene: SceneInit<HeroAssets> = (canvas, assets, opts) =>
  createStageHandle(
    canvas,
    opts,
    { fov: 34, minPitch: -0.15, maxPitch: 0.5, initialYaw: 0.5, initialPitch: 0.24,
      autoYaw: 0.09, breatheAmp: 0.03, orbitScale: 1, enableDblReplay: true },
    (ctx) => {
      const S = 1.5;
      const imgs = [assets.raw, assets.plate, assets.final];
      const layers = imgs.map((img, i) => {
        const L = makeLayer(ctx, img, S, i * 10);
        ctx.content.add(L.group);
        return L;
      });
      const sh = ctx.shadow(1.9);
      sh.position.y = -1.2;
      ctx.content.add(sh);
      const shMat = sh.material as THREE.MeshBasicMaterial;

      // pixel dust: a small burst sampled from the finished tile, flung out and
      // up at the moment of the bang, dissipating in ~0.5s
      const dust = ctx.pixels(assets.final, { grid: 19, size: S, cube: 0.078, cap: 300 });
      dust.mat.opacity = 0;
      dust.mesh.renderOrder = 30; // dust reads over the cards during the bang
      ctx.content.add(dust.mesh);
      const dvx = new Float32Array(dust.count);
      const dvy = new Float32Array(dust.count);
      const dvz = new Float32Array(dust.count);
      for (let i = 0; i < dust.count; i++) {
        const a = dust.rnd[i * 3] * Math.PI * 2;
        const sp = 0.5 + dust.rnd[i * 3 + 1] * 0.7;
        dvx[i] = Math.cos(a) * sp;
        dvy[i] = 0.4 + dust.rnd[i * 3 + 1] * 0.8;
        dvz[i] = Math.sin(a) * sp * 0.5;
      }

      const GAP = 0.54;
      const tY = [-GAP, 0, GAP];
      const tZ = [-0.16, 0, 0.16];
      const ids = ["raw", "plate", "final"] as const;
      const HOLD = 0.4;
      const DUR = 0.62;
      const CDUR = 0.28;
      const RHOLD = 0.12;
      let mode: "intro" | "replay" = "intro";

      const explodeStart = (): number => (mode === "intro" ? HOLD : CDUR + RHOLD);
      const explodeAt = (i: number, t: number): number => {
        if (ctx.reduceMotion) return 1;
        if (mode === "replay" && t < CDUR) return 1 - easeInOutCubic(clamp01(t / CDUR));
        const start = explodeStart() + i * 0.045;
        return start <= t ? easeOutBack(clamp01((t - start) / DUR), 0.9) : 0;
      };

      const extent = new THREE.Box3(new THREE.Vector3(-1.35, -1.7, -0.55), new THREE.Vector3(1.35, 1.75, 0.55));

      return {
        extent,
        labels: layers.map((L, i) => ({ id: ids[i], anchor: L.right })),
        onReplay() {
          mode = "replay";
        },
        update(t) {
          for (let i = 0; i < layers.length; i++) {
            const e = explodeAt(i, t);
            const floaty = ctx.reduceMotion ? 0 : Math.sin(t * 0.8 + i * 1.9) * 0.02;
            layers[i].group.position.set(0, tY[i] * e + floaty, tZ[i] * e);
          }
          const eMid = explodeAt(1, t);
          shMat.opacity = 0.32 * (0.4 + 0.6 * clamp01(eMid));

          // dust burst keyed to the explosion instant
          if (ctx.reduceMotion) {
            dust.mat.opacity = 0;
            return;
          }
          const burstT = t - explodeStart();
          const life = clamp01(burstT / 0.6);
          if (burstT >= 0 && life < 1) {
            dust.mat.opacity = 0.95 * (1 - life * life);
            const g = 1.0 * burstT * burstT;
            for (let i = 0; i < dust.count; i++) {
              const r2 = dust.rnd[i * 3 + 2];
              dust.dummy.position.set(
                dust.bx[i] + dvx[i] * burstT,
                dust.by[i] + dvy[i] * burstT - g,
                dvz[i] * burstT,
              );
              dust.dummy.rotation.set(burstT * (dust.rnd[i * 3] - 0.5) * 8, burstT * r2 * 8, 0);
              dust.dummy.scale.setScalar((1 - life * 0.7) * (0.8 + r2 * 0.5));
              dust.dummy.updateMatrix();
              dust.mesh.setMatrixAt(i, dust.dummy.matrix);
            }
            dust.commit();
          } else if (dust.mat.opacity !== 0) {
            dust.mat.opacity = 0;
          }
        },
      };
    },
  );

// ════════════════════════════════════════════════════════════════════
// 2. READ — the checkup: scan sweep raising pixel dust, then readouts
// ════════════════════════════════════════════════════════════════════
export const createReadScene: SceneInit<ReadAssets> = (canvas, assets, opts) =>
  createStageHandle(
    canvas,
    opts,
    { fov: 32, minPitch: -0.15, maxPitch: 0.5, initialYaw: 0.12, initialPitch: 0.16,
      autoYaw: 0, breatheAmp: 0.02, orbitScale: 1, enableDblReplay: false },
    (ctx) => {
      const S = 1.4;
      const SCAN = 1.2;
      const span = S + 0.24;

      const base = new THREE.Group();
      const baseCard = ctx.card(S + 0.22, S + 0.22, 0.16);
      baseCard.position.z = -0.05;
      baseCard.renderOrder = 0;
      base.add(baseCard);
      const iconMat = ctx.basicTex(assets.icon);
      const { group: iconG } = ctx.plane(iconMat, S, 2);
      base.add(iconG);
      ctx.content.add(base);

      const sh = ctx.shadow(1.7);
      sh.position.y = -1.02;
      ctx.content.add(sh);

      // sweeping coral scan bar (additive) + rising pixel data-dust
      const barMat = ctx.pool.track(new THREE.MeshBasicMaterial({
        color: new THREE.Color(COLOR_CORAL), transparent: true, opacity: 0, depthWrite: false,
        toneMapped: false, blending: THREE.AdditiveBlending, side: THREE.DoubleSide,
      }));
      const bar = new THREE.Mesh(ctx.unitPlane, barMat);
      bar.scale.set(0.06, S + 0.26, 1);
      bar.position.z = 0.08;
      bar.renderOrder = 20;
      ctx.content.add(bar);

      const dust = ctx.pixels(assets.icon, { grid: 24, size: S, cube: 0.042, cap: 210 });
      dust.mat.opacity = 0.95;
      dust.mesh.position.z = 0.06;
      ctx.content.add(dust.mesh);

      // outline layer — lifts up + toward the viewer
      const outMat = ctx.basicTex(assets.outline);
      outMat.opacity = 0;
      const { group: outG } = ctx.plane(outMat, S, 14);
      ctx.content.add(outG);
      const outAnchor = new THREE.Object3D();
      outAnchor.position.set(S * 0.4, S * 0.32, 0);
      outG.add(outAnchor);

      // colour chip — flies from the icon to the upper right
      const chip = ctx.chip(0.42, 0.42, 0.1, assets.seedHex);
      chip.fillMat.opacity = 0;
      setRenderOrder(chip.group, 16);
      const chipG = new THREE.Group();
      chipG.add(chip.group);
      ctx.content.add(chipG);
      const chipAnchor = new THREE.Object3D();
      chipAnchor.position.set(0.18, 0.15, 0);
      chipG.add(chipAnchor);
      const chipStart = new THREE.Vector3(0, 0, 0.05);
      const chipEnd = new THREE.Vector3(0.56, 0.5, 0.24);

      // profile — a row of three tick blocks below the icon
      const tickCols = [COLOR_INK, COLOR_CORAL, COLOR_TEAL];
      const ticks = tickCols.map((col, i) => {
        const mat = ctx.pool.track(new THREE.MeshBasicMaterial({
          color: new THREE.Color(col), transparent: true, opacity: 0,
          depthWrite: false, toneMapped: false, side: THREE.DoubleSide,
        }));
        const m = new THREE.Mesh(ctx.unitPlane, mat);
        m.scale.set(0.07, 0.07, 1);
        m.position.set((i - 1) * 0.14, 0, 0);
        m.renderOrder = 12;
        return { m, mat };
      });
      const profileG = new THREE.Group();
      for (const t of ticks) profileG.add(t.m);
      profileG.position.set(0, -S * 0.54, 0.06);
      ctx.content.add(profileG);
      const profAnchor = new THREE.Object3D();
      profAnchor.position.set(0.3, 0.02, 0);
      profileG.add(profAnchor);

      const hideDust = (): void => {
        for (let i = 0; i < dust.count; i++) {
          dust.dummy.position.set(dust.bx[i], dust.by[i], 0);
          dust.dummy.scale.setScalar(0);
          dust.dummy.updateMatrix();
          dust.mesh.setMatrixAt(i, dust.dummy.matrix);
        }
        dust.commit();
      };
      const terminal = (): void => {
        barMat.opacity = 0;
        iconMat.color.setScalar(1);
        outG.position.set(0, 0.42, 0.28);
        outMat.opacity = 1;
        chipG.position.copy(chipEnd);
        chip.fillMat.opacity = 1;
        for (const t of ticks) {
          t.mat.opacity = 1;
          t.m.scale.setScalar(0.07);
        }
        hideDust();
      };

      const extent = new THREE.Box3(new THREE.Vector3(-1.1, -1.15, -0.35), new THREE.Vector3(1.1, 1.18, 0.4));

      return {
        extent,
        labels: [
          { id: "outline", anchor: outAnchor, visible: () => outMat.opacity > 0.05 },
          { id: "color", anchor: chipAnchor, visible: () => chip.fillMat.opacity > 0.05 },
          { id: "profile", anchor: profAnchor, visible: () => ticks[1].mat.opacity > 0.05 },
        ],
        onReplay() {
          /* replay is just clock=0 → re-scan */
        },
        update(t) {
          if (ctx.reduceMotion) {
            terminal();
            return;
          }
          const sc = clamp01(t / SCAN); // linear — scanners don't ease
          const scanning = t < SCAN;
          const scanX = -span / 2 + sc * span;
          bar.position.x = scanX;
          barMat.opacity = scanning ? 0.5 : 0;
          iconMat.color.setScalar(scanning ? 1 + 0.5 * Math.exp(-Math.pow((sc - 0.5) / 0.22, 2)) : 1);

          // once the scan line passes a column, that column's real pixels lift
          // off and plume upward, fading — data dust in the icon's own colours
          if (scanning) {
            for (let i = 0; i < dust.count; i++) {
              const age = (scanX - dust.bx[i]) / 0.26; // 0 at the line → 1 gone
              if (age >= -0.02 && age < 1) {
                const rise = Math.max(0, age) * 0.45;
                dust.dummy.position.set(
                  dust.bx[i] + (dust.rnd[i * 3] - 0.5) * 0.05 * age,
                  dust.by[i] + rise + Math.sin(t * 6 + dust.rnd[i * 3 + 1] * 6) * 0.02,
                  0.05,
                );
                dust.dummy.scale.setScalar((1 - Math.max(0, age)) * (0.8 + dust.rnd[i * 3 + 2] * 0.5));
              } else {
                dust.dummy.scale.setScalar(0);
              }
              dust.dummy.updateMatrix();
              dust.mesh.setMatrixAt(i, dust.dummy.matrix);
            }
            dust.commit();
          } else {
            hideDust();
          }

          const pOut = easeOutExpo(clamp01((t - SCAN) / 0.5));
          outG.position.set(0, 0.42 * pOut + Math.sin(t * 1.1) * 0.012 * pOut, 0.28 * pOut);
          outMat.opacity = pOut;

          const pCol = easeOutExpo(clamp01((t - (SCAN + 0.15)) / 0.5));
          chipG.position.lerpVectors(chipStart, chipEnd, pCol);
          chipG.position.y += Math.sin(t * 1.0 + 1) * 0.012 * pCol;
          chip.fillMat.opacity = pCol;

          for (let i = 0; i < ticks.length; i++) {
            const p = easeOutBack(clamp01((t - (SCAN + 0.3) - i * 0.08) / 0.42), 0.9);
            ticks[i].mat.opacity = clamp01(p);
            ticks[i].m.scale.setScalar(0.07 * (0.5 + 0.5 * clamp01(p)));
          }
        },
      };
    },
  );

// ════════════════════════════════════════════════════════════════════
// 3. CUT — the background is identified, then shatters into its pixels
// ════════════════════════════════════════════════════════════════════
export const createCutScene: SceneInit<CutAssets> = (canvas, assets, opts) =>
  createStageHandle(
    canvas,
    opts,
    { fov: 32, minPitch: -0.15, maxPitch: 0.5, initialYaw: 0.14, initialPitch: 0.16,
      autoYaw: 0, breatheAmp: 0.02, orbitScale: 1, enableDblReplay: true },
    (ctx) => {
      const S = 1.42;
      const WHITE = new THREE.Color(1, 1, 1);
      const CORALc = new THREE.Color(COLOR_CORAL);

      const baseCard = ctx.card(S + 0.22, S + 0.22, 0.16);
      baseCard.position.z = -0.06;
      baseCard.renderOrder = 0;
      ctx.content.add(baseCard);
      const sh = ctx.shadow(1.7);
      sh.position.y = -1.02;
      ctx.content.add(sh);

      // the icon arrives as TWO real engine layers: its own base (bgLayer, the
      // glass basket) and the true artwork (artLayer, the recycle glyph)
      const bgMat = ctx.basicTex(assets.bgLayer);
      const bg = ctx.plane(bgMat, S, 2);
      bg.group.position.z = 0.01;
      ctx.content.add(bg.group);
      const artMat = ctx.basicTex(assets.artLayer);
      const art = ctx.plane(artMat, S, 3);
      art.group.position.z = 0.02;
      ctx.content.add(art.group);
      const artAnchor = new THREE.Object3D();
      artAnchor.position.set(S * 0.4, -S * 0.15, 0);
      art.group.add(artAnchor);
      // the "base identified" callout rides the coral pixels as they disperse
      const bgAnchor = new THREE.Object3D();
      bgAnchor.position.set(-S * 0.34, S * 0.36, 0.1);
      ctx.content.add(bgAnchor);

      const finalMat = ctx.basicTex(assets.final);
      finalMat.opacity = 0;
      const fin = ctx.plane(finalMat, S, 4);
      fin.group.position.set(0, -0.9, 0.03);
      ctx.content.add(fin.group);
      const finAnchor = new THREE.Object3D();
      finAnchor.position.set(S * 0.4, 0, 0);
      fin.group.add(finAnchor);

      // the BASE layer's own pixels, coral-tinted: only the layer the engine
      // judged background pixelates and rains away — the artwork never moves
      // and never changes colour (the iron law holds inside the viz too)
      const shards = ctx.pixels(assets.bgLayer, { grid: 48, size: S, cube: 0.03, cap: 1800, tint: CORALc, tintAmount: 0.5 });
      shards.mat.opacity = 0;
      shards.mesh.position.z = 0.05;
      shards.mesh.renderOrder = 12; // in front, so the coral pixelate reads clearly
      ctx.content.add(shards.mesh);
      const svx = new Float32Array(shards.count);
      for (let i = 0; i < shards.count; i++) svx[i] = (shards.rnd[i * 3] - 0.5) * 0.55;

      const HOLD = 0.3;
      const SHATTER = HOLD + 0.86; // the panel pixelates and disperses here
      const SDUR = 1.0;
      const placeShards = (p: number): void => {
        for (let i = 0; i < shards.count; i++) {
          const r0 = shards.rnd[i * 3];
          const r1 = shards.rnd[i * 3 + 1];
          const r2 = shards.rnd[i * 3 + 2];
          const pp = clamp01((p - r2 * 0.16) / 0.84); // staggered release
          const fall = 0.8 * pp;
          const sway = Math.sin(pp * 4 + r1 * 6) * 0.07 * pp;
          shards.dummy.position.set(
            shards.bx[i] + svx[i] * pp + sway,
            shards.by[i] - fall,
            (r1 - 0.5) * 0.22 * pp,
          );
          shards.dummy.rotation.set(pp * (r0 - 0.5) * 5, pp * (r1 - 0.5) * 5, 0);
          shards.dummy.scale.setScalar(0.94 * (1 - pp * 0.28));
          shards.dummy.updateMatrix();
          shards.mesh.setMatrixAt(i, shards.dummy.matrix);
        }
        shards.commit();
      };
      const hideShards = (): void => {
        for (let i = 0; i < shards.count; i++) {
          shards.dummy.scale.setScalar(0);
          shards.dummy.updateMatrix();
          shards.mesh.setMatrixAt(i, shards.dummy.matrix);
        }
        shards.commit();
      };

      const terminal = (): void => {
        bgMat.opacity = 0;
        artMat.opacity = 0;
        finalMat.opacity = 1;
        fin.group.position.set(0, 0, 0.03);
        shards.mat.opacity = 0;
        hideShards();
      };

      const extent = new THREE.Box3(new THREE.Vector3(-1.15, -1.5, -0.45), new THREE.Vector3(1.15, 0.85, 0.45));

      return {
        extent,
        labels: [
          { id: "bg", anchor: bgAnchor, visible: () => bgMat.opacity > 0.05 || shards.mat.opacity > 0.05 },
          { id: "art", anchor: artAnchor, visible: () => artMat.opacity > 0.05 },
          { id: "final", anchor: finAnchor, visible: () => finalMat.opacity > 0.05 },
        ],
        onReplay() {
          /* clock=0 replays the split */
        },
        update(t) {
          if (ctx.reduceMotion) {
            terminal();
            return;
          }
          // recognition: ONLY the base layer flushes coral with a double
          // blink, then that layer alone pixelates and rains away — the
          // artwork glyph sits still, untouched, the whole time
          const blinkP = clamp01((t - HOLD) / 0.36);
          const blink = blinkP > 0 && blinkP < 1 ? Math.abs(Math.sin(blinkP * Math.PI * 2)) * 0.6 : 0;
          const t1 = clamp01((t - (HOLD + 0.36)) / 0.4);

          if (t < SHATTER) {
            bgMat.color.copy(WHITE).lerp(CORALc, Math.max(t1, blink));
            bgMat.opacity = 1;
            shards.mat.opacity = 0;
            hideShards();
          } else {
            const sp = clamp01((t - SHATTER) / SDUR);
            bgMat.opacity = 0; // hand off to the coral voxels in one frame
            shards.mat.opacity = 0.98 * (1 - smooth(0.5, 0.82, sp));
            placeShards(sp);
          }

          const t3 = easeOutExpo(clamp01((t - (SHATTER + 0.5)) / 0.55));
          fin.group.position.y = -0.9 + 0.9 * t3;
          finalMat.opacity = t3;
          artMat.opacity = 1 - t3; // the glyph hands off to the finished tile
        },
      };
    },
  );

// ════════════════════════════════════════════════════════════════════
// 4. RESCUE — the rescue pixels converge from around, then merge in
// ════════════════════════════════════════════════════════════════════
export const createRescueScene: SceneInit<RescueAssets> = (canvas, assets, opts) =>
  createStageHandle(
    canvas,
    opts,
    { fov: 32, minPitch: -0.15, maxPitch: 0.5, initialYaw: 0.16, initialPitch: 0.18,
      autoYaw: 0, breatheAmp: 0.02, orbitScale: 1, enableDblReplay: true },
    (ctx) => {
      const S = 1.42;
      const LIFT = 0.45;

      const baseCard = ctx.card(S + 0.22, S + 0.22, 0.16);
      baseCard.position.z = -0.06;
      baseCard.renderOrder = 0;
      ctx.content.add(baseCard);
      const sh = ctx.shadow(1.7);
      sh.position.y = -1.02;
      ctx.content.add(sh);

      const offTex = ctx.tex(assets.off);
      const onTex = ctx.tex(assets.on);
      const tileMat = ctx.pool.track(new THREE.MeshBasicMaterial({
        map: offTex, transparent: true, depthWrite: false, toneMapped: false, side: THREE.DoubleSide,
      }));
      const tile = ctx.plane(tileMat, S, 1);
      ctx.content.add(tile.group);
      const tileAnchor = new THREE.Object3D();
      tileAnchor.position.set(S * 0.4, -S * 0.4, 0);
      tile.group.add(tileAnchor);

      // rescue layer as pixels that converge in from around, floating above the
      // tile, then ride down and merge. Lives in a group so drp drops them.
      const rescueG = new THREE.Group();
      rescueG.position.set(0, LIFT, 0.1);
      ctx.content.add(rescueG);
      const pix = ctx.pixels(assets.rescueLayer, { grid: 60, size: S, cube: 0.046, cap: 1000 });
      pix.mat.opacity = 0;
      rescueG.add(pix.mesh);
      const sxo = new Float32Array(pix.count);
      const syo = new Float32Array(pix.count);
      for (let i = 0; i < pix.count; i++) {
        const a = pix.rnd[i * 3] * Math.PI * 2;
        const rad = 0.3 + pix.rnd[i * 3 + 1] * 0.28;
        // a squashed ring, biased downward, so the fly-in never rises past the frame
        sxo[i] = Math.cos(a) * rad;
        syo[i] = Math.sin(a) * rad * 0.6 - 0.22;
      }
      const rescueAnchor = new THREE.Object3D();
      rescueAnchor.position.set(S * 0.4, S * 0.2, 0);
      rescueG.add(rescueAnchor);

      let app = 0;
      let drp = 0;
      let mode: "intro" | "manual" = "intro";
      let drpFrom = 0;
      let drpTo = 0;
      let drpAt = -999;
      let isOn = false;
      let rebounding = false;
      let reboundT = 0;

      const setMap = (on: boolean): void => {
        if (on === isOn) return;
        isOn = on;
        tileMat.map = on ? onTex : offTex;
        tileMat.needsUpdate = true;
        if (on) {
          rebounding = true;
          reboundT = 0;
        }
      };
      // place pixels: converge (app 0→1) from scattered ring to assembled form
      const placePix = (): void => {
        const conv = app; // linear so the fly-in stays visible, not a snap
        for (let i = 0; i < pix.count; i++) {
          const r0 = pix.rnd[i * 3];
          const jx = Math.sin(reboundClock * 1.2 + r0 * 6) * 0.01 * (1 - drp);
          pix.dummy.position.set(
            lerp(pix.bx[i] + sxo[i], pix.bx[i], conv) + jx,
            lerp(pix.by[i] + syo[i], pix.by[i], conv),
            lerp(0.35, 0, conv),
          );
          // tumble while flying in, settle flat as they land
          pix.dummy.rotation.set((1 - conv) * (r0 - 0.5) * 5, (1 - conv) * (pix.rnd[i * 3 + 1] - 0.5) * 5, 0);
          pix.dummy.scale.setScalar(conv * (0.85 + pix.rnd[i * 3 + 2] * 0.3));
          pix.dummy.updateMatrix();
          pix.mesh.setMatrixAt(i, pix.dummy.matrix);
        }
        pix.commit();
      };
      let reboundClock = 0;

      const extent = new THREE.Box3(new THREE.Vector3(-1.3, -1.1, -0.4), new THREE.Vector3(1.3, 1.35, 0.5));

      return {
        extent,
        labels: [
          { id: "tile", anchor: tileAnchor },
          { id: "rescue", anchor: rescueAnchor, visible: () => pix.mat.opacity > 0.05 },
        ],
        onReplay() {
          mode = "intro";
          app = 0;
          drp = 0;
          drpFrom = 0;
          drpTo = 0;
          drpAt = -999;
          setMap(false);
          rebounding = false;
          tile.group.position.y = 0;
        },
        onState(name, clock) {
          mode = "manual";
          drpFrom = drp;
          drpTo = name === "off" ? 0 : 1;
          drpAt = clock;
        },
        update(t, dt) {
          reboundClock = t;
          if (ctx.reduceMotion) {
            const on = mode === "manual" ? drpTo >= 0.5 : true;
            setMap(on);
            app = 1;
            drp = on ? 1 : 0;
            pix.mat.opacity = on ? 0 : 1;
            placePix();
            rescueG.position.y = LIFT * (1 - drp);
            tile.group.position.y = 0;
            return;
          }
          // clock-derived so the converge is deterministic (and freeze-verifiable)
          if (mode === "intro") {
            app = smooth(0.4, 1.3, t);
            drp = smooth(1.55, 2.05, t);
          } else {
            app = 1;
            drp = drpFrom + (drpTo - drpFrom) * easeInOutCubic(clamp01((t - drpAt) / 0.5));
          }

          setMap(drp > 0.9);
          pix.mat.opacity = app * (1 - smooth(0.8, 1, drp));
          placePix();
          rescueG.position.y = LIFT * (1 - drp) + Math.sin(t * 1.4) * 0.03 * (app * (1 - drp));

          if (rebounding) {
            reboundT += dt;
            const rp = clamp01(reboundT / 0.28);
            tile.group.position.y = -0.038 * Math.sin(rp * Math.PI);
            if (rp >= 1) {
              rebounding = false;
              tile.group.position.y = 0;
            }
          }
        },
      };
    },
  );

// ════════════════════════════════════════════════════════════════════
// 5. PROMISE — plates change, artwork never does (the iron law)
// ════════════════════════════════════════════════════════════════════
export const createPromiseScene: SceneInit<PromiseAssets> = (canvas, assets, opts) =>
  createStageHandle(
    canvas,
    opts,
    { fov: 32, minPitch: -0.12, maxPitch: 0.42, initialYaw: 0, initialPitch: 0.14,
      autoYaw: 0, breatheAmp: 0.015, orbitScale: 0.5, yawClamp: 0.5, enableDblReplay: true },
    (ctx) => {
      const S = 1.15;
      const GAP = 1.4;

      interface Col {
        beforeMat: THREE.MeshBasicMaterial;
        afterMat: THREE.MeshBasicMaterial;
        plateG: THREE.Group;
        artAnchor: THREE.Object3D;
        dir: number;
      }
      const cols: Col[] = assets.items.map((it, i) => {
        const colG = new THREE.Group();
        colG.position.x = (i - 1) * GAP;

        const card = ctx.card(S + 0.18, S + 0.18, 0.14);
        card.position.z = -0.06;
        card.renderOrder = 0;
        colG.add(card);

        const plateG = new THREE.Group();
        const beforeMat = ctx.basicTex(it.plateBefore);
        const before = ctx.plane(beforeMat, S, 1);
        const afterMat = ctx.basicTex(it.plateAfter);
        afterMat.opacity = 0;
        const after = ctx.plane(afterMat, S, 2);
        after.group.position.z = 0.005;
        plateG.add(before.group, after.group);
        colG.add(plateG);

        // art floats off the plate toward the viewer — never moves or changes
        const artMat = ctx.basicTex(it.art);
        const art = ctx.plane(artMat, S, 5);
        art.group.position.set(0, 0.02, 0.18);
        colG.add(art.group);
        const artAnchor = new THREE.Object3D();
        artAnchor.position.set(0, S * 0.5, 0);
        art.group.add(artAnchor);

        const csh = ctx.shadow(1.25);
        csh.position.y = -0.8;
        colG.add(csh);

        ctx.content.add(colG);
        return { beforeMat, afterMat, plateG, artAnchor, dir: i % 2 === 0 ? 1 : -1 };
      });

      let state: "before" | "after" = ctx.reduceMotion ? "after" : "before";
      let from: "before" | "after" = state;
      let changeAt = -999;
      let played = false;

      const applyMix = (mix: number, col: Col): void => {
        col.beforeMat.opacity = 1 - mix;
        col.afterMat.opacity = mix;
      };

      const half = GAP + (S + 0.18) / 2;
      const extent = new THREE.Box3(new THREE.Vector3(-half, -0.95, -0.25), new THREE.Vector3(half, 0.8, 0.3));

      return {
        extent,
        labels: cols.map((c, i) => ({ id: `a${i}`, anchor: c.artAnchor })),
        onReplay() {
          state = "before";
          from = "before";
          changeAt = -999;
          played = false;
          for (const c of cols) {
            applyMix(0, c);
            c.plateG.position.x = 0;
          }
        },
        onState(name, clock) {
          if ((name === "before" || name === "after") && name !== state) {
            from = state;
            state = name;
            changeAt = clock;
            played = true;
          }
        },
        update(t) {
          if (ctx.reduceMotion) {
            const mix = state === "after" ? 1 : 0;
            for (const c of cols) {
              applyMix(mix, c);
              c.plateG.position.x = 0;
            }
            return;
          }
          if (!played && t > 0.9) {
            from = "before";
            state = "after";
            changeAt = t;
            played = true;
          }
          const toV = state === "after" ? 1 : 0;
          const fromV = from === "after" ? 1 : 0;
          for (let i = 0; i < cols.length; i++) {
            const local = clamp01((t - changeAt - i * 0.12) / 0.6);
            const mix = fromV + (toV - fromV) * easeInOutCubic(local);
            applyMix(mix, cols[i]);
            // a plate steps aside and returns; the art above it stays put
            cols[i].plateG.position.x = Math.sin(local * Math.PI) * 0.14 * cols[i].dir;
          }
        },
      };
    },
  );
