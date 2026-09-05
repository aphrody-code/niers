// Pont entre la barre de menu native « Édition » (`App.tsx`) et la vue active qui possède
// réellement une sélection de fichiers/dossiers (`ExplorerView`) — cf. demande utilisatrice
// « editer doit vraiment copier coller et tout select les fichiers dossiers pas du texte ».
// Les `PredefinedMenuItem` Tauri (Copy/Cut/Paste/SelectAll) sont des commandes texte de l'OS :
// elles n'agissent que sur un champ de saisie natif FOCUSÉ, jamais sur une liste HTML custom —
// c'est pour ça qu'elles ne faisaient rien sur les listes de fichiers. Ce module remplace ces
// items par de VRAIS `MenuItem` dont l'action délègue à la vue active, sur le même principe que
// `places.ts` (état module-level + écouteurs, sans passer par un contexte React).
export interface FileOps {
  selectAll: () => void;
  copySelection: () => void;
  paste: () => void;
}

let current: FileOps | null = null;

/** Enregistre les opérations de la vue active (appelé par `ExplorerView` au montage/focus). */
export function registerFileOps(ops: FileOps | null): void {
  current = ops;
}

export function selectAllActive(): void {
  current?.selectAll();
}

export function copySelectionActive(): void {
  current?.copySelection();
}

export function pasteActive(): void {
  current?.paste();
}
