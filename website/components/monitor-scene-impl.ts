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

export interface MountOptions {
  before: string;
  after: string;
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

/** Fine horizontal streaks — brushed metal under anisotropic reflections. */
function brushedRoughnessTexture(): THREE.CanvasTexture {
  const c = document.createElement("canvas");
  c.width = 512;
  c.height = 512;
  const g = c.getContext("2d")!;
  g.fillStyle = "rgb(100,100,100)";
  g.fillRect(0, 0, 512, 512);
  for (let i = 0; i < 1800; i++) {
    const y = Math.random() * 512;
    const v = 90 + Math.floor(Math.random() * 26);
    g.strokeStyle = `rgba(${v},${v},${v},0.3)`;
    g.lineWidth = 0.75;
    const x = Math.random() * 512;
    const len = 60 + Math.random() * 220;
    g.beginPath();
    g.moveTo(x, y);
    g.lineTo(x + len, y);
    g.stroke();
  }
  const t = new THREE.CanvasTexture(c);
  t.wrapS = THREE.RepeatWrapping;
  t.wrapT = THREE.RepeatWrapping;
  t.repeat.set(1.5, 1.5);
  return t;
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
  const envScene = studioEnvironment();
  const envTarget = pmrem.fromScene(envScene, 0.02);
  envScene.traverse((o) => {
    if (o instanceof THREE.Mesh) {
      o.geometry.dispose();
      (o.material as THREE.Material).dispose();
    }
  });
  scene.environment = envTarget.texture;

  const camera = new THREE.PerspectiveCamera(30, 1, 0.1, 60);
  const halfTan = () => Math.tan(THREE.MathUtils.degToRad(camera.fov / 2));

  const key = new THREE.DirectionalLight(0xffffff, 0.55);
  key.position.set(3.5, 6, 4.5);
  scene.add(key);

  // ── the display: thin aluminum slab, white glass front, wedge stand ──
  const group = new THREE.Group();
  scene.add(group);

  const brushed = brushedRoughnessTexture();
  const aluminum = new THREE.MeshPhysicalMaterial({
    color: 0xd8dce1,
    metalness: 1,
    roughness: 0.85,
    roughnessMap: brushed,
    anisotropy: 0.4,
  });
  const glassWhite = new THREE.MeshPhysicalMaterial({
    color: 0xf2f3f5,
    metalness: 0,
    roughness: 0.34,
    clearcoat: 1,
    clearcoatRoughness: 0.07,
  });

  const slab = new THREE.Mesh(new RoundedBoxGeometry(3.36, 2.14, 0.07, 6, 0.035), aluminum);
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
        gl_FragColor = vec4(c, 1.0);
        #include <colorspace_fragment>
      }
    `,
  });
  // the recessed dark seam where the panel meets the glass — the junction
  // detail that makes the front read as a real display, not a sticker
  const seamMat = new THREE.MeshStandardMaterial({ color: 0x0b0c0e, roughness: 0.55 });
  const seam = new THREE.Mesh(new THREE.PlaneGeometry(3.26, 1.86), seamMat);
  seam.position.set(0, 1.59, 0.049);
  group.add(seam);

  const screen = new THREE.Mesh(new THREE.PlaneGeometry(3.2, 1.8), screenMat);
  screen.position.set(0, 1.59, 0.0495);
  group.add(screen);

  // one continuous sheet of cover glass over panel AND bezel: environment
  // streaks run across the whole face, exactly like a real all-in-one
  const coverMat = new THREE.MeshPhysicalMaterial({
    color: 0x000000,
    transparent: true,
    opacity: 0.06,
    roughness: 0.06,
    metalness: 0,
    envMapIntensity: 1.6,
    depthWrite: false,
  });
  const cover = new THREE.Mesh(new THREE.PlaneGeometry(3.3, 2.07), coverMat);
  cover.position.set(0, 1.5, 0.0505);
  group.add(cover);

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
    // On a wide full-bleed stage the copy owns the left, so the whole
    // composition trucks right (camera pans left by ox). Narrow hosts center.
    const ht = halfTan();
    const a = camera.aspect;
    const isWideStage = a > 1.4;
    const cy = cssNum("--dm-screen-cy", 0.5);
    // wide opener: whole product in frame whatever the host shape
    const zA = Math.max(8.8, 2.3 / (ht * a));
    const vhA = 2 * zA * ht;
    const oxA = isWideStage ? 0.17 * (vhA * a) : 0;
    const oyA = (0.5 - cy) * vhA * 0.6;
    LOOK_A.set(-oxA, 1.32 - oyA, 0);
    POS_A.set(4.2 - oxA, 2.6 - oyA, zA);
    // close state: the glass fills --dm-fill of the canvas height
    const fillH = cssNum("--dm-fill", isWideStage ? 0.9 : 0.96);
    const vhB = 2.06 / fillH;
    const zB = 0.05 + vhB / (2 * ht);
    const oxB = isWideStage ? 0.13 * (vhB * a) : 0;
    const oyB = (0.5 - cy) * vhB;
    LOOK_B.set(-0.2 - oxB, 1.59 - oyB, 0.05);
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
    for (const g of [slab, glass, leg, foot, seam, screen, cover, shadow]) g.geometry.dispose();
    for (const m of [aluminum, glassWhite, seamMat, screenMat, coverMat, shadowMat]) m.dispose();
    for (const x of [texBefore, texAfter, shadowTex, brushed]) x.dispose();
    envTarget.dispose();
    pmrem.dispose();
    renderer.dispose();
    canvas.remove();
  };
}
