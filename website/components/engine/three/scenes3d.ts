/**
 * The five /engine/ three.js scenes, as one module. Each factory matches the
 * SceneInit contract exactly: it owns a private WebGLRenderer on the supplied
 * canvas, returns a SceneHandle, and lets the wrappers own all surrounding DOM.
 *
 * Design language: transparent canvas (no theme-bound backdrop); every artwork
 * layer floats on a frosted "glass card" (rounded, half-transparent white plane
 * with a hairline edge) so stacked same-colour layers still read as separate
 * sheets; depth comes from layout, a soft radial contact shadow, layer offset
 * and motion — never bloom/glow/particles/neon; entrances land hard (easeOutExpo
 * / ~3% overshoot back), idle motion is slow and small, reduceMotion renders the
 * end state once. All five share one Stage: renderer + orbit camera (pointer-drag
 * yaw, clamped pitch, inertia, slow auto-spin + breathing idle), a paint loop
 * that pauses offscreen / hidden, per-frame label projection, and full dispose.
 */
import * as THREE from "three";
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
const COLOR_HAIRLINE = 0xdfe3e8;

// ── easing / math ───────────────────────────────────────────────────
const clamp = (x: number, a: number, b: number): number => (x < a ? a : x > b ? b : x);
const clamp01 = (x: number): number => (x < 0 ? 0 : x > 1 ? 1 : x);
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

interface Card {
  group: THREE.Group;
  fillMat: THREE.MeshBasicMaterial;
  lineMat: THREE.LineBasicMaterial;
}

/** frosted glass card: rounded translucent fill + hairline outline, in the XY
 *  plane facing +Z (co-planar with the artwork it backs). */
function buildCard(
  pool: ResourcePool,
  w: number,
  h: number,
  r: number,
  color: THREE.ColorRepresentation,
  fillOpacity: number,
): Card {
  const shape = roundedRectShape(w, h, r);
  const fillMat = pool.track(new THREE.MeshBasicMaterial({
    color: new THREE.Color(color), transparent: true, opacity: fillOpacity,
    depthWrite: false, side: THREE.DoubleSide, toneMapped: false,
  }));
  const fill = new THREE.Mesh(pool.track(new THREE.ShapeGeometry(shape)), fillMat);
  const pts = shape.getPoints(48);
  const arr = new Float32Array(pts.length * 3);
  for (let i = 0; i < pts.length; i++) {
    arr[i * 3] = pts[i].x;
    arr[i * 3 + 1] = pts[i].y;
    arr[i * 3 + 2] = 0;
  }
  const lgeo = pool.track(new THREE.BufferGeometry());
  lgeo.setAttribute("position", new THREE.BufferAttribute(arr, 3));
  const lineMat = pool.track(
    new THREE.LineBasicMaterial({ color: COLOR_HAIRLINE, transparent: true, opacity: 0.9 }),
  );
  const line = new THREE.LineLoop(lgeo, lineMat);
  line.position.z = 0.001;
  const group = new THREE.Group();
  group.add(fill, line);
  return { group, fillMat, lineMat };
}

/** soft radial contact shadow, lying flat on the ground plane */
function buildShadow(pool: ResourcePool, size: number): THREE.Mesh {
  const c = document.createElement("canvas");
  c.width = c.height = 128;
  const g = c.getContext("2d");
  if (g) {
    const grad = g.createRadialGradient(64, 64, 4, 64, 64, 62);
    grad.addColorStop(0, "rgba(22,24,29,0.46)");
    grad.addColorStop(0.6, "rgba(22,24,29,0.15)");
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
  mesh.scale.set(1.25, 1, 1);
  mesh.renderOrder = -10;
  return mesh;
}

function setRenderOrder(obj: THREE.Object3D, ro: number): void {
  obj.traverse((o) => {
    o.renderOrder = ro;
  });
}

// ── the shared Stage ────────────────────────────────────────────────
interface BuildCtx {
  content: THREE.Group;
  pool: ResourcePool;
  reduceMotion: boolean;
  /** unit (1×1) plane facing +Z; scale per mesh */
  unitPlane: THREE.PlaneGeometry;
  tex(img: ImageData): THREE.CanvasTexture;
  /** MeshBasicMaterial pre-wired for a transparent artwork plane */
  basicTex(img: ImageData): THREE.MeshBasicMaterial;
  /** upright plane wearing `mat`, wrapped in an unscaled group (safe for anchors) */
  plane(mat: THREE.Material, size: number, order: number): { group: THREE.Group; mesh: THREE.Mesh };
  card(w: number, h: number, r: number): THREE.Group;
  cardEx(w: number, h: number, r: number, color: THREE.ColorRepresentation, opacity: number): Card;
  shadow(size: number): THREE.Mesh;
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
}

interface StageConfig {
  fov: number; distance: number; targetY: number;
  minPitch: number; maxPitch: number; initialYaw: number; initialPitch: number;
  autoYaw: number; breatheAmp: number; orbitScale: number; enableDblReplay: boolean;
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
  renderer.setClearColor(0x000000, 0);

  const scene = new THREE.Scene();
  const camera = new THREE.PerspectiveCamera(cfg.fov, 1, 0.1, 100);
  const target = new THREE.Vector3(0, cfg.targetY, 0);

  const content = new THREE.Group();
  scene.add(content);

  const pool = new ResourcePool();
  const maxAniso = renderer.capabilities.getMaxAnisotropy();
  const unitPlane = pool.track(new THREE.PlaneGeometry(1, 1));

  const tex = (img: ImageData): THREE.CanvasTexture => {
    const c = document.createElement("canvas");
    c.width = img.width;
    c.height = img.height;
    const g = c.getContext("2d");
    if (g) g.putImageData(img, 0, 0);
    const t = new THREE.CanvasTexture(c);
    t.colorSpace = THREE.SRGBColorSpace;
    t.anisotropy = Math.min(8, maxAniso);
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
  const cardEx = (w: number, h: number, r: number, color: THREE.ColorRepresentation, opacity: number) =>
    buildCard(pool, w, h, r, color, opacity);
  const card = (w: number, h: number, r: number): THREE.Group =>
    buildCard(pool, w, h, r, 0xffffff, 0.5).group;
  const shadow = (size: number): THREE.Mesh => buildShadow(pool, size);

  const logic = build({ content, pool, reduceMotion, unitPlane, tex, basicTex, plane, card, cardEx, shadow });

  // ── orbit state ───────────────────────────────────────────────────
  let yaw = cfg.initialYaw;
  let pitch = cfg.initialPitch;
  let yawVel = 0;
  let pitchVel = 0;
  let dragging = false;
  let lastX = 0;
  let lastY = 0;
  let lastMoveT = 0;
  const sens = 0.0075 * cfg.orbitScale;

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
    _cp.set(cp * Math.sin(yaw), Math.sin(pitch), cp * Math.cos(yaw))
      .multiplyScalar(cfg.distance)
      .add(target);
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
      // labels must never push the page wide: hide when far outside, and
      // clamp the anchor so the chip stays inside the canvas box
      const onCanvas =
        rawX >= -cssW * 0.12 && rawX <= cssW * 1.12 && rawY >= -cssH * 0.12 && rawY <= cssH * 1.12;
      const x = clamp(rawX, 10, Math.max(10, cssW - 112));
      const y = clamp(rawY, 10, Math.max(10, cssH - 26));
      const specVisible = spec.visible ? spec.visible() : true;
      const pt: LabelPoint = { x, y, visible: front && onCanvas && specVisible };
      onLabel(spec.id, pt);
    }
  };

  const orbitActive = (): boolean =>
    dragging || Math.abs(yawVel) > 0.02 || Math.abs(pitchVel) > 0.02;

  const renderFrame = (dt: number): void => {
    clock += dt;
    if (!dragging) {
      yaw += yawVel * dt;
      pitch = clamp(pitch + pitchVel * dt, cfg.minPitch, cfg.maxPitch);
      const decay = Math.exp(-3.2 * dt);
      yawVel *= decay;
      pitchVel *= decay;
      if (Math.abs(yawVel) < 0.02) yawVel = 0;
      if (Math.abs(pitchVel) < 0.02) pitchVel = 0;
      if (!reduceMotion && yawVel === 0) yaw += cfg.autoYaw * dt;
    }
    content.position.y = reduceMotion ? 0 : Math.sin(clock * 0.5) * cfg.breatheAmp;
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

  // ── sizing ────────────────────────────────────────────────────────
  const resize = (): void => {
    const w = canvas.clientWidth || 1;
    const h = canvas.clientHeight || 1;
    cssW = w;
    cssH = h;
    renderer.setSize(w, h, false);
    camera.aspect = w / h;
    camera.updateProjectionMatrix();
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
    // pointer capture is best-effort
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

  const dispose = (): void => {
    running = false;
    cancelAnimationFrame(raf);
    io.disconnect();
    ro.disconnect();
    document.removeEventListener("visibilitychange", onVis);
    canvas.removeEventListener("pointerdown", onDown);
    canvas.removeEventListener("pointermove", onMove);
    window.removeEventListener("pointerup", onUp);
    if (cfg.enableDblReplay) canvas.removeEventListener("dblclick", onDbl);
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
  if (!reduceMotion) wake();

  return { dispose, replay, setState };
}

// ── shared layer primitive (card + upright artwork + right-edge anchor) ──
interface Layer {
  group: THREE.Group;
  mesh: THREE.Mesh;
  mat: THREE.MeshBasicMaterial;
  right: THREE.Object3D;
}
function makeLayer(ctx: BuildCtx, img: ImageData, size: number, order: number): Layer {
  const group = new THREE.Group();
  const card = ctx.card(size + 0.22, size + 0.22, 0.16);
  card.position.z = -0.02;
  setRenderOrder(card, order);
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
// 1. HERO — the engine exploded diagram
// ════════════════════════════════════════════════════════════════════
export const createHeroScene: SceneInit<HeroAssets> = (canvas, assets, opts) =>
  createStageHandle(
    canvas,
    opts,
    { fov: 34, distance: 5.2, targetY: 0.02, minPitch: -0.15, maxPitch: 0.5,
      initialYaw: 0.5, initialPitch: 0.24, autoYaw: 0.09, breatheAmp: 0.03,
      orbitScale: 1, enableDblReplay: true },
    (ctx) => {
      const S = 1.5;
      const imgs = [assets.raw, assets.plate, assets.final];
      const layers = imgs.map((img, i) => {
        const L = makeLayer(ctx, img, S, i * 10);
        ctx.content.add(L.group);
        return L;
      });
      const sh = ctx.shadow(2.2);
      sh.position.y = -1.28;
      ctx.content.add(sh);
      const shMat = sh.material as THREE.MeshBasicMaterial;

      const GAP = 0.56;
      const tY = [-GAP, 0, GAP];
      const tZ = [-0.16, 0, 0.16];
      const ids = ["raw", "plate", "final"] as const;

      const HOLD = 0.4;
      const DUR = 0.62;
      const CDUR = 0.28; // replay collapse
      const RHOLD = 0.12;
      let mode: "intro" | "replay" = "intro";

      const explodeAt = (i: number, t: number): number => {
        if (ctx.reduceMotion) return 1;
        if (mode === "intro") {
          const start = HOLD + i * 0.045;
          return start <= t ? easeOutBack(clamp01((t - start) / DUR), 0.9) : 0;
        }
        if (t < CDUR) return 1 - easeInOutCubic(clamp01(t / CDUR));
        const start = CDUR + RHOLD + i * 0.045;
        return start <= t ? easeOutBack(clamp01((t - start) / DUR), 0.9) : 0;
      };

      return {
        labels: layers.map((L, i) => ({ id: ids[i], anchor: L.right })),
        onReplay() {
          mode = "replay";
        },
        update(t) {
          for (let i = 0; i < layers.length; i++) {
            const e = explodeAt(i, t);
            const floaty = ctx.reduceMotion ? 0 : Math.sin(t * 0.8 + i * 1.9) * 0.02;
            layers[i].group.position.set(0, tY[i] * e + floaty, tZ[i] * e);
            // a transient fan mid-burst; dead straight at rest
            layers[i].group.rotation.z = Math.sin(clamp01(e) * Math.PI) * 0.045 * (i - 1);
          }
          const eMid = explodeAt(1, t);
          shMat.opacity = 0.3 * (0.4 + 0.6 * clamp01(eMid));
        },
      };
    },
  );

// ════════════════════════════════════════════════════════════════════
// 2. READ — the checkup: scan sweep, then extracted readouts
// ════════════════════════════════════════════════════════════════════
export const createReadScene: SceneInit<ReadAssets> = (canvas, assets, opts) =>
  createStageHandle(
    canvas,
    opts,
    { fov: 32, distance: 3.7, targetY: 0, minPitch: -0.15, maxPitch: 0.5,
      initialYaw: 0.12, initialPitch: 0.16, autoYaw: 0, breatheAmp: 0.02,
      orbitScale: 1, enableDblReplay: true },
    (ctx) => {
      const S = 1.4;
      const SCAN = 1.1;
      const span = S + 0.24;

      // base tile
      const base = new THREE.Group();
      const baseCard = ctx.card(S + 0.22, S + 0.22, 0.16);
      baseCard.position.z = -0.02;
      setRenderOrder(baseCard, 0);
      base.add(baseCard);
      const iconMat = ctx.basicTex(assets.icon);
      const { group: iconG } = ctx.plane(iconMat, S, 2);
      base.add(iconG);
      ctx.content.add(base);

      const sh = ctx.shadow(2.0);
      sh.position.y = -1.05;
      ctx.content.add(sh);

      // sweeping coral scan bar (additive; brightens whatever it crosses)
      const barMat = ctx.pool.track(new THREE.MeshBasicMaterial({
        color: new THREE.Color(COLOR_CORAL), transparent: true, opacity: 0, depthWrite: false,
        toneMapped: false, blending: THREE.AdditiveBlending, side: THREE.DoubleSide,
      }));
      const bar = new THREE.Mesh(ctx.unitPlane, barMat);
      bar.scale.set(0.07, S + 0.26, 1);
      bar.position.z = 0.07;
      bar.renderOrder = 20;
      ctx.content.add(bar);

      // outline layer — lifts up + toward the viewer
      const outMat = ctx.basicTex(assets.outline);
      outMat.opacity = 0;
      const { group: outG } = ctx.plane(outMat, S, 14);
      ctx.content.add(outG);
      const outAnchor = new THREE.Object3D();
      outAnchor.position.set(S * 0.4, S * 0.32, 0);
      outG.add(outAnchor);

      // colour chip — flies from the icon to the upper right
      const chip = ctx.cardEx(0.42, 0.42, 0.1, assets.seedHex, 0);
      chip.lineMat.opacity = 0;
      setRenderOrder(chip.group, 16);
      const chipG = new THREE.Group();
      chipG.add(chip.group);
      ctx.content.add(chipG);
      const chipAnchor = new THREE.Object3D();
      chipAnchor.position.set(0.18, 0.15, 0);
      chipG.add(chipAnchor);
      const chipStart = new THREE.Vector3(0, 0, 0.05);
      // stays well inside the frustum (half-extent ≈0.93 at this distance)
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

      const terminal = (): void => {
        barMat.opacity = 0;
        iconMat.color.setScalar(1);
        outG.position.set(0, 0.42, 0.28);
        outMat.opacity = 1;
        chipG.position.copy(chipEnd);
        chip.fillMat.opacity = 1;
        chip.lineMat.opacity = 0.9;
        for (const t of ticks) {
          t.mat.opacity = 1;
          t.m.scale.setScalar(0.07);
        }
      };

      return {
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
          bar.position.x = -span / 2 + sc * span;
          barMat.opacity = scanning ? 0.55 : 0;
          iconMat.color.setScalar(
            scanning ? 1 + 0.5 * Math.exp(-Math.pow((sc - 0.5) / 0.22, 2)) : 1,
          );

          const pOut = easeOutExpo(clamp01((t - SCAN) / 0.5));
          const outIdle = Math.sin(t * 1.1) * 0.012 * pOut;
          outG.position.set(0, 0.42 * pOut + outIdle, 0.28 * pOut);
          outMat.opacity = pOut;

          const pCol = easeOutExpo(clamp01((t - (SCAN + 0.15)) / 0.5));
          chipG.position.lerpVectors(chipStart, chipEnd, pCol);
          chipG.position.y += Math.sin(t * 1.0 + 1) * 0.012 * pCol;
          chip.fillMat.opacity = pCol;
          chip.lineMat.opacity = 0.9 * pCol;

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
// 3. CUT — the background peels away as its own layer
// ════════════════════════════════════════════════════════════════════
export const createCutScene: SceneInit<CutAssets> = (canvas, assets, opts) =>
  createStageHandle(
    canvas,
    opts,
    { fov: 32, distance: 3.7, targetY: 0, minPitch: -0.15, maxPitch: 0.5,
      initialYaw: 0.14, initialPitch: 0.16, autoYaw: 0, breatheAmp: 0.02,
      orbitScale: 1, enableDblReplay: true },
    (ctx) => {
      const S = 1.42;
      const WHITE = new THREE.Color(1, 1, 1);
      const CORALc = new THREE.Color(COLOR_CORAL);

      const baseCard = ctx.card(S + 0.22, S + 0.22, 0.16);
      baseCard.position.z = -0.04;
      setRenderOrder(baseCard, 0);
      ctx.content.add(baseCard);

      const sh = ctx.shadow(2.0);
      sh.position.y = -1.05;
      ctx.content.add(sh);

      const bgMat = ctx.basicTex(assets.bgLayer);
      const bg = ctx.plane(bgMat, S, 1);
      bg.group.position.z = 0;
      ctx.content.add(bg.group);
      const bgAnchor = new THREE.Object3D();
      bgAnchor.position.set(S * 0.4, S * 0.35, 0);
      bg.group.add(bgAnchor);

      const artMat = ctx.basicTex(assets.artLayer);
      const art = ctx.plane(artMat, S, 3);
      art.group.position.z = 0.02;
      ctx.content.add(art.group);
      const artAnchor = new THREE.Object3D();
      artAnchor.position.set(S * 0.4, -S * 0.15, 0);
      art.group.add(artAnchor);

      const finalMat = ctx.basicTex(assets.final);
      finalMat.opacity = 0;
      const fin = ctx.plane(finalMat, S, 4);
      fin.group.position.set(0, -0.9, 0.03);
      ctx.content.add(fin.group);
      const finAnchor = new THREE.Object3D();
      finAnchor.position.set(S * 0.4, 0, 0);
      fin.group.add(finAnchor);

      const HOLD = 0.3;
      const terminal = (): void => {
        bgMat.color.copy(CORALc);
        bgMat.opacity = 0;
        bg.group.position.set(-0.55, -0.6, -0.25);
        artMat.opacity = 0;
        finalMat.opacity = 1;
        fin.group.position.set(0, 0, 0.03);
      };

      return {
        labels: [
          { id: "bg", anchor: bgAnchor, visible: () => bgMat.opacity > 0.05 },
          { id: "art", anchor: artAnchor, visible: () => artMat.opacity > 0.05 },
          { id: "final", anchor: finAnchor, visible: () => finalMat.opacity > 0.05 },
        ],
        onReplay() {
          /* clock=0 replays the peel */
        },
        update(t) {
          if (ctx.reduceMotion) {
            terminal();
            return;
          }
          // recognition: the base blinks twice, then commits to coral
          const blinkP = clamp01((t - HOLD) / 0.36);
          const blink = blinkP > 0 && blinkP < 1 ? Math.abs(Math.sin(blinkP * Math.PI * 2)) * 0.6 : 0;
          const t1 = clamp01((t - (HOLD + 0.36)) / 0.4);
          bgMat.color.copy(WHITE).lerp(CORALc, Math.max(t1, blink));

          const t2 = easeOutCubic(clamp01((t - (HOLD + 0.86)) / 0.6));
          bg.group.position.set(-0.55 * t2, -0.6 * t2, -0.25 * t2);
          bgMat.opacity = 1 - t2;

          const t3 = easeOutExpo(clamp01((t - (HOLD + 1.06)) / 0.6));
          fin.group.position.y = -0.9 + 0.9 * t3;
          finalMat.opacity = t3;
          artMat.opacity = 1 - t3;
        },
      };
    },
  );

// ════════════════════════════════════════════════════════════════════
// 4. RESCUE — the exact rescue pixels, as a separable layer
// ════════════════════════════════════════════════════════════════════
export const createRescueScene: SceneInit<RescueAssets> = (canvas, assets, opts) =>
  createStageHandle(
    canvas,
    opts,
    { fov: 32, distance: 3.75, targetY: 0.05, minPitch: -0.15, maxPitch: 0.5,
      initialYaw: 0.16, initialPitch: 0.18, autoYaw: 0, breatheAmp: 0.02,
      orbitScale: 1, enableDblReplay: true },
    (ctx) => {
      const S = 1.42;
      const LIFT = 0.5;

      const baseCard = ctx.card(S + 0.22, S + 0.22, 0.16);
      baseCard.position.z = -0.03;
      setRenderOrder(baseCard, 0);
      ctx.content.add(baseCard);

      const sh = ctx.shadow(2.0);
      sh.position.y = -1.05;
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

      const rescueMat = ctx.basicTex(assets.rescueLayer);
      rescueMat.opacity = 0;
      const rescue = ctx.plane(rescueMat, S, 5);
      rescue.group.position.set(0, LIFT, 0.1);
      ctx.content.add(rescue.group);
      const rescueAnchor = new THREE.Object3D();
      rescueAnchor.position.set(S * 0.4, S * 0.2, 0);
      rescue.group.add(rescueAnchor);

      // app: rescue appears/floats (0→1). drp: rescue drops & merges (0→1).
      let app = 0;
      let drp = 0;
      let appTarget = 0;
      let drpTarget = 0;
      let mode: "intro" | "manual" = "intro";
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

      return {
        labels: [
          { id: "tile", anchor: tileAnchor },
          { id: "rescue", anchor: rescueAnchor, visible: () => rescueMat.opacity > 0.05 },
        ],
        onReplay() {
          mode = "intro";
          app = 0;
          drp = 0;
          appTarget = 0;
          drpTarget = 0;
          setMap(false);
          rebounding = false;
          tile.group.position.y = 0;
        },
        onState(name) {
          mode = "manual";
          if (name === "off") {
            appTarget = 1;
            drpTarget = 0;
          } else {
            appTarget = 1;
            drpTarget = 1;
          }
        },
        update(t, dt) {
          if (ctx.reduceMotion) {
            const on = mode === "manual" ? drpTarget >= 0.5 : true;
            setMap(on);
            rescueMat.opacity = on ? 0 : 1;
            rescue.group.position.y = on ? 0 : LIFT;
            tile.group.position.y = 0;
            return;
          }
          if (mode === "intro") {
            appTarget = t > 0.4 ? 1 : 0;
            drpTarget = t > 1.05 ? 1 : 0;
          }
          app += (appTarget - app) * (1 - Math.exp(-6 * dt));
          drp += (drpTarget - drp) * (1 - Math.exp(-7 * dt));

          setMap(drp > 0.9);
          rescueMat.opacity = app * (1 - clamp01((drp - 0.8) / 0.2));
          const bob = Math.sin(t * 1.4) * 0.03 * (app * (1 - drp));
          rescue.group.position.y = LIFT * (1 - drp) + bob;

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
    { fov: 32, distance: 6.0, targetY: 0, minPitch: -0.12, maxPitch: 0.42,
      initialYaw: 0, initialPitch: 0.14, autoYaw: 0.04, breatheAmp: 0.015,
      orbitScale: 0.5, enableDblReplay: true },
    (ctx) => {
      const S = 1.2;
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
        card.position.z = -0.03;
        setRenderOrder(card, 0);
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
        // grounding shadow so each floating tile reads as standing on a surface
        const csh = ctx.shadow(1.3);
        csh.position.y = -0.82;
        colG.add(csh);
        const artAnchor = new THREE.Object3D();
        artAnchor.position.set(0, S * 0.5, 0);
        art.group.add(artAnchor);

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

      return {
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
