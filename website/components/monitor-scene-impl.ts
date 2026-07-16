/**
 * The hero 3D scene: an aluminum all-in-one display (iMac-inspired, no logo)
 * whose screen plays the real product story. The camera opens on a wide 3/4
 * product view, then dollies smoothly INTO the screen until it nearly fills
 * the canvas — the desktop becomes the hero background — and only then the
 * coral scan wipe restyles it, dwells, and restores. Loaded lazily from
 * monitor-scene.tsx; this module never runs on the server.
 */
import * as THREE from "three";
import { RoundedBoxGeometry } from "three/examples/jsm/geometries/RoundedBoxGeometry.js";
import { RoomEnvironment } from "three/examples/jsm/environments/RoomEnvironment.js";

export interface MountOptions {
  before: string;
  after: string;
  onPhase?: (phase: "before" | "after") => void;
}

const CORAL = new THREE.Color("#ff6f5e");

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
  const settled = await Promise.allSettled([
    loader.loadAsync(opts.before),
    loader.loadAsync(opts.after),
  ]);
  if (settled[0].status === "fulfilled" && settled[1].status === "fulfilled") {
    texBefore = settled[0].value;
    texAfter = settled[1].value;
  } else {
    for (const s of settled) if (s.status === "fulfilled") s.value.dispose();
    throw new Error("monitor-scene: screen texture failed to load");
  }

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
  const envTex = pmrem.fromScene(new RoomEnvironment(), 0.04).texture;
  scene.environment = envTex;

  const camera = new THREE.PerspectiveCamera(30, 1, 0.1, 60);
  const halfTan = () => Math.tan(THREE.MathUtils.degToRad(camera.fov / 2));

  const key = new THREE.DirectionalLight(0xffffff, 0.55);
  key.position.set(3.5, 6, 4.5);
  scene.add(key);

  // ── the display: thin aluminum slab, white glass front, wedge stand ──
  const group = new THREE.Group();
  scene.add(group);

  const aluminum = new THREE.MeshPhysicalMaterial({
    color: 0xdfe3e7,
    metalness: 0.9,
    roughness: 0.32,
  });
  const glassWhite = new THREE.MeshPhysicalMaterial({
    color: 0xf5f6f8,
    metalness: 0,
    roughness: 0.22,
    clearcoat: 0.8,
    clearcoatRoughness: 0.2,
  });

  const slab = new THREE.Mesh(new RoundedBoxGeometry(3.36, 2.14, 0.07, 4, 0.035), aluminum);
  slab.position.set(0, 1.5, 0);
  group.add(slab);

  const glass = new THREE.Mesh(new RoundedBoxGeometry(3.3, 2.08, 0.02, 4, 0.01), glassWhite);
  glass.position.set(0, 1.5, 0.038);
  group.add(glass);

  const leg = new THREE.Mesh(new RoundedBoxGeometry(0.64, 0.56, 0.045, 4, 0.02), aluminum);
  leg.position.set(0, 0.22, -0.13);
  leg.rotation.x = 0.3;
  group.add(leg);

  const foot = new THREE.Mesh(new RoundedBoxGeometry(0.68, 0.025, 0.46, 4, 0.012), aluminum);
  foot.position.set(0, 0.0125, -0.02);
  group.add(foot);

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
        float sheen = (1.0 - smoothstep(0.0, 0.22, abs(vUv.x * 0.5 + vUv.y * 0.86 - 0.66))) * 0.03;
        c += sheen;
        gl_FragColor = vec4(c, 1.0);
        #include <colorspace_fragment>
      }
    `,
  });
  const screen = new THREE.Mesh(new THREE.PlaneGeometry(3.2, 1.8), screenMat);
  screen.position.set(0, 1.59, 0.0495);
  group.add(screen);

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

  // ── camera choreography ─────────────────────────────────────────
  const YAW_A = -0.42;
  const YAW_B = -0.05;
  const LOOK_A = new THREE.Vector3(0, 1.32, 0);
  // biased toward the icon-dense side of the desktop
  const LOOK_B = new THREE.Vector3(-0.25, 1.59, 0.05);
  const POS_A = new THREE.Vector3();
  const POS_B = new THREE.Vector3();
  const camPos = new THREE.Vector3();
  const camLook = new THREE.Vector3();

  const frameCamera = () => {
    // wide opener: whole product in frame whatever the host shape
    const zA = Math.max(8.8, 2.3 / (halfTan() * camera.aspect));
    POS_A.set(4.2, 2.6, zA);
    // close state: the glass fills ~96% of the view height
    const zB = 0.05 + 2.06 / 0.96 / (2 * halfTan());
    POS_B.set(LOOK_B.x + 0.1, LOOK_B.y, zB);
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
  const onPointer = (e: PointerEvent) => {
    const r = host.getBoundingClientRect();
    targetPanX = ((e.clientX - r.left) / r.width - 0.5) * 0.16;
    targetPanY = ((e.clientY - r.top) / r.height - 0.5) * -0.1;
  };
  const onLeave = () => {
    targetPanX = 0;
    targetPanY = 0;
  };
  host.addEventListener("pointermove", onPointer);
  host.addEventListener("pointerleave", onLeave);

  let raf = 0;
  let running = false;
  let visible = true;
  let pageVisible = !document.hidden;
  let start = 0;
  let last = 0;
  let firstFrame = true;
  let lastPhase: "before" | "after" = "before";

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
    const wipe = forcedWipe ?? wipeAt(wipeT);
    screenMat.uniforms.uWipe.value = wipe;
    const phase = wipe >= 0.5 ? "after" : "before";
    if (phase !== lastPhase) {
      lastPhase = phase;
      opts.onPhase?.(phase);
    }

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
    host.removeEventListener("pointermove", onPointer);
    host.removeEventListener("pointerleave", onLeave);
    for (const g of [slab, glass, leg, foot, screen, shadow]) g.geometry.dispose();
    for (const m of [aluminum, glassWhite, screenMat, shadowMat]) m.dispose();
    for (const x of [texBefore, texAfter, shadowTex, envTex]) x.dispose();
    pmrem.dispose();
    renderer.dispose();
    canvas.remove();
  };
}
