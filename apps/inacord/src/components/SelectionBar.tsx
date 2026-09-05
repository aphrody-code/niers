// Barre flottante de multi-sélection — apparaît au-dessus de la liste dès qu'au moins un élément
// est coché (Ctrl/Maj-clic, Ctrl+A) et porte les actions de LOT.
//
// Elle COMPLÈTE la ligne de statut de l'Explorateur, elle ne la remplace pas : le statut décrit le
// dossier (compteur, chargement, résultats de recherche) et reste lisible en permanence ; cette
// barre décrit la SÉLECTION et n'existe que tant qu'elle existe. Les fusionner obligerait à faire
// disparaître le compte du dossier au moment précis où l'on travaille dessus.
import { Icon } from "@/components/ui/Icon";
import { humanSize } from "@/lib/bytes";

export function SelectionBar({
  count,
  totalSize,
  onClear,
  onCopyPaths,
  onStageIntoMod,
  onExport,
}: {
  count: number;
  /** Somme des tailles des FICHIERS sélectionnés (un dossier VFS n'a pas de taille propre). */
  totalSize: number;
  onClear: () => void;
  onCopyPaths: () => void;
  onStageIntoMod: () => void;
  /** Export en lot vers un dossier, au format choisi (cf. `api.exportMany`). */
  onExport?: () => void;
}) {
  if (count === 0) return null;

  return (
    <div className="pointer-events-none absolute bottom-4 left-1/2 z-20 -translate-x-1/2">
      <div className="pointer-events-auto flex items-center gap-1 rounded-full border border-app-line bg-app-box/95 py-1 pl-3 pr-1 shadow-lg backdrop-blur">
        <span className="whitespace-nowrap type-label-small text-on-surface">
          {count.toLocaleString("fr-FR")} sélectionné(s)
          {totalSize > 0 && <span className="text-on-surface-variant"> · {humanSize(totalSize)}</span>}
        </span>
        <span className="mx-1 h-4 w-px bg-app-line" />
        <SelectionAction icon="content_copy" label="Copier les chemins" onClick={onCopyPaths} />
        {onExport && (
          <SelectionAction icon="download" label="Exporter au format…" onClick={onExport} />
        )}
        <SelectionAction icon="extension" label="Ajouter à un mod…" onClick={onStageIntoMod} />
        <SelectionAction icon="close" label="Tout désélectionner" onClick={onClear} />
      </div>
    </div>
  );
}

function SelectionAction({ icon, label, onClick }: { icon: string; label: string; onClick: () => void }) {
  return (
    <button
      type="button"
      title={label}
      aria-label={label}
      className="flex size-7 items-center justify-center rounded-full text-ink-dull transition-colors hover:bg-app-hover hover:text-ink"
      onClick={onClick}
    >
      <Icon name={icon} size={15} />
    </button>
  );
}
