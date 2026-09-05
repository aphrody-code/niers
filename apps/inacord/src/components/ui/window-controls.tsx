// Boutons de fenêtre custom (réduire/agrandir-restaurer/fermer) — la fenêtre est SANS bordure
// depuis `decorations: false` (`src-tauri/tauri.conf.json`), portage du frameless look de
// spacedrive/spaceui (cf. `var/spacedrive/docs/public/SDGridView.webp` : traffic lights macOS
// intégrées à la barre d'outils, pas de chrome natif séparé). Windows n'a pas de convention
// "traffic lights" — alignement à droite, forme et comportement natifs Windows 11 (Discord/VS
// Code/Spotify frameless), pas une imitation macOS hors de propos sur cette plateforme.
import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { Icon } from "@/components/ui/Icon";
import { cn } from "@/lib/utils";

/** La fenêtre Tauri courante, ou `null` hors de son runtime.
 *
 * Résolue à l'APPEL et non à l'import : `getCurrentWindow()` déréférence
 * `window.__TAURI_INTERNALS__` et jette une `TypeError` quand il n'y a pas de runtime. Au niveau
 * module, cette exception remontait la chaîne d'imports jusqu'à `App.tsx` et empêchait
 * l'application entière de se monter dans un navigateur — donc de la déboguer autrement qu'à
 * l'aveugle dans la fenêtre native. */
function fenetre() {
  try {
    return getCurrentWindow();
  } catch {
    return null;
  }
}

export function WindowControls({ className }: { className?: string }) {
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    const win = fenetre();
    if (!win) return;
    win.isMaximized().then(setMaximized).catch(() => {});
    const unlisten = win.onResized(() => {
      win.isMaximized().then(setMaximized).catch(() => {});
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  return (
    <div className={cn("flex items-center gap-0.5", className)}>
      <button
        type="button"
        className="flex h-8 w-8 items-center justify-center rounded-md text-ink-dull transition-colors hover:bg-app-hover hover:text-ink"
        title="Réduire"
        aria-label="Réduire"
        onClick={() => fenetre()?.minimize()}
      >
        <Icon name="remove" size={16} />
      </button>
      <button
        type="button"
        className="flex h-8 w-8 items-center justify-center rounded-md text-ink-dull transition-colors hover:bg-app-hover hover:text-ink"
        title={maximized ? "Restaurer" : "Agrandir"}
        aria-label={maximized ? "Restaurer" : "Agrandir"}
        onClick={() => fenetre()?.toggleMaximize()}
      >
        <Icon name={maximized ? "fullscreen_exit" : "crop_square"} size={14} />
      </button>
      <button
        type="button"
        className="flex h-8 w-8 items-center justify-center rounded-md text-ink-dull transition-colors hover:bg-status-error hover:text-white"
        title="Fermer"
        aria-label="Fermer"
        onClick={() => fenetre()?.close()}
      >
        <Icon name="close" size={16} />
      </button>
    </div>
  );
}
