// Recherche chara/waza sur le miroir wiki (`supabase-*.sqlite`, table `inagle_characters` /
// `inagle_skills`) — via `tauri-plugin-sql` directement (pas de commande Rust : `nie-wiki`
// dépend de `rusqlite`, qui entre en conflit de lien natif avec le `sqlx-sqlite` du plugin
// dans CE binaire, cf. `src-tauri/Cargo.toml`). Les requêtes SQL ci-dessous sont copiées
// TELLES QUELLES depuis `crates/tools/nie-wiki/src/query.rs` (`search_characters`/`search_skills`)
// — même vérité SQL, juste un moteur d'exécution différent.
import Database from "@tauri-apps/plugin-sql";
import { japaneseToRomaji } from "@rosegriffon/azalee/text";

import { dedoublonnerParNom, type EntreeNoms } from "@/lib/traduction";
import {
  REQUETES_INDEX_NOMS,
  SQL_ENCADREMENT,
  SQL_ROSTER,
  SQL_ROSTER_SANS_JSON,
  SQL_SKILLS_BRUTS,
  idsTechniques,
  sqlTechniquesParIds,
  sqlTechniquesParIdsSansJson,
  type LigneEncadrement,
  type LigneNoms,
  type LigneRoster,
  type LigneTechnique,
} from "@/lib/wikiQueries";

export interface CharaRow {
  id: string;
  chara_id: string;
  name_fr: string | null;
  name_en: string | null;
  name_ja: string | null;
  element: string | null;
  position: string | null;
  rarity_label: string | null;
  internal_code: string | null;
  slug: string | null;
  base_slug: string | null;
}

export interface WazaRow {
  id: string;
  name_fr: string | null;
  name_en: string | null;
  name_ja: string | null;
  category: string | null;
  element: string | null;
  power_max: number | null;
  power_min: number | null;
  tp_cost: number | null;
  description_fr: string | null;
  description_en: string | null;
  internal_code: string | null;
  is_hyper: number | null;
}

/** `sanitizeFilter` — identique à `nie_wiki::query::sanitize_filter` : retire `%,().*\` (pas `_`). */
function sanitizeFilter(input: string): string {
  return input.replace(/[%,().*\\]/g, "");
}

/** URI sqlite pour un chemin de fichier arbitraire (sqlx veut des `/`, pas des `\`). */
function toSqliteUri(path: string): string {
  return `sqlite:${path.replace(/\\/g, "/")}`;
}

const connections = new Map<string, Promise<Database>>();

function connect(dbPath: string): Promise<Database> {
  const uri = toSqliteUri(dbPath);
  let p = connections.get(uri);
  if (!p) {
    p = Database.load(uri);
    connections.set(uri, p);
  }
  return p;
}

/** Nom résolu depuis un `code` (basename sans extension, cf. `vfsIndexDb.codeOf`) — utilisé par
 * l'Explorateur/le détail de fichier pour afficher « Mark Evans » plutôt que « c01000100 ». */
export interface ResolvedName {
  kind: "chara" | "skill" | "item";
  name: string;
  /** Élément/poste (perso) ou catégorie (technique/objet), pour contexte, si connu. */
  extra: string | null;
}

/** Découpe `arr` en tranches d'au plus `size` éléments (paramètres SQLite bornés ~999). */
function chunk<T>(arr: T[], size: number): T[][] {
  const out: T[][] = [];
  for (let i = 0; i < arr.length; i += size) out.push(arr.slice(i, i + size));
  return out;
}

export const wikiDb = {
  /**
   * Résout un lot de `code`s (basenames VFS sans extension) vers leur personnage/technique/objet
   * en UNE poignée de requêtes `IN (...)` — plutôt qu'une requête par fichier affiché (un dossier
   * de personnages peut lister des milliers d'entrées), sur le même principe que l'index
   * `vfs_files` : précision + un seul aller-retour au lieu de N.
   */
  async resolveManyByCode(dbPath: string, codes: string[]): Promise<Map<string, ResolvedName>> {
    const unique = [...new Set(codes)].filter(Boolean);
    if (unique.length === 0) return new Map();
    const db = await connect(dbPath);
    const out = new Map<string, ResolvedName>();

    for (const batch of chunk(unique, 400)) {
      const placeholders = batch.map((_, i) => `$${i + 1}`).join(",");

      const chars = await db.select<{ internal_code: string; name_fr: string | null; name_en: string | null; element: string | null; position: string | null }[]>(
        `SELECT internal_code, name_fr, name_en, element, position FROM inagle_characters WHERE internal_code IN (${placeholders})`,
        batch,
      );
      for (const c of chars) {
        if (out.has(c.internal_code)) continue;
        const extra = [c.element, c.position].filter(Boolean).join(" · ");
        out.set(c.internal_code, { kind: "chara", name: c.name_fr ?? c.name_en ?? c.internal_code, extra: extra || null });
      }

      const skills = await db.select<{ internal_code: string; name_fr: string | null; name_en: string | null; category: string | null }[]>(
        `SELECT internal_code, name_fr, name_en, category FROM inagle_skills WHERE internal_code IN (${placeholders})`,
        batch,
      );
      for (const s of skills) {
        if (out.has(s.internal_code)) continue;
        out.set(s.internal_code, { kind: "skill", name: s.name_fr ?? s.name_en ?? s.internal_code, extra: s.category });
      }

      const items = await db.select<{ internal_code: string; name_fr: string | null; name_en: string | null; category: string | null }[]>(
        `SELECT internal_code, name_fr, name_en, category FROM inagle_items WHERE internal_code IN (${placeholders})`,
        batch,
      );
      for (const it of items) {
        if (out.has(it.internal_code)) continue;
        out.set(it.internal_code, { kind: "item", name: it.name_fr ?? it.name_en ?? it.internal_code, extra: it.category });
      }
    }

    return out;
  },

  async searchChara(dbPath: string, query: string): Promise<CharaRow[]> {
    const db = await connect(dbPath);
    const q = sanitizeFilter(query);
    const likePat = `%${q}%`;
    return db.select<CharaRow[]>(
      `SELECT id, chara_id, name_fr, name_en, name_ja, element, position,
              rarity_label, internal_code, slug, base_slug
       FROM inagle_characters
       WHERE id = $1
          OR chara_id = $1
          OR internal_code = $1
          OR slug = $1
          OR base_slug = $1
          OR name_fr LIKE $2
          OR name_en LIKE $2
          OR name_ja LIKE $2
       ORDER BY zukan_order ASC NULLS LAST, id ASC
       LIMIT 50`,
      [q, likePat],
    );
  },

  async searchWaza(dbPath: string, query: string): Promise<WazaRow[]> {
    const db = await connect(dbPath);
    const q = sanitizeFilter(query);
    const likePat = `%${q}%`;
    return db.select<WazaRow[]>(
      `SELECT id, name_fr, name_en, name_ja,
              category, element,
              power_max, power_min, tp_cost,
              description_fr, description_en,
              internal_code, is_hyper
       FROM inagle_skills
       WHERE id = $1
          OR internal_code = $1
          OR name_fr LIKE $2
          OR name_en LIKE $2
          OR name_ja LIKE $2
       ORDER BY name_fr ASC
       LIMIT 20`,
      [q, likePat],
    );
  },

  // ─────────────────────────────────────────────────────────────────────────
  // Vues migrées depuis `apps/azalee` — cf. `docs/MIGRATION-EXPLORATEUR.md`.
  // Le SQL vit dans `wikiQueries.ts` (pur, rejouable hors application) ; ici, seule
  // l'exécution.
  // ─────────────────────────────────────────────────────────────────────────

  /**
   * Index complet des noms multilingues — le Traducteur.
   *
   * Les sept requêtes partent EN PARALLÈLE : ce sont six tables distinctes d'un fichier local,
   * rien ne les sérialise. Le romaji est dérivé de `name_ja` à la lecture (aucune colonne
   * `name_roma` n'existe dans le miroir) puis mémorisé dans l'entrée — le calculer à chaque
   * frappe sur 9 500 lignes coûterait plus cher que toute la recherche.
   */
  async chargerIndexNoms(dbPath: string): Promise<EntreeNoms[]> {
    const d = await connect(dbPath);
    const lots = await Promise.all(
      REQUETES_INDEX_NOMS.map(async ({ type, sql }) => {
        const lignes = await d.select<LigneNoms[]>(sql);
        return lignes.map(
          (l): EntreeNoms => ({
            type,
            id: String(l.id),
            nomFr: l.name_fr,
            nomEn: l.name_en,
            nomJa: l.name_ja,
            romaji: japaneseToRomaji(l.name_ja),
            code: l.internal_code,
          }),
        );
      }),
    );
    return dedoublonnerParNom(lots.flat());
  },

  /**
   * Roster complet (6 166 personnages) — générateur d'équipe, comparateur, constructeur.
   *
   * Repli sans JSON1 : `json_extract` est compilé par défaut dans SQLite depuis 3.38 mais rien ne
   * le garantit dans un binaire tiers. Si la requête échoue, la même sans extraction JSON est
   * rejouée — on perd le code de rareté exact, jamais la liste.
   */
  async chargerRoster(dbPath: string): Promise<LigneRoster[]> {
    const d = await connect(dbPath);
    try {
      return await d.select<LigneRoster[]>(SQL_ROSTER);
    } catch {
      return d.select<LigneRoster[]>(SQL_ROSTER_SANS_JSON);
    }
  },

  /** Encadrement (`inagle_coordinators`, 102 lignes) — entraîneurs, managers, coordinateurs. */
  async chargerEncadrement(dbPath: string): Promise<LigneEncadrement[]> {
    const d = await connect(dbPath);
    return d.select<LigneEncadrement[]>(SQL_ENCADREMENT);
  },

  /**
   * Techniques d'un personnage — une requête pour lire la colonne `skills`, une seconde pour
   * résoudre tous ses identifiants d'un coup. Le wiki appelait `wikiService.getSkill` une fois
   * par technique, soit six allers-retours par personnage comparé.
   */
  async techniquesDuPersonnage(dbPath: string, charaId: string): Promise<LigneTechnique[]> {
    const d = await connect(dbPath);
    const lignes = await d.select<{ skills: string | null }[]>(SQL_SKILLS_BRUTS, [charaId]);
    const ids = idsTechniques(lignes[0]?.skills ?? null);
    if (ids.length === 0) return [];
    try {
      return await d.select<LigneTechnique[]>(sqlTechniquesParIds(ids.length), ids);
    } catch {
      return d.select<LigneTechnique[]>(sqlTechniquesParIdsSansJson(ids.length), ids);
    }
  },

  /**
   * Volumétrie du miroir, pour le tableau de bord. Compte les tables réellement présentes puis
   * n'interroge que celles-là : un miroir peut être partiel (dump interrompu), et une requête sur
   * une table absente ferait échouer tout le panneau au lieu d'afficher les autres chiffres.
   */
  async stats(dbPath: string): Promise<StatsMiroir> {
    const d = await connect(dbPath);
    const tables = await d.select<{ name: string }[]>(
      `SELECT name FROM sqlite_master WHERE type = 'table' AND name LIKE 'inagle_%'`,
    );
    const presentes = new Set(tables.map((t) => t.name));
    const compter = async (table: string): Promise<number | null> => {
      if (!presentes.has(table)) return null;
      const [r] = await d.select<{ n: number }[]>(`SELECT count(*) AS n FROM ${table}`);
      return r?.n ?? 0;
    };
    return {
      tables: presentes.size,
      personnages: await compter("inagle_characters"),
      techniques: await compter("inagle_skills"),
      objets: await compter("inagle_items"),
      equipes: await compter("inagle_teams"),
      avatars: await compter("inagle_keshins"),
    };
  },
};

/** Volumétrie du miroir wiki — cf. [`wikiDb.stats`]. `null` = table absente de ce miroir. */
export interface StatsMiroir {
  tables: number;
  personnages: number | null;
  techniques: number | null;
  objets: number | null;
  equipes: number | null;
  avatars: number | null;
}
