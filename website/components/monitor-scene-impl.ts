/**
 * The hero 3D scene: an aluminum all-in-one display (iMac-inspired, no logo)
 * whose screen plays the real product story. The camera opens on a wide 3/4
 * product view, then dollies smoothly INTO the screen until it nearly fills
 * the canvas — the desktop becomes the hero background — and only then the
 * coral scan wipe restyles it, dwells, and restores. Loaded lazily from
 * monitor-scene.tsx; this module never runs on the server.
 */
import * as THREE from "three";
import { RGBELoader } from "three/examples/jsm/loaders/RGBELoader.js";
import { GLTFLoader, type GLTF } from "three/examples/jsm/loaders/GLTFLoader.js";

export interface MountOptions {
  before: string;
  after: string;
}

const CORAL = new THREE.Color("#ff6f5e");
/** production HDRI yaw — tuned so the key light rakes across the front glass */
const ENV_ROT_DEG = 90;

/** easeInOutQuint — the silky dolly */
const inOutQuint = (x: number) =>
  x < 0.5 ? 16 * x * x * x * x * x : 1 - Math.pow(-2 * x + 2, 5) / 2;
/** easeInOutCubic */
const inOutCubic = (x: number) => (x < 0.5 ? 4 * x * x * x : 1 - Math.pow(-2 * x + 2, 3) / 2);

/** Dolly: hold wide, then fly to the screen. */
const DOLLY_DELAY = 0.25;
const DOLLY_DUR = 2.45;
/** Wipe starts once the screen owns the frame. */
const WIPE_DELAY = DOLLY_DELAY + DOLLY_DUR + 0.35;

/** Wipe timeline: hold before, scan to after, dwell, restore, breathe. */
const TIMELINE = [
  { dur: 1.0, from: 0, to: 0 },
  { dur: 1.35, from: 0, to: 1 },
  { dur: 4.2, from: 1, to: 1 },
  { dur: 1.1, from: 1, to: 0 },
  { dur: 2.0, from: 0, to: 0 },
] as const;
const TIMELINE_TOTAL = TIMELINE.reduce((s, p) => s + p.dur, 0);

function wipeAt(t: number): number {
  let local = t % TIMELINE_TOTAL;
  for (const phase of TIMELINE) {
    if (local < phase.dur) {
      const x = local / phase.dur;
      return phase.from + (phase.to - phase.from) * inOutCubic(x);
    }
    local -= phase.dur;
  }
  return 0;
}

/**
 * Product-shot studio for the IBL: a mid-gray room with one big softbox
 * upper-left, a tall rim strip on the right and a low front fill. The long
 * rectangular highlights these paint on aluminum edges and glass are what
 * separates a rendered product from a toy.
 */
function studioEnvironment(): THREE.Scene {
  const s = new THREE.Scene();
  const room = new THREE.Mesh(
    new THREE.BoxGeometry(30, 30, 30),
    new THREE.MeshBasicMaterial({ color: 0x62666d, side: THREE.BackSide }),
  );
  s.add(room);
  const floor = new THREE.Mesh(
    new THREE.PlaneGeometry(30, 30),
    new THREE.MeshBasicMaterial({ color: 0x2e3136 }),
  );
  floor.rotation.x = -Math.PI / 2;
  floor.position.y = -14.9;
  s.add(floor);

  const softbox = (w: number, h: number, intensity: number, pos: [number, number, number]) => {
    const m = new THREE.Mesh(
      new THREE.PlaneGeometry(w, h),
      new THREE.MeshBasicMaterial({ color: new THREE.Color().setScalar(intensity) }),
    );
    m.position.set(...pos);
    m.lookAt(0, 0, 0);
    s.add(m);
  };
  softbox(13, 9, 5.5, [-9, 10, 7]); // key: big and high, upper-left front
  softbox(2.2, 15, 9, [13, 3, -5]); // rim: tall thin strip, right rear
  softbox(9, 4, 1.6, [3, 1, 14]); // fill: low front
  softbox(6, 6, 2.2, [0, 14.5, 0]); // top
  return s;
}

function contactShadowTexture(): THREE.CanvasTexture {
  const c = document.createElement("canvas");
  c.width = 256;
  c.height = 128;
  const g = c.getContext("2d")!;
  const grad = g.createRadialGradient(128, 64, 8, 128, 64, 120);
  grad.addColorStop(0, "rgba(22,24,29,0.30)");
  grad.addColorStop(0.55, "rgba(22,24,29,0.10)");
  grad.addColorStop(1, "rgba(22,24,29,0)");
  g.scale(1, 0.5);
  g.translate(0, 64);
  g.fillStyle = grad;
  g.fillRect(0, 0, 256, 256);
  return new THREE.CanvasTexture(c);
}

export async function mount(host: HTMLElement, opts: MountOptions): Promise<() => void> {
  // Load textures BEFORE creating any GL resource: a rejected fetch leaks
  // nothing, and an unmount-during-load has nothing to tear down yet.
  const loader = new THREE.TextureLoader();
  let texBefore: THREE.Texture;
  let texAfter: THREE.Texture;
  // design-review hook: ?dm3drot=<deg> spins the HDRI for lighting tuning
  const params = new URLSearchParams(window.location.search);
  // real studio HDRI (Poly Haven, CC0) — the same lighting the good Apple-style
  // web renders use; the procedural softbox scene stays as a fallback
  const settled = await Promise.allSettled([
    loader.loadAsync(opts.before),
    loader.loadAsync(opts.after),
    new RGBELoader().loadAsync("/img/studio.hdr"),
    new GLTFLoader().loadAsync("/img/studio-display.glb"),
  ] as const);
  const disposeSettled = () => {
    if (settled[0].status === "fulfilled") settled[0].value.dispose();
    if (settled[1].status === "fulfilled") settled[1].value.dispose();
    if (settled[2].status === "fulfilled") settled[2].value.dispose();
  };
  if (settled[0].status === "fulfilled" && settled[1].status === "fulfilled") {
    texBefore = settled[0].value;
    texAfter = settled[1].value;
  } else {
    disposeSettled();
    throw new Error("monitor-scene: screen texture failed to load");
  }
  if (settled[3].status !== "fulfilled") {
    disposeSettled();
    throw new Error("monitor-scene: device model failed to load");
  }
  const gltfModel: GLTF = settled[3].value;
  const hdrTex = settled[2].status === "fulfilled" ? settled[2].value : null;

  const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
  renderer.outputColorSpace = THREE.SRGBColorSpace;
  renderer.toneMapping = THREE.ACESFilmicToneMapping;
  renderer.toneMappingExposure = 1.05;
  const canvas = renderer.domElement;
  canvas.style.cssText = "position:absolute;inset:0;width:100%;height:100%;opacity:0;transition:opacity .7s ease";
  host.appendChild(canvas);

  const scene = new THREE.Scene();
  const pmrem = new THREE.PMREMGenerator(renderer);
  let envTarget: THREE.WebGLRenderTarget;
  if (hdrTex) {
    hdrTex.mapping = THREE.EquirectangularReflectionMapping;
    envTarget = pmrem.fromEquirectangular(hdrTex);
    hdrTex.dispose();
  } else {
    const envScene = studioEnvironment();
    envTarget = pmrem.fromScene(envScene, 0.02);
    envScene.traverse((o) => {
      if (o instanceof THREE.Mesh) {
        o.geometry.dispose();
        (o.material as THREE.Material).dispose();
      }
    });
  }
  scene.environment = envTarget.texture;
  const envRotDeg = parseFloat(params.get("dm3drot") ?? "");
  scene.environmentRotation.set(0, THREE.MathUtils.degToRad(Number.isFinite(envRotDeg) ? envRotDeg : ENV_ROT_DEG), 0);

  const camera = new THREE.PerspectiveCamera(30, 1, 0.1, 60);
  const halfTan = () => Math.tan(THREE.MathUtils.degToRad(camera.fov / 2));

  const key = new THREE.DirectionalLight(0xffffff, 0.55);
  key.position.set(3.5, 6, 4.5);
  scene.add(key);

  // ── the display: a real Apple Studio Display scan ─────────────────
  // "Apple Studio Display" by alboxer2000_ (sketchfab.com/alboxer2000_),
  // CC-BY-4.0 — attribution lives in the site footer.
  const group = new THREE.Group();
  scene.add(group);

  const model = gltfModel.scene;
  {
    // normalize: face width → the world units the choreography expects,
    // feet on y=0, depth centered for clean yaw
    const raw = new THREE.Box3().setFromObject(model);
    model.scale.setScalar(3.32 / (raw.max.x - raw.min.x));
    model.updateMatrixWorld(true);
    const scaled = new THREE.Box3().setFromObject(model);
    model.position.y -= scaled.min.y;
    model.position.z -= (scaled.min.z + scaled.max.z) / 2;
    model.updateMatrixWorld(true);
  }
  group.add(model);

  // the illuminated panel inside the GLB (emissive-white material) marks
  // where OUR screen shader goes — take the LARGEST such surface, the model
  // also carries small emissive bits (LED, port glow)
  let panel: THREE.Mesh | undefined;
  let panelArea = 0;
  const candBox = new THREE.Box3();
  const candSize = new THREE.Vector3();
  model.traverse((o) => {
    const m = o as THREE.Mesh;
    const mat = m.material as THREE.MeshStandardMaterial | undefined;
    if (!m.isMesh || !mat?.emissive || mat.emissive.getHex() !== 0xffffff) return;
    candBox.setFromObject(m).getSize(candSize);
    const area = candSize.x * candSize.y;
    if (area > panelArea) {
      panelArea = area;
      panel = m;
    }
  });
  if (!panel) throw new Error("monitor-scene: screen panel not found in device model");
  const screenPanel: THREE.Mesh = panel;
  const panelBox = new THREE.Box3().setFromObject(screenPanel);
  const PANEL_W = panelBox.max.x - panelBox.min.x;
  const PANEL_H = panelBox.max.y - panelBox.min.y;
  const PANEL_C = panelBox.getCenter(new THREE.Vector3());
  const PANEL_Z = panelBox.max.z;

  const maxAniso = renderer.capabilities.getMaxAnisotropy();
  for (const t of [texBefore, texAfter]) {
    t.colorSpace = THREE.SRGBColorSpace;
    t.anisotropy = Math.min(8, maxAniso);
    t.minFilter = THREE.LinearMipmapLinearFilter;
  }

  const screenMat = new THREE.ShaderMaterial({
    uniforms: {
      uBefore: { value: texBefore },
      uAfter: { value: texAfter },
      uWipe: { value: 0 },
      uCoral: { value: CORAL },
    },
    vertexShader: /* glsl */ `
      varying vec2 vUv;
      void main() {
        vUv = uv;
        gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
      }
    `,
    fragmentShader: /* glsl */ `
      uniform sampler2D uBefore;
      uniform sampler2D uAfter;
      uniform float uWipe;
      uniform vec3 uCoral;
      varying vec2 vUv;
      void main() {
        vec3 beforeC = texture2D(uBefore, vUv).rgb;
        vec3 afterC = texture2D(uAfter, vUv).rgb;
        float m = smoothstep(uWipe - 0.0025, uWipe + 0.0025, vUv.x);
        vec3 c = mix(afterC, beforeC, m);
        float scanning = step(0.0005, uWipe) * step(uWipe, 0.9995);
        float line = 1.0 - smoothstep(0.0, 0.004, abs(vUv.x - uWipe));
        c = mix(c, uCoral, line * scanning);
        gl_FragColor = vec4(c, 1.0);
        #include <colorspace_fragment>
      }
    `,
  });
  // The panel mesh's OWN geometry hosts the shader — flush by definition,
  // tilt included. Its UVs are rebuilt from the geometry data: the thin local
  // axis is the normal; of the two in-plane axes, the more world-vertical one
  // becomes v, oriented so u+ runs world-right and v=1 is the top.
  {
    const geo = screenPanel.geometry as THREE.BufferGeometry;
    geo.computeBoundingBox();
    const bb = geo.boundingBox!;
    const ext = [bb.max.x - bb.min.x, bb.max.y - bb.min.y, bb.max.z - bb.min.z];
    const mins = [bb.min.x, bb.min.y, bb.min.z];
    const thin = ext.indexOf(Math.min(...ext));
    const inPlane = [0, 1, 2].filter((a) => a !== thin);
    const worldDir = (axis: number) => {
      const v = new THREE.Vector3();
      v.setComponent(axis, 1);
      return v.transformDirection(screenPanel.matrixWorld);
    };
    const d0 = worldDir(inPlane[0]);
    const d1 = worldDir(inPlane[1]);
    const [uAx, vAx] =
      Math.abs(d0.y) > Math.abs(d1.y) ? [inPlane[1], inPlane[0]] : [inPlane[0], inPlane[1]];
    const uFlip = worldDir(uAx).x < 0;
    const vFlip = worldDir(vAx).y < 0;
    const pos = geo.getAttribute("position");
    const uv = new Float32Array(pos.count * 2);
    const p = new THREE.Vector3();
    for (let i = 0; i < pos.count; i++) {
      p.set(pos.getX(i), pos.getY(i), pos.getZ(i));
      let u = (p.getComponent(uAx) - mins[uAx]) / ext[uAx];
      let v = (p.getComponent(vAx) - mins[vAx]) / ext[vAx];
      if (uFlip) u = 1 - u;
      if (vFlip) v = 1 - v;
      uv[2 * i] = u;
      uv[2 * i + 1] = v;
    }
    geo.setAttribute("uv", new THREE.BufferAttribute(uv, 2));
    screenMat.side = THREE.DoubleSide;
    (screenPanel.material as THREE.Material).dispose();
    screenPanel.material = screenMat;
    screenPanel.visible = true;
  }

  // the original panel material carried the glass reflection the shader
  // swap removed — reinstate it as a coincident clone of the panel mesh
  // wearing near-mirror black glass that only contributes env streaks
  const glassOverlayMat = new THREE.MeshPhysicalMaterial({
    color: 0x000000,
    transparent: true,
    opacity: 0.06,
    roughness: 0.04,
    metalness: 0,
    envMapIntensity: 1.5,
    depthWrite: false,
    polygonOffset: true,
    polygonOffsetFactor: -2,
    side: THREE.DoubleSide,
  });
  const glassOverlay = screenPanel.clone();
  glassOverlay.material = glassOverlayMat;
  glassOverlay.renderOrder = 10;
  screenPanel.parent?.add(glassOverlay);

  const shadowTex = contactShadowTexture();
  const shadowMat = new THREE.MeshBasicMaterial({
    map: shadowTex,
    transparent: true,
    depthWrite: false,
  });
  const shadow = new THREE.Mesh(new THREE.PlaneGeometry(4.2, 2.1), shadowMat);
  shadow.rotation.x = -Math.PI / 2;
  shadow.position.y = 0.001;
  scene.add(shadow);

  // ── camera choreography: the machine simply TURNS — opening on the
  // RIGHT side's thickness (edge-on), swinging through frontal, settling
  // where the LEFT bezel width faces the lens — with a slight push-in.
  // The whole screen content stays inside the viewport once settled.
  const YAW_A = 0.45;
  const YAW_B = -0.58;
  const LOOK_A = new THREE.Vector3();
  const LOOK_B = new THREE.Vector3();
  const POS_A = new THREE.Vector3();
  const POS_B = new THREE.Vector3();
  const camPos = new THREE.Vector3();
  const camLook = new THREE.Vector3();

  // hosts can tune the composition: --dm-screen-cy places the screen's center
  // as a fraction of canvas height (hero extends the canvas below the fold so
  // the machine's body is never cropped); --dm-fill sets glass height fill.
  const cssNum = (name: string, fallback: number) => {
    const v = parseFloat(getComputedStyle(host).getPropertyValue(name));
    return Number.isFinite(v) ? v : fallback;
  };

  const frameCamera = () => {
    // One composition, two distances: the end state fits the ENTIRE panel
    // inside the canvas (--dm-fill of its height), parked on the right of a
    // wide stage / centered on a narrow one; the opener is the same framing
    // pulled back ~12% so the entrance reads as a turn with a slight push-in.
    const ht = halfTan();
    const a = camera.aspect;
    const isWideStage = a > 1.4;
    const cy = cssNum("--dm-screen-cy", 0.5);
    const fillH = cssNum("--dm-fill", isWideStage ? 0.62 : 0.8);
    // never crop the panel horizontally: back off to whichever fit is farther
    // (1.25 covers the yaw-projected near edge plus the bezel)
    const vhB = Math.max(PANEL_H / fillH, (PANEL_W * 1.35) / (0.96 * a));
    const vwB = vhB * a;
    const zB = PANEL_Z + vhB / (2 * ht);
    const oyB = (0.5 - cy) * vhB;
    // machine center rendered at 66% of a wide stage, centered otherwise
    const ox = isWideStage ? (0.72 - 0.5) * vwB : 0;
    LOOK_B.set(PANEL_C.x - ox, PANEL_C.y - oyB, PANEL_Z);
    POS_B.set(LOOK_B.x, LOOK_B.y, zB);
    LOOK_A.copy(LOOK_B);
    POS_A.set(POS_B.x, POS_B.y, PANEL_Z + (zB - PANEL_Z) * 1.12);
  };
  frameCamera();

  const resize = () => {
    const w = host.clientWidth;
    const h = host.clientHeight;
    if (w === 0 || h === 0) return;
    renderer.setSize(w, h, false);
    camera.aspect = w / h;
    frameCamera();
    camera.updateProjectionMatrix();
  };
  resize();
  const ro = new ResizeObserver(resize);
  ro.observe(host);

  // ── interaction + loop ──────────────────────────────────────────
  let targetPanX = 0;
  let targetPanY = 0;
  let panX = 0;
  let panY = 0;
  // window-level: the copy floats above the canvas, so host-level events
  // would go dead over half the stage
  const onPointer = (e: PointerEvent) => {
    const r = host.getBoundingClientRect();
    if (r.width === 0) return;
    targetPanX = ((e.clientX - r.left) / r.width - 0.5) * 0.16;
    targetPanY = ((e.clientY - r.top) / r.height - 0.5) * -0.1;
  };
  const onLeave = () => {
    targetPanX = 0;
    targetPanY = 0;
  };
  window.addEventListener("pointermove", onPointer, { passive: true });
  document.documentElement.addEventListener("pointerleave", onLeave);

  let raf = 0;
  let running = false;
  let visible = true;
  let pageVisible = !document.hidden;
  let start = 0;
  let last = 0;
  let firstFrame = true;

  // deterministic states for visual acceptance shots: ?dm3d=before|after|scan|wide
  const forced = new URLSearchParams(window.location.search).get("dm3d");
  const forcedWipe = forced === "after" ? 1 : forced === "scan" ? 0.46 : forced === "before" ? 0 : null;
  const forcedS = forced === "wide" ? 0 : forcedWipe !== null ? 1 : null;

  const frame = (now: number) => {
    raf = requestAnimationFrame(frame);
    if (start === 0) {
      start = now;
      last = now;
    }
    const dt = Math.min((now - last) / 1000, 0.05);
    last = now;
    const t = (now - start) / 1000;

    // dolly progress
    const s = forcedS ?? inOutQuint(Math.min(Math.max((t - DOLLY_DELAY) / DOLLY_DUR, 0), 1));

    panX += (targetPanX - panX) * (1 - Math.exp(-5 * dt));
    panY += (targetPanY - panY) * (1 - Math.exp(-5 * dt));

    camPos.lerpVectors(POS_A, POS_B, s);
    camLook.lerpVectors(LOOK_A, LOOK_B, s);
    // gentle breathing after arrival + pointer peek, both fade in with s
    const idle = Math.sin(t * 0.4) * 0.02 * s;
    camera.position.set(camPos.x + panX * s, camPos.y + panY * s, camPos.z + idle);
    camera.lookAt(camLook);

    group.rotation.y = YAW_A + (YAW_B - YAW_A) * s + panX * 0.12 * s;
    group.position.y = Math.sin(t * 0.8) * 0.01 * (1 - s * 0.75);

    const wipeT = forcedWipe !== null ? 0 : Math.max(t - WIPE_DELAY, 0);
    screenMat.uniforms.uWipe.value = forcedWipe ?? wipeAt(wipeT);

    renderer.render(scene, camera);
    if (firstFrame) {
      firstFrame = false;
      canvas.style.opacity = "1";
    }
  };

  let pausedAt = 0;
  const setRunning = () => {
    const should = visible && pageVisible;
    if (should && !running) {
      running = true;
      raf = requestAnimationFrame((now) => {
        // freeze the timeline across the pause instead of jumping forward
        if (pausedAt && start) start += now - pausedAt;
        pausedAt = 0;
        last = now;
        frame(now);
      });
    } else if (!should && running) {
      running = false;
      pausedAt = performance.now();
      cancelAnimationFrame(raf);
    }
  };

  const io = new IntersectionObserver(
    (records) => {
      visible = records.some((r) => r.isIntersecting);
      setRunning();
    },
    { threshold: 0.05 },
  );
  io.observe(host);

  const onVis = () => {
    pageVisible = !document.hidden;
    setRunning();
  };
  document.addEventListener("visibilitychange", onVis);

  setRunning();

  return () => {
    running = false;
    cancelAnimationFrame(raf);
    io.disconnect();
    ro.disconnect();
    document.removeEventListener("visibilitychange", onVis);
    window.removeEventListener("pointermove", onPointer);
    document.documentElement.removeEventListener("pointerleave", onLeave);
    model.traverse((o) => {
      const m = o as THREE.Mesh;
      if (!m.isMesh) return;
      m.geometry.dispose();
      for (const mat of Array.isArray(m.material) ? m.material : [m.material]) {
        for (const v of Object.values(mat)) if (v instanceof THREE.Texture) v.dispose();
        mat.dispose();
      }
    });
    shadow.geometry.dispose();
    for (const m of [screenMat, shadowMat, glassOverlayMat]) m.dispose();
    for (const x of [texBefore, texAfter, shadowTex]) x.dispose();
    envTarget.dispose();
    pmrem.dispose();
    renderer.dispose();
    canvas.remove();
  };
}
