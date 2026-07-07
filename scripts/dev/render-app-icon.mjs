#!/usr/bin/env node
import { deflateSync } from "node:zlib";
import { writeFileSync } from "node:fs";
import { join } from "node:path";

const root = new URL("../..", import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1");
const assetDir = join(root, "src", "DeskMakeover.App", "Assets");

const sizes = [16, 24, 32, 48, 64, 128, 256];
writeFileSync(join(assetDir, "app-icon.svg"), svgSource());
const png512 = renderPng(512);
writeFileSync(join(assetDir, "app-icon.png"), png512);
writeFileSync(join(assetDir, "app.ico"), makeIco(sizes.map((s) => renderPng(s))));

function svgSource() {
  return String.raw`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" role="img" aria-label="DeskMakeover">
  <defs>
    <linearGradient id="coral" x1="96" y1="48" x2="416" y2="464" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="#ff8b74"/>
      <stop offset="0.55" stop-color="#ff6f5e"/>
      <stop offset="1" stop-color="#d94e42"/>
    </linearGradient>
    <linearGradient id="panel" x1="126" y1="128" x2="386" y2="356" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="#fff8ef"/>
      <stop offset="1" stop-color="#f5e2d1"/>
    </linearGradient>
    <filter id="softShadow" x="-20%" y="-20%" width="140%" height="150%">
      <feDropShadow dx="0" dy="22" stdDeviation="22" flood-color="#7a211a" flood-opacity="0.26"/>
    </filter>
  </defs>

  <path d="M256 32c77 0 127 12 164 49s49 87 49 175-12 138-49 175-87 49-164 49-127-12-164-49-49-87-49-175 12-138 49-175 87-49 164-49Z" fill="url(#coral)"/>
  <path d="M115 141c0-22 18-40 40-40h202c22 0 40 18 40 40v186c0 22-18 40-40 40H155c-22 0-40-18-40-40V141Z" fill="url(#panel)" filter="url(#softShadow)"/>
  <path d="M147 156c0-12 10-22 22-22h174c12 0 22 10 22 22v38H147v-38Z" fill="#fffaf4"/>
  <path d="M147 194h218v118c0 13-10 23-23 23H170c-13 0-23-10-23-23V194Z" fill="#2b2728" opacity="0.92"/>

  <rect x="174" y="224" width="54" height="54" rx="14" fill="#fff3e7"/>
  <rect x="244" y="224" width="54" height="54" rx="14" fill="#ffd8c8"/>
  <rect x="314" y="224" width="24" height="54" rx="12" fill="#ffb6a3"/>
  <rect x="174" y="294" width="84" height="12" rx="6" fill="#fff3e7" opacity="0.72"/>
  <rect x="274" y="294" width="64" height="12" rx="6" fill="#fff3e7" opacity="0.42"/>

  <path d="M369 88l10 29 29 10-29 10-10 29-10-29-29-10 29-10 10-29Z" fill="#fff8ef"/>
  <path d="M118 352l7 19 19 7-19 7-7 19-7-19-19-7 19-7 7-19Z" fill="#fff8ef" opacity="0.92"/>
  <path d="M155 133h202c10 0 18 8 18 18v176c0 10-8 18-18 18H155c-10 0-18-8-18-18V151c0-10 8-18 18-18Z" fill="none" stroke="#fff8ef" stroke-opacity="0.32" stroke-width="6"/>
</svg>
`;
}

function renderPng(size) {
  const rgba = Buffer.alloc(size * size * 4);
  const samples = size < 48 ? 3 : 4;
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      let r = 0, g = 0, b = 0, a = 0;
      for (let sy = 0; sy < samples; sy++) {
        for (let sx = 0; sx < samples; sx++) {
          const px = (x + (sx + 0.5) / samples) / size * 512;
          const py = (y + (sy + 0.5) / samples) / size * 512;
          const c = pixel(px, py);
          const alpha = c.a / 255;
          r += c.r * alpha; g += c.g * alpha; b += c.b * alpha; a += alpha;
        }
      }
      const n = samples * samples;
      const o = (y * size + x) * 4;
      rgba[o + 3] = Math.round(a / n * 255);
      if (rgba[o + 3] > 0) {
        rgba[o] = Math.round(r / a);
        rgba[o + 1] = Math.round(g / a);
        rgba[o + 2] = Math.round(b / a);
      }
    }
  }

  return encodePng(size, size, rgba);
}

function pixel(x, y) {
  let c = transparent();
  if (insideSquircle(x, y, 256, 256, 224, 5.1)) {
    c = over(coralGradient(x, y), c);
  }

  const shadowAlpha = Math.max(0, 1 - distanceRoundRect(x, y - 22, 256, 234, 143, 118, 40) / 70);
  if (shadowAlpha > 0) {
    c = over({ r: 122, g: 33, b: 26, a: Math.round(54 * shadowAlpha) }, c);
  }

  if (roundRect(x, y, 256, 234, 282, 266, 40)) {
    c = over(panelGradient(x, y), c);
  }
  if (roundRect(x, y, 256, 164, 228, 68, 24)) {
    c = over({ r: 255, g: 250, b: 244, a: 255 }, c);
  }
  if (roundRect(x, y, 256, 264, 228, 140, 22)) {
    c = over({ r: 43, g: 39, b: 40, a: 235 }, c);
  }

  c = tile(c, x, y, 201, 251, 54, 54, 14, { r: 255, g: 243, b: 231, a: 255 });
  c = tile(c, x, y, 271, 251, 54, 54, 14, { r: 255, g: 216, b: 200, a: 255 });
  c = tile(c, x, y, 326, 251, 24, 54, 12, { r: 255, g: 182, b: 163, a: 255 });
  c = tile(c, x, y, 216, 300, 84, 12, 6, { r: 255, g: 243, b: 231, a: 184 });
  c = tile(c, x, y, 306, 300, 64, 12, 6, { r: 255, g: 243, b: 231, a: 108 });

  if (sparkle(x, y, 369, 127, 39)) c = over({ r: 255, g: 248, b: 239, a: 255 }, c);
  if (sparkle(x, y, 118, 378, 26)) c = over({ r: 255, g: 248, b: 239, a: 235 }, c);

  if (strokeRoundRect(x, y, 256, 239, 238, 212, 28, 6)) {
    c = over({ r: 255, g: 248, b: 239, a: 82 }, c);
  }
  return c;
}

function tile(base, x, y, cx, cy, w, h, r, color) {
  return roundRect(x, y, cx, cy, w, h, r) ? over(color, base) : base;
}

function roundRect(x, y, cx, cy, w, h, r) {
  return distanceRoundRect(x, y, cx, cy, w, h, r) <= 0;
}

function strokeRoundRect(x, y, cx, cy, w, h, r, stroke) {
  const d = distanceRoundRect(x, y, cx, cy, w, h, r);
  return d <= 0 && d >= -stroke;
}

function distanceRoundRect(x, y, cx, cy, w, h, r) {
  const qx = Math.abs(x - cx) - w / 2 + r;
  const qy = Math.abs(y - cy) - h / 2 + r;
  return Math.hypot(Math.max(qx, 0), Math.max(qy, 0)) + Math.min(Math.max(qx, qy), 0) - r;
}

function insideSquircle(x, y, cx, cy, radius, n) {
  const dx = Math.abs((x - cx) / radius);
  const dy = Math.abs((y - cy) / radius);
  return Math.pow(dx, n) + Math.pow(dy, n) <= 1;
}

function sparkle(x, y, cx, cy, r) {
  const dx = Math.abs(x - cx);
  const dy = Math.abs(y - cy);
  return dx + dy * 0.72 < r && dy + dx * 0.72 < r;
}

function coralGradient(x, y) {
  const t = clamp((x * 0.35 + y * 0.65) / 512);
  return mix({ r: 255, g: 139, b: 116, a: 255 }, { r: 217, g: 78, b: 66, a: 255 }, t);
}

function panelGradient(x, y) {
  const t = clamp((x * 0.25 + y * 0.75 - 100) / 360);
  return mix({ r: 255, g: 248, b: 239, a: 255 }, { r: 245, g: 226, b: 209, a: 255 }, t);
}

function over(src, dst) {
  const sa = src.a / 255;
  const da = dst.a / 255;
  const oa = sa + da * (1 - sa);
  if (oa <= 0) return transparent();
  return {
    r: Math.round((src.r * sa + dst.r * da * (1 - sa)) / oa),
    g: Math.round((src.g * sa + dst.g * da * (1 - sa)) / oa),
    b: Math.round((src.b * sa + dst.b * da * (1 - sa)) / oa),
    a: Math.round(oa * 255),
  };
}

function mix(a, b, t) {
  return {
    r: Math.round(a.r + (b.r - a.r) * t),
    g: Math.round(a.g + (b.g - a.g) * t),
    b: Math.round(a.b + (b.b - a.b) * t),
    a: Math.round(a.a + (b.a - a.a) * t),
  };
}

function transparent() {
  return { r: 0, g: 0, b: 0, a: 0 };
}

function clamp(v) {
  return Math.max(0, Math.min(1, v));
}

function encodePng(width, height, rgba) {
  const stride = width * 4;
  const raw = Buffer.alloc((stride + 1) * height);
  for (let y = 0; y < height; y++) {
    raw[y * (stride + 1)] = 0;
    rgba.copy(raw, y * (stride + 1) + 1, y * stride, (y + 1) * stride);
  }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 6; // RGBA
  ihdr[10] = 0; // compression
  ihdr[11] = 0; // filter
  ihdr[12] = 0; // interlace
  return Buffer.concat([
    Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
    chunk("IHDR", ihdr),
    chunk("IDAT", deflateSync(raw, { level: 9 })),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

function makeIco(images) {
  const header = Buffer.alloc(6);
  header.writeUInt16LE(0, 0);
  header.writeUInt16LE(1, 2);
  header.writeUInt16LE(images.length, 4);
  let offset = 6 + images.length * 16;
  const entries = [];
  for (let i = 0; i < images.length; i++) {
    const size = sizes[i];
    const entry = Buffer.alloc(16);
    entry[0] = size === 256 ? 0 : size;
    entry[1] = size === 256 ? 0 : size;
    entry[2] = 0;
    entry[3] = 0;
    entry.writeUInt16LE(1, 4);
    entry.writeUInt16LE(32, 6);
    entry.writeUInt32LE(images[i].length, 8);
    entry.writeUInt32LE(offset, 12);
    entries.push(entry);
    offset += images[i].length;
  }
  return Buffer.concat([header, ...entries, ...images]);
}

function u32(...nums) {
  const b = Buffer.alloc(nums.length * 4);
  nums.forEach((n, i) => b.writeUInt32BE(n >>> 0, i * 4));
  return b;
}

function chunk(type, data) {
  const name = Buffer.from(type);
  const len = u32(data.length);
  const crc = u32(crc32(Buffer.concat([name, data])));
  return Buffer.concat([len, name, data, crc]);
}

function crc32(buf) {
  let c = 0xffffffff;
  for (const byte of buf) {
    c ^= byte;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  }
  return (c ^ 0xffffffff) >>> 0;
}
