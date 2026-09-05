// Résolution réactive « code de fichier VFS → nom réel » (perso/technique/objet), cf. demande
// utilisatrice « affiche le nom des joueurs, technique, objets etc lié à un fichier au lieu de
// juste l'id ». S'appuie sur `wikiDb.resolveManyByCode` (miroir local `supabase-*.sqlite`, cf.
// `SettingsView` → auto-détecté par défaut via `api.defaultWikiDb`). Sans miroir configuré,
// résout silencieusement vers rien (repli déjà en place : le nom de fichier brut reste affiché).
import { useEffect, useState } from "react";
import { wikiDb, type ResolvedName } from "@/lib/wikiDb";

/** Cache MODULE-LEVEL (partagé par tous les composants), clé `${dbPath}::${code}` — un code
 * donné (`c01000100`, `rhd10010`…) a un nom stable, jamais besoin de le résoudre deux fois. */
const cache = new Map<string, ResolvedName | null>();
const inFlight = new Set<string>();

/**
 * Résout un lot de `codes` (basenames VFS sans extension, cf. `vfsIndexDb.codeOf`) vers leur
 * personnage/technique/objet — une seule requête `IN (...)` par lot manquant, jamais une
 * requête par fichier affiché. Retourne la portion déjà connue du cache (peut grandir entre deux
 * rendus au fur et à mesure que la résolution en arrière-plan se termine).
 */
export function useResolvedNames(dbPath: string, codes: string[]): Map<string, ResolvedName> {
  const [, bump] = useState(0);

  useEffect(() => {
    if (!dbPath.trim() || codes.length === 0) return;
    const missing = codes.filter((c) => c && !cache.has(`${dbPath}::${c}`) && !inFlight.has(`${dbPath}::${c}`));
    if (missing.length === 0) return;
    missing.forEach((c) => inFlight.add(`${dbPath}::${c}`));

    wikiDb
      .resolveManyByCode(dbPath, missing)
      .then((resolved) => {
        for (const c of missing) {
          cache.set(`${dbPath}::${c}`, resolved.get(c) ?? null);
          inFlight.delete(`${dbPath}::${c}`);
        }
        bump((n) => n + 1);
      })
      .catch(() => {
        // Miroir absent/corrompu → on retient l'échec (évite de re-tenter en boucle), le nom de
        // fichier brut reste affiché en repli.
        for (const c of missing) {
          cache.set(`${dbPath}::${c}`, null);
          inFlight.delete(`${dbPath}::${c}`);
        }
        bump((n) => n + 1);
      });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [dbPath, codes]);

  const out = new Map<string, ResolvedName>();
  for (const c of codes) {
    const v = cache.get(`${dbPath}::${c}`);
    if (v) out.set(c, v);
  }
  return out;
}

/** Résolution ponctuelle d'un seul code (ex. `DetailPane`) — même cache que le lot. */
export function useResolvedName(dbPath: string, code: string | null): ResolvedName | null {
  const map = useResolvedNames(dbPath, code ? [code] : []);
  return code ? (map.get(code) ?? null) : null;
}
