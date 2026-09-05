// Remplacement local de `next/image` — les composants du wiki (`components/wiki/*`, portés
// depuis `apps/azalee`) l'importent partout, et il n'existe pas hors de Next : sans ce shim,
// aucun d'eux ne compile ici.
//
// Ce que Next fait et que nous ne faisons PAS : l'optimisation serveur (redimensionnement,
// WebP, `srcset`). Elle n'a aucun sens dans une application de bureau — les images viennent soit
// du CDN du wiki, déjà dimensionnées, soit du VFS local via `lib/thumbs.ts`. Les props propres à
// Next (`priority`, `unoptimized`, `quality`, `loader`, `placeholder`, `blurDataURL`, `sizes`)
// sont acceptées puis ignorées, pour que le code porté reste identique à sa source.
import type { CSSProperties, ImgHTMLAttributes } from "react";

import { useSettings } from "@/lib/settings";
import { useThumbnail } from "@/lib/thumbs";
import { estCheminVfs } from "@/lib/wikiImages";
import { cn } from "@/lib/utils";

export interface ImageProps
  extends Omit<ImgHTMLAttributes<HTMLImageElement>, "width" | "height" | "placeholder"> {
  src: string;
  alt: string;
  width?: number | string;
  height?: number | string;
  /** `next/image` : remplit le parent positionné. Rendu ici par un `position:absolute` + `inset:0`. */
  fill?: boolean;
  priority?: boolean;
  unoptimized?: boolean;
  quality?: number;
  sizes?: string;
  placeholder?: string;
  blurDataURL?: string;
}

/**
 * Image dont la source est un fichier du VFS (`data/dx11/menu/…/x_l.g4tx`) : le décodage passe
 * par `lib/thumbs` — vignette bornée côté Rust, cache LRU, file de concurrence et chargement
 * différé à l'entrée dans le viewport. C'est la même mécanique que les grilles de l'Explorateur,
 * pas un second chemin de décodage.
 */
function ImageVfs({ src, alt, width, height, fill, className, style, ...rest }: ImageProps) {
  const settings = useSettings();
  const ext = src.split(".").pop() ?? "";
  const { ref, src: decodee } = useThumbnail(src, ext, settings.gameDir);
  return (
    <span
      ref={ref}
      className={cn("inline-flex items-center justify-center overflow-hidden", className)}
      style={fill ? { position: "absolute", inset: 0, ...style } : { width, height, ...style }}
    >
      {decodee && (
        <img src={decodee} alt={alt} className="h-full w-full object-contain" {...rest} />
      )}
    </span>
  );
}

export function Image({
  src,
  alt,
  width,
  height,
  fill,
  className,
  style,
  // Props Next-only : acceptées pour la compatibilité de source, sans effet ici.
  priority: _priority,
  unoptimized: _unoptimized,
  quality: _quality,
  sizes: _sizes,
  placeholder: _placeholder,
  blurDataURL: _blurDataURL,
  ...rest
}: ImageProps) {
  // Un chemin du VFS n'est pas une URL : il doit être décodé (g4tx → PNG) avant d'être affiché.
  if (estCheminVfs(src)) {
    return (
      <ImageVfs src={src} alt={alt} width={width} height={height} fill={fill} className={className} style={style} {...rest} />
    );
  }
  const styleFill: CSSProperties | undefined = fill
    ? { position: "absolute", inset: 0, width: "100%", height: "100%", ...style }
    : style;
  return (
    <img
      src={src}
      alt={alt}
      width={fill ? undefined : width}
      height={fill ? undefined : height}
      className={cn(className)}
      style={styleFill}
      // Une image du CDN absente ne doit pas laisser l'icône « image cassée » du navigateur au
      // milieu d'une carte : on masque l'élément, la carte garde sa mise en page.
      onError={(e) => {
        (e.currentTarget as HTMLImageElement).style.visibility = "hidden";
      }}
      {...rest}
    />
  );
}

export default Image;
