/**
 * The hero 3D scene: a physically-shaded desktop monitor whose screen plays
 * the real product story — the actual before render, a scan wipe to the
 * styled render, then a restore back. Loaded lazily from monitor-scene.tsx;
 * this module never runs on the server.
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

/** easeOutQuint */
const outQuint = (x: number) => 1 - Math.pow(1 - x, 5);
/** easeInOutCubic */
const inOutCubic = (x: number) => (x < 0.5 ? 4 * x * x * x : 1 - Math.pow(-2 * x + 2, 3) / 2);

/** Wipe timeline: hold before, scan to after, dwell, restore, breathe. */
const TIMELINE = [
  { dur: 1.3, from: 0, to: 0 },
  { dur: 1.15, from: 0, to: 1 },
  { dur: 3.6, from: 1, to: 1 },
  { dur: 0.95, from: 1, to: 0 },
  { dur: 1.7, from: 0, to: 0 },
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
  grad.addColorStop(0, "rgba(22,24,29,0.32)");
  grad.addColorStop(0.55, "rgba(22,24,29,0.10)");
  grad.addColorStop(1, "rgba(22,24,29,0)");
  g.scale(1, 0.5);
  g.translate(0, 64);
  g.fillStyle = grad;
  g.fillRect(0, 0, 256, 256);
  return new THREE.CanvasTexture(c);
}

export async function mount(host: HTMLElement, opts: MountOptions): Promise<() => void> {
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

  const camera = new THREE.PerspectiveCamera(30, 1, 0.1, 40);
  const frameCamera = () => {
    // keep the rotated monitor (~3.6 world units wide) inside narrow hosts,
    // and lift it in frame when the host is portrait-ish
    const halfTan = Math.tan(THREE.MathUtils.degToRad(camera.fov / 2));
    const z = Math.max(6.7, 2.0 / (halfTan * camera.aspect));
    const ty = camera.aspect < 1 ? 1.6 : 1.35;
    camera.position.set(2.7, ty + 0.45, z);
    camera.lookAt(0, ty, 0);
  };
  frameCamera();

  const key = new THREE.DirectionalLight(0xffffff, 0.55);
  key.position.set(3.5, 6, 4.5);
  scene.add(key);

  // ── the monitor ─────────────────────────────────────────────────
  const group = new THREE.Group();
  scene.add(group);

  const graphite = new THREE.MeshPhysicalMaterial({
    color: 0x23262c,
    metalness: 0.55,
    roughness: 0.38,
    clearcoat: 0.35,
    clearcoatRoughness: 0.25,
  });
  const aluminum = new THREE.MeshPhysicalMaterial({
    color: 0xd9dce0,
    metalness: 0.95,
    roughness: 0.3,
  });

  const foot = new THREE.Mesh(new RoundedBoxGeometry(1.15, 0.05, 0.66, 4, 0.02), aluminum);
  foot.position.set(0, 0.025, 0.02);
  group.add(foot);

  const neck = new THREE.Mesh(new RoundedBoxGeometry(0.16, 0.82, 0.055, 4, 0.02), aluminum);
  neck.position.set(0, 0.44, -0.075);
  group.add(neck);

  const slab = new THREE.Mesh(new RoundedBoxGeometry(3.34, 1.94, 0.115, 4, 0.04), graphite);
  slab.position.set(0, 1.59, 0);
  group.add(slab);

  const loader = new THREE.TextureLoader();
  const [texBefore, texAfter] = await Promise.all([
    loader.loadAsync(opts.before),
    loader.loadAsync(opts.after),
  ]);
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
        float line = 1.0 - smoothstep(0.0, 0.005, abs(vUv.x - uWipe));
        c = mix(c, uCoral, line * scanning);
        float sheen = smoothstep(0.22, 0.0, abs(vUv.x * 0.5 + vUv.y * 0.86 - 0.66)) * 0.045;
        c += sheen;
        gl_FragColor = vec4(c, 1.0);
        #include <colorspace_fragment>
      }
    `,
  });
  const screen = new THREE.Mesh(new THREE.PlaneGeometry(3.2, 1.8), screenMat);
  screen.position.set(0, 1.59, 0.0595);
  group.add(screen);

  const dotMat = new THREE.MeshBasicMaterial({ color: CORAL });
  const dot = new THREE.Mesh(new THREE.CircleGeometry(0.016, 24), dotMat);
  dot.position.set(0, 0.652, 0.0595);
  group.add(dot);

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

  // ── sizing ──────────────────────────────────────────────────────
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
  const BASE_YAW = -0.3;
  let targetYawOff = 0;
  let targetPitchOff = 0;
  let yawOff = 0;
  let pitchOff = 0;
  const onPointer = (e: PointerEvent) => {
    const r = host.getBoundingClientRect();
    const nx = (e.clientX - r.left) / r.width - 0.5;
    const ny = (e.clientY - r.top) / r.height - 0.5;
    targetYawOff = nx * 0.11;
    targetPitchOff = ny * 0.05;
  };
  const onLeave = () => {
    targetYawOff = 0;
    targetPitchOff = 0;
  };
  host.addEventListener("pointermove", onPointer);
  host.addEventListener("pointerleave", onLeave);

  let raf = 0;
  let running = false;
  let visible = true;
  let pageVisible = !document.hidden;
  let start = 0;
  let last = 0;
  let entranceDone = false;
  let firstFrame = true;
  let lastPhase: "before" | "after" = "before";

  // deterministic states for visual acceptance shots: ?dm3d=before|after|scan
  const forced = new URLSearchParams(window.location.search).get("dm3d");
  const forcedWipe = forced === "after" ? 1 : forced === "scan" ? 0.46 : forced === "before" ? 0 : null;

  const frame = (now: number) => {
    raf = requestAnimationFrame(frame);
    if (start === 0) {
      start = now;
      last = now;
    }
    const dt = Math.min((now - last) / 1000, 0.05);
    last = now;
    const t = (now - start) / 1000;

    const enter = forcedWipe !== null ? 1 : Math.min(t / 1.05, 1);
    const e = outQuint(enter);
    if (!entranceDone && enter >= 1) entranceDone = true;

    yawOff += (targetYawOff - yawOff) * (1 - Math.exp(-6 * dt));
    pitchOff += (targetPitchOff - pitchOff) * (1 - Math.exp(-6 * dt));

    group.position.y = (1 - e) * -0.24 + Math.sin(t * 0.9) * 0.012;
    group.rotation.y = BASE_YAW + (1 - e) * 0.26 + Math.sin(t * 0.5) * 0.018 + yawOff;
    group.rotation.x = pitchOff;

    const wipeT = entranceDone ? t - 1.05 : 0;
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

  const setRunning = () => {
    const should = visible && pageVisible;
    if (should && !running) {
      running = true;
      last = 0;
      start = start === 0 ? 0 : start; // timeline keeps absolute clock
      raf = requestAnimationFrame((now) => {
        last = now;
        frame(now);
      });
    } else if (!should && running) {
      running = false;
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
    for (const g of [foot, neck, slab, screen, dot, shadow]) g.geometry.dispose();
    for (const m of [graphite, aluminum, screenMat, dotMat, shadowMat]) m.dispose();
    for (const x of [texBefore, texAfter, shadowTex, envTex]) x.dispose();
    pmrem.dispose();
    renderer.dispose();
    canvas.remove();
  };
}
