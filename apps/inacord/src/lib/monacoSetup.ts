// Bundle Monaco EN LOCAL (pas de CDN) — cf. demande utilisatrice « le text editeur de vs code
// pour les fichiers cfg.bin, json etc » + roadmap §2.1 (« offline, bundle local, pas de CDN »).
// L'app tourne hors ligne (Tauri desktop, aucun serveur) : `@monaco-editor/react` charge Monaco
// depuis un CDN PAR DÉFAUT (`loader.config`) — comportement inacceptable ici, corrigé en pointant
// le loader vers le paquet npm `monaco-editor` importé statiquement (bundlé par Vite avec le
// reste de l'app).
//
// Web Workers : Monaco a besoin d'un worker par langage pour la coloration/validation avancée.
// Vite 5+ supporte nativement les imports `?worker` (aucun plugin monaco-vite requis, cf. exemple
// officiel monaco-editor pour Vite) — `self.MonacoEnvironment.getWorker` route vers le bon worker
// selon le `label` demandé par Monaco.
import * as monaco from "monaco-editor";
import { loader } from "@monaco-editor/react";
import EditorWorker from "monaco-editor/editor/editor.worker?worker";
import JsonWorker from "monaco-editor/language/json/json.worker?worker";

let installed = false;

/** Idempotent — sûr à appeler depuis plusieurs composants montés en parallèle. */
export function installMonacoOffline(): void {
  if (installed) return;
  installed = true;

  self.MonacoEnvironment = {
    getWorker(_workerId: string, label: string) {
      if (label === "json") return new JsonWorker();
      return new EditorWorker();
    },
  };

  loader.config({ monaco });
}
