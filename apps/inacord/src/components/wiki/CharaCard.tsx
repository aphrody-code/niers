// Carte de personnage — la seule carte du dossier `wiki/` qui n'existe pas côté site : le wiki
// rend ses personnages dans un tableau (`CharacterTable`) et ses visages depuis un CDN.
//
// Ici le portrait vient du **VFS local** (`data/dx11/menu/200_icon/10_icon_chr/face/<code>_l.g4tx`,
// décodé par `ui/image.tsx` → `lib/thumbs`), donc hors ligne et à la source. La mise en page
// reprend celle des autres cartes portées (cadre arrondi, dégradé de survol, pastilles).
import { Image } from "@/components/ui/image";
import { getCharacterFaceUrl } from "@/lib/wikiImages";
import { cn } from "@/lib/utils";

export interface CharaCardProps {
  /** Code interne (`c01000010`) — sert au portrait ET à l'éditeur de propriétés. */
  code: string;
  name: string;
  element?: string;
  mainPosition?: string;
  subPosition?: string;
  team?: string | null;
  series?: string | null;
  /** Somme des sept stats à Lv99 (base de comparaison, cf. `CharaDto::stats`). */
  total?: number;
  skillCount?: number;
  selected?: boolean;
  onClick?: () => void;
  className?: string;
}

/**
 * Teintes d'élément. `ElementIcon` (porté du wiki) rend `null` ici : son icône vit dans un ATLAS
 * du jeu qu'aucun index ne découpe (cf. `lib/wikiImages.ts`), donc l'élément DISPARAISSAIT
 * silencieusement de la carte — vérifié à l'écran. Une pastille colorée le rend lisible sans
 * prétendre afficher l'icône officielle.
 */
const ELEMENTS: Record<string, string> = {
  Feu: "bg-red-500/20 text-red-300",
  Vent: "bg-emerald-500/20 text-emerald-300",
  Forêt: "bg-lime-600/20 text-lime-300",
  Montagne: "bg-amber-600/20 text-amber-300",
  Néant: "bg-violet-500/20 text-violet-300",
};

/** Teintes de poste — mêmes familles de couleur que les rôles du constructeur d'équipe. */
const POSTES: Record<string, string> = {
  FW: "bg-red-500/15 text-red-300",
  MF: "bg-emerald-500/15 text-emerald-300",
  DF: "bg-sky-500/15 text-sky-300",
  GK: "bg-amber-500/15 text-amber-300",
};

export function CharaCard({
  code,
  name,
  element,
  mainPosition,
  subPosition,
  team,
  series,
  total,
  skillCount,
  selected,
  onClick,
  className,
}: CharaCardProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      title={`${name} — ${code}`}
      className={cn(
        "group flex w-full flex-col items-stretch gap-2 rounded-xl border p-2 text-left transition-colors",
        selected
          ? "border-accent bg-accent/10"
          : "border-app-line bg-app-box/60 hover:border-accent/40 hover:bg-app-hover",
        className,
      )}
    >
      <div className="relative flex h-24 items-center justify-center overflow-hidden rounded-lg bg-app-darkBox/60">
        <Image src={getCharacterFaceUrl(code)} alt={name} width={96} height={96} />
        {mainPosition && (
          <span
            className={cn(
              "absolute left-1 top-1 rounded px-1.5 py-0.5 text-[10px] font-bold",
              POSTES[mainPosition] ?? "bg-app-selected/30 text-ink-dull",
            )}
          >
            {mainPosition}
            {subPosition && subPosition !== "—" ? ` / ${subPosition}` : ""}
          </span>
        )}
        {element && (
          <span
            className={cn(
              "absolute right-1 top-1 rounded px-1.5 py-0.5 text-[10px] font-bold",
              ELEMENTS[element] ?? "bg-app-selected/30 text-ink-dull",
            )}
          >
            {element}
          </span>
        )}
      </div>

      <div className="min-w-0">
        <div className="truncate text-sm font-semibold text-ink">{name}</div>
        <div className="truncate text-[11px] text-ink-faint">{team ?? series ?? code}</div>
      </div>

      <div className="flex items-center justify-between text-[11px] text-ink-dull">
        {/* « Lv99 UR » est la base de comparaison commune, pas la fiche d'un exemplaire — cf. la
         * note de `CharaDto::stats` côté Rust. */}
        <span title="Somme des 7 stats au niveau 99, rang UR">
          {total ? `${total.toLocaleString("fr-FR")} pts` : "—"}
        </span>
        <span title="Techniques apprises">{skillCount ?? 0} tech.</span>
      </div>
    </button>
  );
}
