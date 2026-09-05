// Journal DURABLE des opérations longues (§8 ROADMAP — « job system durable inspiré spacedrive »,
// point noté comme prochaine étape concrète). Table `jobs` de `mods.db` (migration v3,
// `tauri-plugin-sql` — pas de nouvelle dépendance `rusqlite`, cf. la contrainte de lien natif
// documentée dans `Cargo.toml`).
//
// Ce que ça change concrètement : la progression d'une réindexation (~255 800 fichiers, plusieurs
// minutes) ne vivait que dans un `useState`. Fermer l'app, changer d'onglet ou recharger la
// perdait sans trace, et un job coupé en cours de route était indiscernable d'un job jamais lancé.
// Désormais chaque opération laisse une ligne : état, avancement, erreur, horodatage.
//
// Ce que ça ne fait PAS (limite assumée, à ne pas laisser croire) : il n'y a pas de REPRISE
// automatique. Le moteur d'exécution reste `nie-tasks` côté Rust, en mémoire process — un job
// interrompu est marqué `interrupted` au démarrage suivant et doit être relancé à la main. La
// reprise demanderait de rendre chaque tâche redémarrable à partir d'un point de contrôle
// persisté, ce qui n'est pas le cas aujourd'hui.
import Database from "@tauri-apps/plugin-sql";
import { useSyncExternalStore } from "react";

export type JobStatus = "running" | "done" | "error" | "canceled" | "interrupted";

export interface JobRow {
  id: string;
  kind: string;
  label: string;
  status: JobStatus;
  progress: number;
  total: number;
  error: string | null;
  created_at: string;
  updated_at: string;
}

let dbPromise: Promise<Database> | null = null;
function db(): Promise<Database> {
  return (dbPromise ??= Database.load("sqlite:mods.db"));
}

// Cache en mémoire + abonnés : l'UI (popover du gestionnaire de jobs) doit se rafraîchir à chaque
// mise à jour de progression sans repasser par un `setInterval` de sondage SQL.
let cache: JobRow[] = [];
const listeners = new Set<() => void>();

function notify() {
  listeners.forEach((l) => l());
}

async function refresh(): Promise<JobRow[]> {
  const d = await db();
  cache = await d.select<JobRow[]>("SELECT * FROM jobs ORDER BY created_at DESC LIMIT 50");
  notify();
  return cache;
}

export const jobsDb = {
  /** Recharge le journal depuis SQLite (après une opération externe, ex. purge). */
  refresh,

  /**
   * Marque `interrupted` tout job resté `running` d'une session précédente. À appeler UNE fois au
   * démarrage : sans process pour les poursuivre, ces jobs ne peuvent pas rester « en cours » —
   * les laisser tels quels afficherait éternellement une opération fantôme dans le gestionnaire.
   */
  async reconcileOnStartup(): Promise<number> {
    const d = await db();
    const res = await d.execute(
      "UPDATE jobs SET status = 'interrupted', updated_at = datetime('now') WHERE status = 'running'",
    );
    await refresh();
    return res.rowsAffected;
  },

  async create(kind: string, label: string, total = 0): Promise<string> {
    const id = crypto.randomUUID();
    const d = await db();
    await d.execute("INSERT INTO jobs (id, kind, label, status, progress, total) VALUES ($1, $2, $3, 'running', 0, $4)", [
      id,
      kind,
      label,
      total,
    ]);
    await refresh();
    return id;
  },

  /** Avancement. Volontairement sans `refresh()` complet : appelé très souvent (chaque lot). */
  async progress(id: string, done: number, total: number): Promise<void> {
    const d = await db();
    await d.execute("UPDATE jobs SET progress = $1, total = $2, updated_at = datetime('now') WHERE id = $3", [
      done,
      total,
      id,
    ]);
    const row = cache.find((j) => j.id === id);
    if (row) {
      row.progress = done;
      row.total = total;
      notify();
    } else {
      await refresh();
    }
  },

  async finish(id: string, status: Exclude<JobStatus, "running">, error?: string): Promise<void> {
    const d = await db();
    await d.execute("UPDATE jobs SET status = $1, error = $2, updated_at = datetime('now') WHERE id = $3", [
      status,
      error ?? null,
      id,
    ]);
    await refresh();
  },

  /** Purge les jobs terminés (garde les `running`). */
  async clearFinished(): Promise<void> {
    const d = await db();
    await d.execute("DELETE FROM jobs WHERE status != 'running'");
    await refresh();
  },
};

function subscribe(cb: () => void): () => void {
  listeners.add(cb);
  // Premier abonné : amorce le cache (asynchrone, la vue se re-rendra à l'arrivée des données).
  if (listeners.size === 1) void refresh();
  return () => listeners.delete(cb);
}

/** Journal réactif — même patron que `lib/settings.ts`/`lib/places.ts` (pas de provider). */
export function useJobs(): JobRow[] {
  return useSyncExternalStore(
    subscribe,
    () => cache,
    () => cache,
  );
}

