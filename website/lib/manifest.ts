import manifest from "./image-manifest.json";

export interface ImageVariant {
  w: number;
  h: number;
  avif: string;
  webp: string;
}

export interface ImageEntry {
  w: number;
  h: number;
  variants: ImageVariant[];
}

interface ManifestMeta {
  textures: Record<string, string>;
}

const entries = manifest as unknown as Record<string, ImageEntry | ManifestMeta>;

export function img(key: string): ImageEntry {
  const entry = entries[key];
  if (!entry || key === "__meta") throw new Error(`image-manifest: unknown key "${key}"`);
  return entry as ImageEntry;
}

export function texture(key: string): string {
  const meta = entries.__meta as ManifestMeta | undefined;
  const url = meta?.textures?.[key];
  if (!url) throw new Error(`image-manifest: unknown texture "${key}"`);
  return url;
}
