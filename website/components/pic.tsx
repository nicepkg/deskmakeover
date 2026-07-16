import manifest from "@/lib/image-manifest.json";

type ManifestEntry = {
  w: number;
  h: number;
  variants: { w: number; h: number; avif: string; webp: string }[];
};

const images = manifest as Record<string, ManifestEntry>;

interface PicProps {
  name: string;
  alt: string;
  sizes?: string;
  eager?: boolean;
  className?: string;
  imgClassName?: string;
}

/**
 * Static <picture> backed by the build-time image manifest: AVIF + WebP
 * sources, intrinsic dimensions (zero CLS), lazy by default.
 */
export function Pic({ name, alt, sizes, eager = false, className, imgClassName }: PicProps) {
  const entry = images[name];
  if (!entry) throw new Error(`Pic: unknown image key "${name}" — run scripts/build-images.mjs`);
  const srcSet = (kind: "avif" | "webp") => entry.variants.map((v) => `${v[kind]} ${v.w}w`).join(", ");
  const fallback = entry.variants[0];
  return (
    <picture className={className}>
      {entry.variants.length > 1 ? (
        <>
          <source type="image/avif" srcSet={srcSet("avif")} sizes={sizes} />
          <source type="image/webp" srcSet={srcSet("webp")} sizes={sizes} />
        </>
      ) : (
        <source type="image/avif" srcSet={fallback.avif} />
      )}
      <img
        src={fallback.webp}
        width={entry.w}
        height={entry.h}
        alt={alt}
        loading={eager ? "eager" : "lazy"}
        decoding={eager ? "sync" : "async"}
        fetchPriority={eager ? "high" : undefined}
        className={imgClassName}
      />
    </picture>
  );
}

export function imageEntry(name: string): ManifestEntry {
  const entry = images[name];
  if (!entry) throw new Error(`imageEntry: unknown image key "${name}"`);
  return entry;
}
