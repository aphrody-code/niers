// SQL du miroir wiki (`supabase-*.sqlite`) pour les vues migrées depuis `apps/azalee` —
// **pur** : aucune importation Tauri, aucune connexion. `wikiDb.ts` l'exécute, ce module ne fait
// que décrire les requêtes et la forme des lignes.
//
// Cette séparation n'est pas cosmétique : `@tauri-apps/plugin-sql` n'existe qu'à l'intérieur de
// la webview, donc tout module qui l'importe est invérifiable hors application. En gardant le SQL
// ici, la vérification (`verification-migration.ts`) rejoue EXACTEMENT les requêtes que
// l'application envoie, sur le vrai miroir — c'est ce qui distingue « la vue compile » de « la
// vue a réellement des lignes à afficher ».
//
// Les tables du miroir ne sont pas celles de Supabase. Deux écarts mesurés le 2026-09-02 :
//
//   * le wiki interroge `inagle_keshins_clean` / `inagle_souls_clean` — ce sont des VUES
//     Postgres, absentes du miroir. Les tables de base `inagle_keshins` (305) et `inagle_souls`
//     (56) existent, ce sont elles qu'on lit ;
//   * `inagle_characters.rarity_code` est vide sur les 6 166 lignes ; le code de rareté ne vit
//     que dans le JSON `data` (`$.rarityCode` : Normal 0, Expérimenté 2, Héros 10, BASARA 20 —
//     noter le 10, absent de la liste de `GameDataView`). C'est cette valeur qu'attend
//     `api.gameDataCalculateStats`, donc c'est elle qu'on extrait.

import type { EntreeNoms } from "@/lib/traduction";

// ---------------------------------------------------------------------------
// Index de noms — le Traducteur
// ---------------------------------------------------------------------------

/** Une ligne de nom, forme commune aux six tables interrogées. */
export interface LigneNoms {
  id: string;
  name_fr: string | null;
  name_en: string | null;
  name_ja: string | null;
  internal_code: string | null;
}

/**
 * Les six requêtes de l'index de noms, dans l'ordre d'affichage. Chacune rend la même forme
 * (`LigneNoms`), ce qui permet de les charger en parallèle et de concaténer sans transformation.
 *
 * `inagle_characters` exclut les variantes `…_5000` (mêmes que le wiki : doublons de zukan sans
 * fiche propre) et trie par `zukan_order` pour que le dédoublonnage par nom retienne la variante
 * canonique — cf. `traduction.dedoublonnerParNom`.
 */
export const REQUETES_INDEX_NOMS: readonly {
  type: EntreeNoms["type"];
  sql: string;
}[] = [
  {
    type: "chara",
    sql: `SELECT id, name_fr, name_en, name_ja, internal_code
          FROM inagle_characters
          WHERE internal_code IS NULL OR internal_code NOT LIKE '%\\_5000' ESCAPE '\\'
          ORDER BY zukan_order ASC, id ASC`,
  },
  {
    type: "waza",
    sql: `SELECT id, name_fr, name_en, name_ja, internal_code
          FROM inagle_skills
          ORDER BY name_fr ASC`,
  },
  {
    type: "objet",
    sql: `SELECT id, name_fr, name_en, name_ja, internal_code
          FROM inagle_items
          WHERE category IS NULL OR category <> 'special_tactics'
          ORDER BY name_fr ASC`,
  },
  {
    type: "tactique",
    sql: `SELECT id, name_fr, name_en, name_ja, internal_code
          FROM inagle_items
          WHERE category = 'special_tactics'
          ORDER BY name_fr ASC`,
  },
  {
    type: "equipe",
    sql: `SELECT id, name_fr, name_en, name_ja, internal_code
          FROM inagle_teams
          ORDER BY name_fr ASC`,
  },
  {
    type: "keshin",
    sql: `SELECT id, name_fr, name_en, name_ja, NULL AS internal_code
          FROM inagle_keshins
          ORDER BY name_fr ASC`,
  },
  {
    type: "totem",
    sql: `SELECT id, name_fr, name_en, name_ja, NULL AS internal_code
          FROM inagle_souls
          ORDER BY name_fr ASC`,
  },
];

// ---------------------------------------------------------------------------
// Roster — générateur d'équipe, comparateur, constructeur
// ---------------------------------------------------------------------------

/** Une ligne du roster telle que `SQL_ROSTER` la rend. */
export interface LigneRoster {
  id: string;
  chara_id: string | null;
  name_fr: string | null;
  name_en: string | null;
  name_ja: string | null;
  internal_code: string | null;
  element: string | null;
  position: string | null;
  sub_position: string | null;
  rarity_label: string | null;
  rarity_code: number | null;
  series: string | null;
  gender: string | null;
  team_id: string | null;
  zukan_order: number | null;
  /** Stats Lv99 — colonnes scalaires, source de vérité du miroir (la colonne `stats` est morte). */
  stat_frappe: number | null;
  stat_controle: number | null;
  stat_technique: number | null;
  stat_pression: number | null;
  stat_physique: number | null;
  stat_agilite: number | null;
  stat_intelligence: number | null;
}

/**
 * Le roster complet (6 166 lignes, ~1 Mo une fois réduit à ces colonnes).
 *
 * Le wiki paginait (`limit: 6000`) parce que chaque page était un aller-retour HTTP vers
 * Supabase ; ici la base est un fichier local ouvert par le processus, un `SELECT` complet coûte
 * quelques dizaines de millisecondes et évite toute pagination dans l'interface.
 *
 * Les stats Lv99 viennent des colonnes scalaires `stat_*` et NON du JSON `data` : la colonne
 * `data` pèse 10 Mo au total, et les paliers intermédiaires n'y sont de toute façon pas fiables
 * (`$.stats.lv30` est nul sur 6 166 lignes sur 6 166). Pour un niveau autre que 99, l'explorateur
 * n'interpole pas — il appelle `api.gameDataCalculateStats`, c'est-à-dire les vraies tables de
 * croissance de `nie-core`.
 */
export const SQL_ROSTER = `
  SELECT id, chara_id, name_fr, name_en, name_ja, internal_code,
         element, position, rarity_label,
         CAST(json_extract(data, '$.rarityCode') AS INTEGER) AS rarity_code,
         json_extract(data, '$.subPosition')                 AS sub_position,
         series, gender, team_id, zukan_order,
         stat_frappe, stat_controle, stat_technique, stat_pression,
         stat_physique, stat_agilite, stat_intelligence
  FROM inagle_characters
  WHERE internal_code IS NULL OR internal_code NOT LIKE '%\\_5000' ESCAPE '\\'
  ORDER BY zukan_order ASC, id ASC`;

/**
 * Repli sans JSON1 : mêmes colonnes, `rarity_code`/`sub_position` à `NULL`.
 *
 * `json_extract` est fourni par l'extension JSON1, compilée par défaut dans SQLite depuis 3.38
 * mais pas garantie dans tout binaire tiers. Plutôt que de parier, `wikiDb` rejoue cette requête
 * si la première échoue : on perd le code de rareté exact (le calcul de stats retombe alors sur
 * la rareté choisie à la main), jamais la liste.
 */
export const SQL_ROSTER_SANS_JSON = SQL_ROSTER.replace(
  "CAST(json_extract(data, '$.rarityCode') AS INTEGER) AS rarity_code,\n         json_extract(data, '$.subPosition')                 AS sub_position,",
  "NULL AS rarity_code,\n         NULL AS sub_position,",
);

// ---------------------------------------------------------------------------
// Encadrement — le générateur d'équipe
// ---------------------------------------------------------------------------

/** Une ligne d'encadrement (`inagle_coordinators`, 102 lignes : 3 Coach, 68 Manager, 31 Coordinator). */
export interface LigneEncadrement {
  id: number;
  name_localised: string | null;
  name_romaji: string | null;
  name_kanji: string | null;
  role: string | null;
  playstyle: string | null;
  element: string | null;
  buff: string | null;
  requirements: string | null;
}

/** Tout l'encadrement — 102 lignes, aucune pagination à prévoir. */
export const SQL_ENCADREMENT = `
  SELECT id, name_localised, name_romaji, name_kanji, role, playstyle, element, buff, requirements
  FROM inagle_coordinators
  ORDER BY role ASC, id ASC`;

// ---------------------------------------------------------------------------
// Techniques d'un personnage — le comparateur
// ---------------------------------------------------------------------------

/** Une technique résolue pour l'affichage du comparateur. */
export interface LigneTechnique {
  id: string;
  name_fr: string | null;
  name_en: string | null;
  category: string | null;
  element: string | null;
  power_max: number | null;
  tp_cost: number | null;
  is_hyper: number | null;
}

/**
 * Les techniques d'un personnage. `inagle_characters.skills` est un tableau JSON
 * `[{"skillId":"0x8C382852","learnLevel":0}, …]` : on lit la colonne telle quelle et on résout
 * les identifiants par un `IN (…)` — mêmes lignes que le wiki, sans son aller-retour par
 * technique (`wikiService.getSkill` appelé N fois).
 */
export const SQL_SKILLS_BRUTS = `SELECT skills FROM inagle_characters WHERE id = $1`;

/**
 * Construit la requête de résolution d'un lot d'identifiants de techniques.
 *
 * **La clé de jointure n'est pas `inagle_skills.id`.** Mesuré le 2026-09-02 : `id` vaut
 * `rhd10010` / `whk00010` (code interne), tandis que `inagle_characters.skills` porte des hachages
 * `0x8C382852`. Le hachage vit dans le JSON `data` de la table des techniques — `$.skillID`,
 * renseigné sur les 1 002 lignes ; la colonne `hash_id`, elle, est vide sur les 1 002. Joindre sur
 * `id` rend donc TOUJOURS zéro ligne, et le comparateur afficherait « Aucune technique » pour
 * chaque personnage sans que rien n'échoue. Les deux autres branches (`id`, `internal_code`)
 * couvrent un miroir qui porterait le hachage en clair.
 */
export function sqlTechniquesParIds(nombre: number): string {
  const trous = Array.from({ length: nombre }, (_, i) => `$${i + 1}`).join(",");
  return `SELECT id, name_fr, name_en, category, element, power_max, tp_cost, is_hyper
          FROM inagle_skills
          WHERE json_extract(data, '$.skillID') IN (${trous})
             OR id IN (${trous})
             OR internal_code IN (${trous})`;
}

/** Repli sans JSON1 : jointure sur `id`/`internal_code` seulement (cf. `SQL_ROSTER_SANS_JSON`). */
export function sqlTechniquesParIdsSansJson(nombre: number): string {
  const trous = Array.from({ length: nombre }, (_, i) => `$${i + 1}`).join(",");
  return `SELECT id, name_fr, name_en, category, element, power_max, tp_cost, is_hyper
          FROM inagle_skills WHERE id IN (${trous}) OR internal_code IN (${trous})`;
}

/**
 * Extrait les identifiants de techniques de la colonne `skills`, quelle que soit sa forme.
 *
 * Le miroir stocke tantôt `[{"skillId":"0x…","learnLevel":13}]`, tantôt une liste de chaînes.
 * L'identifiant fantôme `0xDBEDB6B8` (créneau de technique liée à l'aura, vide par construction)
 * est écarté — comme le fait `apps/azalee/app/tools/compare/page.tsx`.
 */
export function idsTechniques(brut: string | null): string[] {
  if (!brut) return [];
  let parse: unknown;
  try {
    parse = JSON.parse(brut);
  } catch {
    return [];
  }
  if (!Array.isArray(parse)) return [];
  const sortie: string[] = [];
  for (const item of parse) {
    const id =
      typeof item === "string"
        ? item
        : typeof item === "object" && item !== null && "skillId" in item
          ? String((item as { skillId: unknown }).skillId)
          : null;
    if (id && id !== "0xDBEDB6B8" && !sortie.includes(id)) sortie.push(id);
  }
  return sortie;
}
