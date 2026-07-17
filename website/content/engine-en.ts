import type { EngineDict } from "./engine-types";

const GH = "https://github.com/nicepkg/deskmakeover";
const CORE = `${GH}/tree/main/crates/dm-icon-core`;
const BLOB = `${GH}/blob/main/crates/dm-icon-core`;

export const ENGINE_EN: EngineDict = {
  meta: {
    title: "The Pixel Engine: how DeskMakeover reads and repaints your icons",
    description:
      "Portrait, separation, rescue, finish: a ~12,000-line purpose-built, fully deterministic pure-Rust pipeline that renders byte-identical pixels on the desktop and in your browser. Watch it run step by step, then drive the same WASM build yourself.",
  },
  hero: {
    eyebrow: "ENGINE · DM-ICON-CORE",
    title: "It reads your icons before it touches them",
    sub: "Behind every look is one purpose-built, fully deterministic pixel pipeline: it studies each icon first, then repaints it under hard rules. Zero ML, zero uploads; the same input always yields the same pixels. This page runs it in front of you, then hands you the controls.",
    stats: [
      { value: 11946, unit: " lines", label: "of pure-Rust pixel core" },
      { value: 1487, unit: " icons", label: "in the byte-parity corpus" },
      { value: 57, unit: " tests", label: "over the core algorithms" },
      { value: 88, unit: " KB", label: "the same engine, in this page" },
    ],
  },
  portrait: {
    index: "01",
    kicker: "PORTRAIT",
    title: "A portrait, before a single pixel moves",
    body: "Before anything is restyled, the pipeline reads. A five-step classification decides what the icon is; probes along the canvas edge and a shape ring find any background it brought along; the dominant colour and hue spread are measured in OKLab; a silhouette match confirms whether it already is a standard shape. Every icon gets a profile, and every later stage reads that profile instead of guessing again.",
    steps: [
      { key: "CLASSIFY", label: "Classify" },
      { key: "BACKGROUND", label: "Background" },
      { key: "COLOR", label: "Colour" },
      { key: "SILHOUETTE", label: "Silhouette" },
      { key: "PROFILE", label: "Profile" },
    ],
    probeCaption:
      "Sampling probes on the canvas-edge and shape rings. A corner-symmetry check rejects dog-eared document pages.",
    iouLabel: "Silhouette match threshold",
  },
  separate: {
    index: "02",
    kicker: "SEPARATE",
    title: "What is subject, what is ground",
    body: "Icons with transparent edges give up their silhouette directly. Opaque ones get a border-seeded flood that eats the background inward, layer by layer, under a local tolerance. Plate-like icons take one more step: an Otsu split over colour distance separates the plate from the artwork.",
    stages: [
      { title: "Alpha silhouette", detail: "A transparent-edged icon's subject is its opaque pixels" },
      { title: "Border flood", detail: "BFS spreads in from edge seeds; tolerance is relative to the last pixel" },
      { title: "Otsu split", detail: "Plate-like silhouettes divide into plate and artwork by colour distance" },
    ],
    floodCaption: "The flood advances in discrete layers. That is the algorithm's real gait, not a transition.",
    replay: "Replay",
  },
  rescue: {
    index: "03",
    kicker: "RESCUE",
    title: "When a subject starts to melt",
    body: "On a fresh plate, the subject's colour can sit too close to it. At 32 to 48 pixels, hue contrast alone is unreliable: at that spatial frequency the eye resolves chroma far worse than lightness. So the rescue samples the subject's outer rim, judges the colour you actually see after compositing, and runs paired OKLab-distance and lightness thresholds; once too large a share of the rim melts, an outline and shadow are added. A share, not a mean, so a bimodal rim cannot hide.",
    beats: [
      { key: "DANGER", title: "Danger", detail: "The subject's colour hugs the new plate; its edge is about to vanish" },
      { key: "DETECT", title: "Detect", detail: "Rim probes judge point by point, on the composited colour you see" },
      { key: "RESCUE", title: "Rescue", detail: "An outline draws on, a soft shadow lands, the subject is back" },
    ],
    gauges: { deltaE: "OKLab distance", deltaL: "Lightness gap", melt: "Melt share" },
    caption: "Only both thresholds failing counts. Judged at source resolution, so every preview size reaches the same verdict.",
  },
  invariant: {
    index: "04",
    kicker: "INVARIANT",
    title: "Your brand colour stays your brand colour",
    body: "One rule cannot be crossed anywhere in the pipeline: subject pixels are never recoloured. Distinction comes from plate, outline, and shadow. When derived plate hues collide, the engine rotates them apart on the wheel deterministically, and the same artwork always lands on the same plate.",
    rule: "Subject pixels are never recoloured",
    ruleNote: "A hard rule in compose/field.rs. No look gets around it.",
    wheelCaption: "A gap of at least about 12 degrees, a rotation never past 18. Same input, same rotation, no randomness.",
  },
  color: {
    index: "05",
    kicker: "COLOR",
    title: "Colour is computed, not picked",
    body: "All colour math runs in linear-light sRGB and OKLab. The mono look maps through a Material-style tonal duotone, a 256-step ramp laid out end to end. Continuous corners are a faithful port of Figma's open-source smoothing math: per corner, one arc and two tangent cubics. Scaling is a true area average in linear light with premultiplied alpha. Composed, not reinvented.",
    points: [
      { title: "OKLab throughout", detail: "Distance, harmony bands and contrast live in a perceptually uniform space" },
      { title: "256-step duotone", detail: "The mono look's lights and darks map through one tonal ramp" },
      { title: "Squircle math", detail: "A faithful port of Figma's MIT smoothing algorithm, credited" },
      { title: "Linear-light resampling", detail: "Downscales are true area averages; upscales are 4×4 supersampled" },
    ],
  },
  finish: {
    index: "06",
    kicker: "FINISH",
    title: "One icon, three textures",
    body: "A finish is an algorithm, not an overlay. Each one is a deterministic computation over pixels, run fresh for every icon.",
    finishes: [
      {
        key: "glass",
        kicker: "GLASS",
        name: "Glass",
        recipe: ["A translucent slab body", "Fresnel highlights and rim refraction", "A grounding halo just outside the slab"],
      },
      {
        key: "pixel",
        kicker: "PIXEL",
        name: "Pixel",
        recipe: ["Linear-light average per grid cell", "Mapped to a candy palette", "A contour ring drawn along the outline"],
      },
      {
        key: "sticker",
        kicker: "STICKER",
        name: "Sticker",
        recipe: ["The artwork shrinks a step", "A white die-cut border grows from the chamfer distance", "A soft shadow falls from the same distance field"],
      },
    ],
  },
  guarantee: {
    index: "07",
    kicker: "GUARANTEE",
    title: "Reproducible honesty",
    body: "Power is only worth shipping if it fails closed. Degenerate inputs have a whole test family; every buffer size is overflow-checked; render size has a hard cap; errors return codes, never panics. And the desktop app snapshots your desktop before touching it, so restore is always one click.",
    items: [
      { title: "Never panics", detail: "Empty input, zero sizes and overflowing sizes all return error codes" },
      { title: "Overflow-checked", detail: "Every buffer allocation goes through checked multiplication" },
      { title: "Hard caps", detail: "An oversized render is refused outright; nothing writes past a buffer" },
      { title: "Snapshot first", detail: "The desktop is backed up before any change; restore is one click" },
    ],
    receiptsLead: "Every number links to the exact source. This page does not ask for trust, just a click.",
    receipts: [
      { value: "11,946 lines", label: "pure-Rust pixel core", href: CORE },
      { value: "forbid(unsafe_code)", label: "unsafe banned in the core", href: `${BLOB}/src/lib.rs` },
      { value: "1,487 icons", label: "byte-parity corpus", href: `${BLOB}/tests/parity_determinism.rs` },
      { value: "57 tests", label: "over the core algorithms", href: `${CORE}/tests` },
      { value: "88 KB", label: "the WASM build (gzip)", href: `${GH}/tree/main/crates/dm-icon-wasm` },
      { value: "MIT", label: "all of it open source", href: `${GH}/blob/main/LICENSE` },
    ],
  },
  playground: {
    index: "08",
    kicker: "LIVE",
    title: "Now you drive it",
    body: "This canvas is powered by dm-icon-wasm: the same Rust pipeline the desktop app ships, compiled into an 88 KB module running in your browser. Every frame is computed on the spot, not a recording. Pick a sample icon or drop in your own image, then drag the controls and watch the pixels answer.",
    badge: "Byte-identical with the desktop build",
    sampleLabel: "Sample icons",
    uploadCta: "Try your own image",
    uploadNote: "Your image is processed in your browser and never leaves it.",
    controls: {
      shape: "Shape",
      look: "Look",
      hue: "Plate hue",
      finish: "Finish",
      original: "Compare original",
    },
    options: {
      shapes: [
        { tag: "Apple", label: "Squircle" },
        { tag: "Circle", label: "Circle" },
        { tag: "Tile", label: "Tile" },
        { tag: "Diamond", label: "Diamond" },
        { tag: "Flower", label: "Flower" },
        { tag: "Pebble", label: "Pebble" },
      ],
      looks: [
        { tag: "Original", label: "Original" },
        { tag: "BlackWhite", label: "Black & white" },
        { tag: "Mono", label: "Mono" },
      ],
      finishes: [
        { tag: "None", label: "None" },
        { tag: "Glass", label: "Glass" },
        { tag: "Pixel", label: "Pixel" },
        { tag: "Sticker", label: "Sticker" },
      ],
    },
    autoHue: "Auto",
    loading: "Loading the engine",
    fallbackNote: "This browser cannot run WASM; below is a pre-rendered result sheet.",
  },
  cta: {
    title: "It is already waiting for your desktop",
    body: "This pipeline is not a demo piece. It is the exact code DeskMakeover runs on real desktops every day.",
    download: "Download DeskMakeover",
    github: "Read the source on GitHub",
  },
};
