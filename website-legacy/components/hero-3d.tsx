"use client";

import { useEffect, useImperativeHandle, useRef, forwardRef } from "react";

/**
 * The 3D makeover scene (three.js, dynamically imported). A real desktop
 * render lies as a slab in space; ~60 styled icon cards hover scattered in
 * 3D, then fly in a left-to-right wave and land exactly on their spots,
 * covering the stock icons — the makeover, literally. Restore sends them
 * back into space. Pointer orbits the scene. The parent owns the state
 * machine; this component only animates.
 */

type Cell = { x: number; y: number; w: number; h: number };

export interface Hero3dHandle {
  assemble(ms: number): void;
  scatter(ms: number): void;
  set(assembled: boolean): void;
}

interface Hero3dProps {
  beforeUrl: string;
  afterUrl: string;
  cells: Cell[];
  className?: string;
  onReady?: () => void;
  onFail?: () => void;
}

const SLAB_W = 3.6;
const SLAB_H = SLAB_W * (1124 / 2000);
const CARD_COUNT = 64;

function easeOutCubic(t: number) {
  return 1 - Math.pow(1 - t, 3);
}
function easeInCubic(t: number) {
  return t * t * t;
}

export const Hero3d = forwardRef<Hero3dHandle, Hero3dProps>(function Hero3d(
  { beforeUrl, afterUrl, cells, className, onReady, onFail },
  ref,
) {
  const hostRef = useRef<HTMLDivElement>(null);
  const animRef = useRef<{
    dir: "in" | "out";
    t0: number;
    dur: number;
    progress: number; // 0 scattered, 1 assembled
    from: number;
  }>({ dir: "in", t0: 0, dur: 0, progress: 0, from: 0 });

  useImperativeHandle(ref, () => ({
    assemble(ms) {
      const a = animRef.current;
      a.dir = "in";
      a.from = a.progress;
      a.dur = ms;
      a.t0 = performance.now();
    },
    scatter(ms) {
      const a = animRef.current;
      a.dir = "out";
      a.from = a.progress;
      a.dur = ms;
      a.t0 = performance.now();
    },
    set(assembled) {
      const a = animRef.current;
      a.progress = assembled ? 1 : 0;
      a.from = a.progress;
      a.dur = 0;
    },
  }));

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    let dead = false;
    let cleanup: (() => void) | undefined;

    (async () => {
      let THREE: typeof import("three");
      try {
        THREE = await import("three");
      } catch {
        onFail?.();
        return;
      }
      if (dead) return;

      const renderer = new THREE.WebGLRenderer({ alpha: true, antialias: true, powerPreference: "low-power" });
      if (!renderer.getContext()) {
        onFail?.();
        return;
      }
      renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
      renderer.setClearColor(0x000000, 0);
      host.appendChild(renderer.domElement);
      renderer.domElement.style.width = "100%";
      renderer.domElement.style.height = "100%";
      renderer.domElement.style.display = "block";

      const scene = new THREE.Scene();
      const camera = new THREE.PerspectiveCamera(32, 16 / 9, 0.1, 30);
      camera.position.set(0, 0.32, 4.4);
      camera.lookAt(0, 0, 0);

      const group = new THREE.Group();
      group.rotation.x = -0.17;
      group.rotation.y = -0.16;
      scene.add(group);

      const loader = new THREE.TextureLoader();
      const load = (url: string) =>
        new Promise<import("three").Texture>((resolve, reject) => {
          loader.load(
            url,
            (t) => {
              t.colorSpace = THREE.SRGBColorSpace;
              t.anisotropy = Math.min(4, renderer.capabilities.getMaxAnisotropy());
              resolve(t);
            },
            undefined,
            reject,
          );
        });

      let beforeTex: import("three").Texture;
      let afterTex: import("three").Texture;
      try {
        [beforeTex, afterTex] = await Promise.all([load(beforeUrl), load(afterUrl)]);
      } catch {
        onFail?.();
        renderer.dispose();
        host.removeChild(renderer.domElement);
        return;
      }
      if (dead) {
        renderer.dispose();
        return;
      }

      // the desktop slab (stock desktop)
      const slabGeo = new THREE.PlaneGeometry(SLAB_W, SLAB_H);
      const slabMat = new THREE.MeshBasicMaterial({ map: beforeTex });
      const slab = new THREE.Mesh(slabGeo, slabMat);
      group.add(slab);

      // styled icon cards, one per detected cell, UV-windowed into the after render
      const picked = [...cells].sort((a, b) => b.w * b.h - a.w * a.h).slice(0, CARD_COUNT);
      const cardMat = new THREE.MeshBasicMaterial({ map: afterTex, transparent: true });
      type Card = {
        mesh: import("three").Mesh;
        home: import("three").Vector3;
        away: import("three").Vector3;
        awayRot: import("three").Euler;
        delay: number;
      };
      const cards: Card[] = [];
      const rand = (seed: number) => {
        // deterministic pseudo-random so SSR/replays match
        const x = Math.sin(seed * 127.1 + 311.7) * 43758.5453;
        return x - Math.floor(x);
      };
      picked.forEach((c, i) => {
        const w = (c.w / 100) * SLAB_W;
        const hgt = (c.h / 100) * SLAB_H;
        const geo = new THREE.PlaneGeometry(w, hgt);
        const uv = geo.attributes.uv as import("three").BufferAttribute;
        for (let k = 0; k < uv.count; k++) {
          const u = uv.getX(k);
          const v = uv.getY(k);
          uv.setXY(k, (c.x + u * c.w) / 100, 1 - (c.y + (1 - v) * c.h) / 100);
        }
        const mesh = new THREE.Mesh(geo, cardMat);
        const home = new THREE.Vector3(
          ((c.x + c.w / 2) / 100) * SLAB_W - SLAB_W / 2,
          SLAB_H / 2 - ((c.y + c.h / 2) / 100) * SLAB_H,
          0.012,
        );
        const away = new THREE.Vector3(
          home.x * 1.7 + (rand(i) - 0.5) * 2.2,
          home.y * 1.6 + (rand(i + 40) - 0.5) * 1.4 + 0.25,
          0.7 + rand(i + 80) * 1.5,
        );
        const awayRot = new THREE.Euler((rand(i + 120) - 0.5) * 1.6, (rand(i + 160) - 0.5) * 1.6, (rand(i + 200) - 0.5) * 0.8);
        mesh.position.copy(away);
        mesh.rotation.copy(awayRot);
        group.add(mesh);
        cards.push({ mesh, home, away, awayRot, delay: (c.x / 100) * 0.45 + rand(i + 240) * 0.08 });
      });

      // pointer orbit
      const pointer = { x: 0, y: 0, tx: 0, ty: 0 };
      const onMove = (e: PointerEvent) => {
        const r = host.getBoundingClientRect();
        pointer.tx = ((e.clientX - r.left) / r.width - 0.5) * 2;
        pointer.ty = ((e.clientY - r.top) / r.height - 0.5) * 2;
      };
      const onLeave = () => {
        pointer.tx = 0;
        pointer.ty = 0;
      };
      host.addEventListener("pointermove", onMove);
      host.addEventListener("pointerleave", onLeave);

      const resize = () => {
        const w = host.clientWidth;
        const h = host.clientHeight;
        renderer.setSize(w, h, false);
        camera.aspect = w / h;
        camera.updateProjectionMatrix();
      };
      resize();
      const ro = new ResizeObserver(resize);
      ro.observe(host);

      let visible = true;
      const io = new IntersectionObserver((es) => {
        for (const e of es) visible = e.isIntersecting;
      });
      io.observe(host);

      const tmp = new THREE.Vector3();
      let raf = 0;
      const tick = () => {
        raf = requestAnimationFrame(tick);
        if (!visible || document.hidden) return;
        const now = performance.now();
        const a = animRef.current;
        if (a.dur > 0) {
          const t = Math.min(1, (now - a.t0) / a.dur);
          a.progress = a.from + ((a.dir === "in" ? 1 : 0) - a.from) * t;
          if (t >= 1) a.dur = 0;
        }
        // per-card staged progress with x-wave delays
        const span = 0.55; // portion of the tween each card takes
        for (const card of cards) {
          const local = Math.min(1, Math.max(0, (a.progress - card.delay * (a.dir === "in" ? 1 : 0.3)) / span));
          const e = a.dir === "in" ? easeOutCubic(local) : easeInCubic(local);
          tmp.lerpVectors(card.away, card.home, e);
          card.mesh.position.copy(tmp);
          card.mesh.rotation.set(card.awayRot.x * (1 - e), card.awayRot.y * (1 - e), card.awayRot.z * (1 - e));
          (card.mesh.material as import("three").MeshBasicMaterial).opacity = 0.25 + 0.75 * Math.min(1, e * 3 + a.progress);
        }
        // orbit + idle drift
        pointer.x += (pointer.tx - pointer.x) * 0.06;
        pointer.y += (pointer.ty - pointer.y) * 0.06;
        group.rotation.y = -0.16 + pointer.x * 0.18 + Math.sin(now / 5200) * 0.022;
        group.rotation.x = -0.17 - pointer.y * 0.09 + Math.cos(now / 6100) * 0.012;
        renderer.render(scene, camera);
      };
      tick();
      onReady?.();

      cleanup = () => {
        cancelAnimationFrame(raf);
        ro.disconnect();
        io.disconnect();
        host.removeEventListener("pointermove", onMove);
        host.removeEventListener("pointerleave", onLeave);
        slabGeo.dispose();
        slabMat.dispose();
        cardMat.dispose();
        for (const c of cards) c.mesh.geometry.dispose();
        beforeTex.dispose();
        afterTex.dispose();
        renderer.dispose();
        if (renderer.domElement.parentElement === host) host.removeChild(renderer.domElement);
      };
    })();

    return () => {
      dead = true;
      cleanup?.();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [beforeUrl, afterUrl]);

  return <div ref={hostRef} className={className} aria-hidden="true" />;
});
