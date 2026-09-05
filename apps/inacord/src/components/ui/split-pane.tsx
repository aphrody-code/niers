// Panneaux redimensionnables — la base du layout d'un éditeur (Unreal, Unity, Blender : tout est
// une grille de panneaux qu'on tire).
//
// Écrit ici plutôt que porté de `spaceui/primitives/Resizable.tsx` : celui-ci dépend du paquet
// `react-resizable-layout` (absent du projet) et ne gère qu'un panneau de tête sur un seul axe.
// L'éditeur a besoin des deux axes ET d'un panneau de queue (l'inspecteur à droite se
// redimensionne par son bord GAUCHE, pas droit).
import { useCallback, useEffect, useRef, useState } from "react";

import { cn } from "@/lib/utils";

export interface SplitPaneProps {
  /** Axe de séparation : `x` = deux colonnes, `y` = deux lignes. */
  axis?: "x" | "y";
  /** Panneau dont la taille est pilotée : `start` (haut/gauche) ou `end` (bas/droite). */
  side?: "start" | "end";
  /** Taille initiale du panneau piloté, en px. */
  defaultSize: number;
  min?: number;
  max?: number;
  /** Clé de persistance (localStorage) — un layout d'éditeur doit survivre au redémarrage. */
  storageKey?: string;
  /** Panneau piloté (celui qui a `size`). */
  panel: React.ReactNode;
  /** Panneau qui prend le reste de la place. */
  children: React.ReactNode;
  className?: string;
}

function loadSize(key: string | undefined, fallback: number): number {
  if (!key) return fallback;
  const raw = localStorage.getItem(`nie-explorer:split:${key}`);
  const n = raw ? Number(raw) : NaN;
  return Number.isFinite(n) ? n : fallback;
}

export function SplitPane({
  axis = "x",
  side = "start",
  defaultSize,
  min = 120,
  max = 900,
  storageKey,
  panel,
  children,
  className,
}: SplitPaneProps) {
  const [size, setSize] = useState(() => loadSize(storageKey, defaultSize));
  const dragging = useRef(false);
  const hostRef = useRef<HTMLDivElement | null>(null);

  const onPointerDown = useCallback((e: React.PointerEvent) => {
    e.preventDefault();
    dragging.current = true;
    (e.target as HTMLElement).setPointerCapture(e.pointerId);
    // Curseur forcé sur tout le document : sans ça, il redevient une flèche dès que le pointeur
    // quitte la poignée de quelques pixels pendant le glissé.
    document.body.style.setProperty("cursor", axis === "x" ? "col-resize" : "row-resize", "important");
    document.body.style.userSelect = "none";
  }, [axis]);

  const onPointerMove = useCallback(
    (e: React.PointerEvent) => {
      if (!dragging.current || !hostRef.current) return;
      const rect = hostRef.current.getBoundingClientRect();
      let next: number;
      if (axis === "x") {
        next = side === "start" ? e.clientX - rect.left : rect.right - e.clientX;
      } else {
        next = side === "start" ? e.clientY - rect.top : rect.bottom - e.clientY;
      }
      setSize(Math.max(min, Math.min(max, Math.round(next))));
    },
    [axis, side, min, max],
  );

  const endDrag = useCallback(() => {
    if (!dragging.current) return;
    dragging.current = false;
    document.body.style.removeProperty("cursor");
    document.body.style.userSelect = "";
    if (storageKey) localStorage.setItem(`nie-explorer:split:${storageKey}`, String(size));
  }, [size, storageKey]);

  // Un pointerup hors de la poignée (relâché au-dessus d'un autre panneau) doit terminer le
  // glissé : sans ce filet, la poignée reste « collée » au curseur.
  useEffect(() => {
    window.addEventListener("pointerup", endDrag);
    return () => window.removeEventListener("pointerup", endDrag);
  }, [endDrag]);

  const handle = (
    <div
      role="separator"
      aria-orientation={axis === "x" ? "vertical" : "horizontal"}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={endDrag}
      className={cn(
        "group relative shrink-0 bg-app-line/40 transition-colors hover:bg-accent/60",
        axis === "x" ? "w-px cursor-col-resize" : "h-px cursor-row-resize",
      )}
    >
      {/* Zone de saisie plus large que le trait visible — un séparateur d'1 px est intenable
       * à la souris. */}
      <div
        className={cn("absolute", axis === "x" ? "inset-y-0 -left-1 w-3" : "inset-x-0 -top-1 h-3")}
      />
    </div>
  );

  // `flex flex-col` sur les deux enveloppes, et pas seulement `overflow-hidden` : en `display:
  // block`, un enfant qui se dit `flex-1` ne reçoit AUCUNE contrainte de hauteur — `flex-1` ne
  // veut rien dire hors d'un conteneur flex — et retombe sur sa hauteur automatique. Mesuré dans
  // l'Explorateur : l'enveloppe faisait bien 836 px, son enfant 197 px, et la zone de liste
  // qu'il contient 57 px. Une zone défilante qui grandit avec son contenu ne déborde jamais :
  // son `overflow: scroll` n'a alors rien à faire défiler, et tout ce qui dépasse est coupé par
  // l'`overflow-hidden` ci-dessous. C'est la cause du « scroll impossible » dans toutes les vues
  // bâties sur ce composant — Explorateur, Éditeur, Lua.
  const sized = (
    <div
      className="flex min-h-0 min-w-0 shrink-0 flex-col overflow-hidden"
      style={axis === "x" ? { width: size } : { height: size }}
    >
      {panel}
    </div>
  );

  return (
    <div ref={hostRef} className={cn("flex min-h-0 min-w-0", axis === "x" ? "flex-row" : "flex-col", className)}>
      {side === "start" ? (
        <>
          {sized}
          {handle}
          <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">{children}</div>
        </>
      ) : (
        <>
          <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">{children}</div>
          {handle}
          {sized}
        </>
      )}
    </div>
  );
}
