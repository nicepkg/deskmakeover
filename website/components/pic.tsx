import { img } from "@/lib/manifest";

export function Pic({
  id,
  alt,
  sizes = "100vw",
  priority = false,
  className,
  imgClassName,
}: {
  id: string;
  alt: string;
  sizes?: string;
  priority?: boolean;
  className?: string;
  imgClassName?: string;
}) {
  const entry = img(id);
  const avif = entry.variants.map((v) => `${v.avif} ${v.w}w`).join(", ");
  const webp = entry.variants.map((v) => `${v.webp} ${v.w}w`).join(", ");
  const fallback = entry.variants[entry.variants.length - 1].webp;
  return (
    <picture className={className}>
      <source type="image/avif" srcSet={avif} sizes={sizes} />
      <source type="image/webp" srcSet={webp} sizes={sizes} />
      {/* eslint-disable-next-line @next/next/no-img-element */}
      <img
        src={fallback}
        width={entry.w}
        height={entry.h}
        alt={alt}
        loading={priority ? "eager" : "lazy"}
        fetchPriority={priority ? "high" : undefined}
        decoding="async"
        className={imgClassName}
      />
    </picture>
  );
}
