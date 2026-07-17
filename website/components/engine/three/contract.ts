/**
 * Contract between the /engine/ React wrappers and the three.js scene module
 * (scenes3d.ts). Wrappers own DOM (captions, chips, buttons, HTML labels) and
 * asset preparation; scenes own everything inside the canvas.
 */

export interface LabelPoint {
  /** canvas-relative CSS px */
  x: number;
  y: number;
  visible: boolean;
}

export interface SceneHandle {
  dispose(): void;
  /** re-run the scene's entrance choreography from the start */
  replay(): void;
  /** optional named states driven by wrapper UI (e.g. "before"/"after", "off"/"on") */
  setState?(name: string): void;
}

export interface SceneCommonOpts {
  reduceMotion: boolean;
  /** stream label anchor positions every frame; ids are scene-specific */
  onLabel?: (id: string, pt: LabelPoint) => void;
}

export type SceneInit<A> = (canvas: HTMLCanvasElement, assets: A, opts: SceneCommonOpts) => SceneHandle;

/** hero — the layer explosion (bottom → top: raw, plate, final) */
export interface HeroAssets {
  raw: ImageData;
  plate: ImageData;
  final: ImageData;
}

/** read — the checkup: scan sweep + extracted readouts */
export interface ReadAssets {
  icon: ImageData;
  /** white edge-outline texture computed from the icon's alpha */
  outline: ImageData;
  /** the engine's real decode-time hue seed for the colour chip */
  seedHex: string;
}

/** cut — the icon's own background peels away as its own layer */
export interface CutAssets {
  /** the icon's pixels the engine judged background (rest transparent) */
  bgLayer: ImageData;
  /** the icon's pixels the engine judged artwork (rest transparent) */
  artLayer: ImageData;
  /** the finished tile (artwork on its derived plate) */
  final: ImageData;
}

/** rescue — the exact pixels the rescue added, as a separable layer */
export interface RescueAssets {
  off: ImageData;
  on: ImageData;
  /** on − off: the outline + shadow the engine drew, alone */
  rescueLayer: ImageData;
}

/** promise — three tiles; plates change, artwork never does */
export interface PromiseAssets {
  items: {
    /** raw artwork layer (identical in both states) */
    art: ImageData;
    /** plate-only tile, collided colour */
    plateBefore: ImageData;
    /** plate-only tile, spread colour */
    plateAfter: ImageData;
  }[];
}
