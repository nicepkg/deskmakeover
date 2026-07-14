import { Filter, GlProgram } from 'pixi.js'

// Pixi binds uOutputFrame/uOutputTexture to the vertex stage. Pass each
// fragment's GLOBAL physical-pixel position down for the rounded-rect SDF.
const VERTEX = `
precision highp float;
in vec2 aPosition;
out vec2 vTextureCoord;
out vec2 vGlobalPx;

uniform vec4 uInputSize;
uniform vec4 uOutputFrame;
uniform vec4 uOutputTexture;

vec4 filterVertexPosition(void) {
  vec2 position = aPosition * uOutputFrame.zw + uOutputFrame.xy;
  position.x = position.x * (2.0 / uOutputTexture.x) - 1.0;
  position.y = position.y * (2.0 * uOutputTexture.z / uOutputTexture.y) - uOutputTexture.z;
  return vec4(position, 0.0, 1.0);
}

void main(void) {
  gl_Position = filterVertexPosition();
  vTextureCoord = aPosition * (uOutputFrame.zw * uInputSize.zw);
  vGlobalPx = aPosition * uOutputFrame.zw + uOutputFrame.xy;
}
`

// Complete port of /tmp/liquid-glass webgl.html — every term, including the
// gaussian drop shadow OUTSIDE the SDF (the reference shader owns its shadow;
// zone-node hides shadowG for glass so it is not drawn twice).
//
// The reference computes in CSS pixels (uResolution = innerWidth/Height, DPR
// absorbed by three.js setPixelRatio), so its px-literal constants — shadow
// falloff 800, AA 1.5, inner-rim 2..5 — are CSS-px sized. We divide vGlobalPx
// by uK (physical px per desktop px) to run the same math in the same units at
// any render scale; only texture-UV conversions multiply back by uK.
const FRAGMENT = `
precision highp float;
in vec2 vTextureCoord;
in vec2 vGlobalPx;
out vec4 finalColor;

uniform sampler2D uTexture;
uniform vec4 uInputSize;
uniform vec4 uInputClamp;

uniform vec2 uCenter;
uniform vec2 uHalf;
uniform float uRadius;
uniform float uThickness;
uniform float uBezel;
uniform float uIOR;
uniform float uBlur;
uniform float uSpecular;
uniform float uTint;
uniform float uShadow;
uniform float uK;

float sdRoundedRect(vec2 p, vec2 halfSize, float r) {
  vec2 q = abs(p) - halfSize + r;
  return min(max(q.x, q.y), 0.0) + length(max(q, 0.0)) - r;
}

// Outward normal of the rounded rect. The reference differentiates the SDF
// numerically, but max(q.x, q.y) makes that direction FLIP across the interior
// corner bisectors — a hair-thin seam where the refraction jumps. Corner arcs
// keep the exact (already smooth) normal; the two edge normals blend over an
// 8px band around the ridge instead of switching.
vec2 sdNormal(vec2 p, vec2 halfSize, float r) {
  vec2 q = abs(p) - halfSize + r;
  vec2 s = vec2(p.x < 0.0 ? -1.0 : 1.0, p.y < 0.0 ? -1.0 : 1.0);
  if (q.x > 0.0 && q.y > 0.0) return s * normalize(q);
  float w = smoothstep(-8.0, 8.0, q.x - q.y);
  return normalize(mix(vec2(0.0, s.y), vec2(s.x, 0.0), w) + vec2(1e-6));
}

float surfaceHeight(float t) {
  float s = 1.0 - t;
  return pow(1.0 - s*s*s*s, 0.25);
}

vec3 sampleBg(vec2 uv) {
  return texture(uTexture, clamp(uv, uInputClamp.xy, uInputClamp.zw)).rgb;
}

vec3 sampleBgBlurred(vec2 uv, float radius) {
  if (radius < 0.5) return sampleBg(uv);
  vec3 sum = vec3(0.0);
  vec2 px = uInputSize.zw * uK;
  vec2 offsets[16];
  offsets[0]  = vec2(-0.94201, -0.39906);
  offsets[1]  = vec2( 0.94558, -0.76890);
  offsets[2]  = vec2(-0.09418, -0.92938);
  offsets[3]  = vec2( 0.34495,  0.29387);
  offsets[4]  = vec2(-0.91588, -0.45771);
  offsets[5]  = vec2(-0.81544,  0.48568);
  offsets[6]  = vec2(-0.38277, -0.56071);
  offsets[7]  = vec2(-0.12675,  0.84686);
  offsets[8]  = vec2( 0.89642,  0.41254);
  offsets[9]  = vec2( 0.18150, -0.30020);
  offsets[10] = vec2(-0.01445, -0.16001);
  offsets[11] = vec2( 0.59614,  0.71118);
  offsets[12] = vec2( 0.49742, -0.47280);
  offsets[13] = vec2( 0.80685,  0.04588);
  offsets[14] = vec2(-0.32490, -0.03965);
  offsets[15] = vec2(-0.60975,  0.06566);
  for (int i = 0; i < 16; i++) {
    sum += sampleBg(uv + offsets[i] * radius * px);
  }
  return sum / 16.0;
}

void main(void) {
  vec2 p = vGlobalPx / uK - uCenter;
  float sd = sdRoundedRect(p, uHalf, uRadius);

  if (sd > 0.0) {
    float shadowFalloff = exp(-sd * sd / 800.0);
    float shadowAlpha = uShadow * shadowFalloff * 0.6;
    finalColor = vec4(0.0, 0.0, 0.0, shadowAlpha);
    return;
  }

  float distFromEdge = -sd;
  // Deliberate departure from the reference, which also caps bezel at uRadius:
  // that ties dome width to corner rounding, and moderate 20–28px corners would
  // crush the 60px dome to nothing (owner wants moderate corners AND the full
  // bevel). Safe without the cap because sdNormal is smooth at any radius —
  // the reference's numeric gradient was the fragile part, not the profile.
  float bezel = max(0.001, min(uBezel, min(uHalf.x, uHalf.y) - 1.0));
  float t = clamp(distFromEdge / bezel, 0.0, 1.0);

  float h = surfaceHeight(t);
  float dt = 0.001;
  float h2 = surfaceHeight(min(t + dt, 1.0));
  float dh = (h2 - h) / dt;

  float slopeAngle = atan(dh * (uThickness / bezel));
  float sinR = clamp(sin(slopeAngle) / uIOR, -1.0, 1.0);
  float thetaR = asin(sinR);
  float displacement = h * uThickness * (tan(slopeAngle) - tan(thetaR));

  vec2 grad = sdNormal(p, uHalf, uRadius);

  vec2 refractedUV = vTextureCoord - grad * (displacement * uK) * uInputSize.zw;
  vec3 color = sampleBgBlurred(refractedUV, uBlur);

  vec2 lightDir = normalize(vec2(0.5, -0.7));
  float rimDot = abs(dot(grad, lightDir));
  float rimFalloff = 1.0 - smoothstep(0.0, bezel * 0.4, distFromEdge);
  float specHighlight = pow(rimDot * rimFalloff, 1.5);
  color += vec3(specHighlight * uSpecular);

  float innerShadow = 1.0 - smoothstep(0.0, bezel * 0.6, distFromEdge);
  color *= mix(1.0, 0.7, innerShadow * 0.3);

  float innerRim = smoothstep(0.0, 2.0, distFromEdge)
    * (1.0 - smoothstep(2.0, 5.0, distFromEdge));
  color += vec3(innerRim * 0.15 * uSpecular);
  color = mix(color, vec3(1.0), uTint);

  float alpha = smoothstep(0.0, 1.5, distFromEdge);
  finalColor = vec4(color * alpha, alpha);
}
`

/** All lengths in DESKTOP (CSS) px — the reference's unit. `k` = physical px
 *  per desktop px (renderScale), used only for px↔UV conversion in-shader. */
export interface GlassParams {
  centerX: number
  centerY: number
  halfW: number
  halfH: number
  radius: number
  thickness: number
  bezel: number
  ior: number
  blur: number
  specular: number
  tint: number
  shadow: number
  k: number
}

interface GlassUniforms {
  uCenter: Float32Array
  uHalf: Float32Array
  uRadius: number
  uThickness: number
  uBezel: number
  uIOR: number
  uBlur: number
  uSpecular: number
  uTint: number
  uShadow: number
  uK: number
}

export class LiquidGlassFilter extends Filter {
  constructor() {
    super({
      glProgram: GlProgram.from({ vertex: VERTEX, fragment: FRAGMENT }),
      resources: {
        uGlass: {
          uCenter: { value: new Float32Array([0, 0]), type: 'vec2<f32>' },
          uHalf: { value: new Float32Array([1, 1]), type: 'vec2<f32>' },
          uRadius: { value: 60, type: 'f32' },
          uThickness: { value: 50, type: 'f32' },
          uBezel: { value: 60, type: 'f32' },
          uIOR: { value: 3, type: 'f32' },
          uBlur: { value: 1.5, type: 'f32' },
          uSpecular: { value: 0.55, type: 'f32' },
          uTint: { value: 0.08, type: 'f32' },
          uShadow: { value: 0.5, type: 'f32' },
          uK: { value: 1, type: 'f32' },
        },
      },
    })
  }

  configure(p: GlassParams): void {
    const u = this.resources.uGlass.uniforms as unknown as GlassUniforms
    u.uCenter[0] = p.centerX
    u.uCenter[1] = p.centerY
    u.uHalf[0] = Math.max(1, p.halfW)
    u.uHalf[1] = Math.max(1, p.halfH)
    u.uRadius = Math.max(0, p.radius)
    u.uThickness = Math.max(0, p.thickness)
    u.uBezel = Math.max(0.001, p.bezel)
    u.uIOR = Math.max(1, p.ior)
    u.uBlur = Math.max(0, p.blur)
    u.uSpecular = Math.max(0, p.specular)
    u.uTint = Math.min(1, Math.max(0, p.tint))
    u.uShadow = Math.max(0, p.shadow)
    u.uK = Math.max(0.01, p.k)
  }
}
