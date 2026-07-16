"use client";

import { useEffect, useImperativeHandle, useRef, forwardRef } from "react";

/**
 * The GPU transformation plane. A single quad mixes the real before/after
 * desktop renders along an animated scan edge: pixels ripple and chroma-split
 * as they are re-minted, a coral glow rides the edge, a faint sheen drifts
 * across the settled desktop, and a soft light follows the pointer.
 * Raw WebGL1, zero dependencies. The parent decides when to play; this
 * component only tweens uProgress and draws.
 */

const VERT = `
attribute vec2 aPos;
varying vec2 vUv;
void main() {
  vUv = vec2(aPos.x * 0.5 + 0.5, 0.5 - aPos.y * 0.5);
  gl_Position = vec4(aPos, 0.0, 1.0);
}`;

const FRAG = `
precision highp float;
uniform sampler2D uBefore;
uniform sampler2D uAfter;
uniform float uProgress;
uniform float uActive;
uniform float uTime;
uniform vec2 uMouse;
varying vec2 vUv;

void main() {
  vec2 uv = vUv;
  float edge = uProgress;
  float d = uv.x - edge;
  float band = 0.055;
  float w = exp(-abs(d) / band) * uActive;

  float ripple = sin(uv.y * 64.0 - uTime * 7.0) * 0.0045 * w;
  vec2 uvB = clamp(uv + vec2(ripple, ripple * 0.6), 0.0, 1.0);
  vec2 uvA = clamp(uv - vec2(ripple, ripple * 0.6), 0.0, 1.0);

  float ca = 0.0035 * w;
  vec3 before = vec3(
    texture2D(uBefore, clamp(uvB + vec2(ca, 0.0), 0.0, 1.0)).r,
    texture2D(uBefore, uvB).g,
    texture2D(uBefore, clamp(uvB - vec2(ca, 0.0), 0.0, 1.0)).b);
  vec3 after = vec3(
    texture2D(uAfter, clamp(uvA + vec2(ca, 0.0), 0.0, 1.0)).r,
    texture2D(uAfter, uvA).g,
    texture2D(uAfter, clamp(uvA - vec2(ca, 0.0), 0.0, 1.0)).b);

  float side = 1.0 - step(edge, uv.x);
  vec3 col = mix(before, after, side);

  // freshly-minted lift just behind the edge
  col *= 1.0 + 0.10 * exp(-max(-d, 0.0) / 0.05) * uActive * side;

  // coral glow riding the scan edge
  vec3 coral = vec3(1.0, 0.435, 0.369);
  float line = exp(-pow(abs(d) / 0.012, 1.6)) * uActive;
  col += coral * line * 0.6;

  // idle sheen drifting across the settled desktop
  float sweep = fract(uTime * 0.045);
  float sheenPos = uv.x + uv.y * 0.35 - sweep * 2.2 + 0.6;
  float sheen = exp(-pow(sheenPos / 0.09, 2.0)) * 0.045;
  col += vec3(sheen);

  // soft pointer light
  if (uMouse.x >= 0.0) {
    float md = distance(uv * vec2(1.78, 1.0), uMouse * vec2(1.78, 1.0));
    col *= 1.0 + 0.06 * exp(-md * md / 0.045);
  }

  gl_FragColor = vec4(clamp(col, 0.0, 1.0), 1.0);
}`;

export interface DeskGlHandle {
  /** tween the scan edge to 0 (before) or 1 (after) over ms */
  play(target: 0 | 1, ms: number): void;
  set(target: 0 | 1): void;
}

interface DeskGlProps {
  beforeUrl: string;
  afterUrl: string;
  className?: string;
  onReady?: () => void;
  onFail?: () => void;
}

function easeScan(t: number) {
  // approximates cubic-bezier(0.45, 0.05, 0.25, 1)
  return t < 0.5 ? 2.9 * t * t * (1.1 - t) : 1 - Math.pow(1 - t, 2.4) * 0.9;
}

export const DeskGl = forwardRef<DeskGlHandle, DeskGlProps>(function DeskGl(
  { beforeUrl, afterUrl, className, onReady, onFail },
  ref,
) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const stateRef = useRef<{
    progress: number;
    from: number;
    to: number;
    t0: number;
    dur: number;
    mouse: [number, number];
    running: boolean;
    visible: boolean;
    draw?: () => void;
  }>({ progress: 0, from: 0, to: 0, t0: 0, dur: 0, mouse: [-1, -1], running: false, visible: true });

  useImperativeHandle(ref, () => ({
    play(target, ms) {
      const s = stateRef.current;
      s.from = s.progress;
      s.to = target;
      s.dur = ms;
      s.t0 = performance.now();
    },
    set(target) {
      const s = stateRef.current;
      s.progress = target;
      s.from = target;
      s.to = target;
      s.dur = 0;
    },
  }));

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const gl = canvas.getContext("webgl", { antialias: false, alpha: false, powerPreference: "low-power" });
    if (!gl) {
      onFail?.();
      return;
    }
    let dead = false;
    const s = stateRef.current;

    const compile = (type: number, src: string) => {
      const sh = gl.createShader(type)!;
      gl.shaderSource(sh, src);
      gl.compileShader(sh);
      if (!gl.getShaderParameter(sh, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(sh) ?? "shader");
      return sh;
    };

    let prog: WebGLProgram;
    try {
      prog = gl.createProgram()!;
      gl.attachShader(prog, compile(gl.VERTEX_SHADER, VERT));
      gl.attachShader(prog, compile(gl.FRAGMENT_SHADER, FRAG));
      gl.linkProgram(prog);
      if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) throw new Error("link");
    } catch {
      onFail?.();
      return;
    }
    gl.useProgram(prog);

    const quad = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, quad);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
    const aPos = gl.getAttribLocation(prog, "aPos");
    gl.enableVertexAttribArray(aPos);
    gl.vertexAttribPointer(aPos, 2, gl.FLOAT, false, 0, 0);

    const uProgress = gl.getUniformLocation(prog, "uProgress");
    const uActive = gl.getUniformLocation(prog, "uActive");
    const uTime = gl.getUniformLocation(prog, "uTime");
    const uMouse = gl.getUniformLocation(prog, "uMouse");
    gl.uniform1i(gl.getUniformLocation(prog, "uBefore"), 0);
    gl.uniform1i(gl.getUniformLocation(prog, "uAfter"), 1);

    const makeTexture = (unit: number, source: TexImageSource) => {
      const tex = gl.createTexture();
      gl.activeTexture(gl.TEXTURE0 + unit);
      gl.bindTexture(gl.TEXTURE_2D, tex);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGB, gl.RGB, gl.UNSIGNED_BYTE, source);
    };

    const loadImage = (url: string) =>
      new Promise<HTMLImageElement>((resolve, reject) => {
        const img = new Image();
        img.onload = () => resolve(img);
        img.onerror = reject;
        img.src = url;
      });

    const resize = () => {
      const dpr = Math.min(window.devicePixelRatio || 1, 2);
      const w = Math.round(canvas.clientWidth * dpr);
      const hpx = Math.round(canvas.clientHeight * dpr);
      if (canvas.width !== w || canvas.height !== hpx) {
        canvas.width = w;
        canvas.height = hpx;
        gl.viewport(0, 0, w, hpx);
      }
    };
    const ro = new ResizeObserver(resize);
    ro.observe(canvas);

    let raf = 0;
    const draw = () => {
      raf = requestAnimationFrame(draw);
      if (!s.visible || document.hidden) return;
      resize();
      const now = performance.now();
      if (s.progress !== s.to || s.dur > 0) {
        const t = s.dur === 0 ? 1 : Math.min(1, (now - s.t0) / s.dur);
        s.progress = s.from + (s.to - s.from) * easeScan(t);
        if (t >= 1) {
          s.progress = s.to;
          s.dur = 0;
        }
      }
      const animating = s.dur > 0;
      gl.uniform1f(uProgress, s.progress);
      gl.uniform1f(uActive, animating ? 1 : 0);
      gl.uniform1f(uTime, now / 1000);
      gl.uniform2f(uMouse, s.mouse[0], s.mouse[1]);
      gl.drawArrays(gl.TRIANGLES, 0, 3);
    };

    const io = new IntersectionObserver((entries) => {
      for (const e of entries) s.visible = e.isIntersecting;
    });
    io.observe(canvas);

    const onPointer = (e: PointerEvent) => {
      const r = canvas.getBoundingClientRect();
      s.mouse = [(e.clientX - r.left) / r.width, (e.clientY - r.top) / r.height];
    };
    const onLeave = () => {
      s.mouse = [-1, -1];
    };
    canvas.addEventListener("pointermove", onPointer);
    canvas.addEventListener("pointerleave", onLeave);

    Promise.all([loadImage(beforeUrl), loadImage(afterUrl)])
      .then(([b, a]) => {
        if (dead) return;
        makeTexture(0, b);
        makeTexture(1, a);
        resize();
        draw();
        onReady?.();
      })
      .catch(() => onFail?.());

    return () => {
      dead = true;
      cancelAnimationFrame(raf);
      ro.disconnect();
      io.disconnect();
      canvas.removeEventListener("pointermove", onPointer);
      canvas.removeEventListener("pointerleave", onLeave);
      gl.getExtension("WEBGL_lose_context")?.loseContext();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [beforeUrl, afterUrl]);

  return <canvas ref={canvasRef} className={className} aria-hidden="true" />;
});
