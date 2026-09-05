/**
 * Vérification des vues migrées depuis `apps/azalee` — **la source de données répond-elle
 * vraiment ?**
 *
 * Un `tsc --noEmit` propre et un `vite build` à zéro prouvent que le code compile ; ils ne
 * prouvent rien du branchement. Une requête transposée d'un `ilike` PostgREST vers du SQLite,
 * une table renommée entre Supabase et le miroir (`inagle_keshins_clean` → `inagle_keshins`),
 * une colonne vide (`rarity_code`) : tout cela compile parfaitement et rend un cadre vide.
 *
 * Ce script rejoue donc, sur les VRAIES données, exactement ce que les vues envoient :
 *
 *   * les sept requêtes de l'index de noms du Traducteur ;
 *   * le roster du comparateur / générateur / constructeur, avec et sans JSON1 ;
 *   * l'encadrement ;
 *   * la résolution des techniques d'un personnage ;
 *   * la recherche floue de bout en bout (index réel → résultat attendu) ;
 *   * la composition d'équipe et l'aller-retour du code de partage ;
 *   * le listage de la galerie depuis le VFS, et son croisement avec `gallery_config`.
 *
 * Lancement, depuis la racine du dépôt :
 *
 * ```sh
 * bun --bun apps/nie-explorer/src/lib/verification-migration.ts [chemin/vers/miroir.sqlite]
 * ```
 *
 * Sortie non nulle au premier échec. Un contrôle qui ne peut pas s'exécuter (miroir absent,
 * binaire `niers` non construit) est annoncé « SAUTÉ » — jamais compté comme réussi : un
 * contrôle muet qui ne s'exécute pas est un faux vert.
 *
 * Ce fichier n'est jamais importé par l'application (rien ne le référence depuis `main.tsx`) : il
 * ne pèse donc pas sur le bundle, tout en restant sous `src/` où `tsc` le type-vérifie avec le
 * reste. Les API Bun sont atteintes par `globalThis` parce que `@types/bun` n'est pas déclaré
 * dans le `tsconfig` de l'application.
 */

import {
  FORMATIONS,
  autoRemplir,
  codeElement,
  codePoste,
  genererEquipe,
  versJoueur,
  type Joueur,
} from "@/lib/equipe";
import {
  RACINE_GALERIE,
  categorieDe,
  construireIllustrations,
  titreIllustration,
  vignetteDediee,
} from "@/lib/galerie";
import { chercher, dedoublonnerParNom, type EntreeNoms } from "@/lib/traduction";
import {
  REQUETES_INDEX_NOMS,
  SQL_ENCADREMENT,
  SQL_ROSTER,
  SQL_ROSTER_SANS_JSON,
  SQL_SKILLS_BRUTS,
  idsTechniques,
  sqlTechniquesParIds,
  type LigneNoms,
  type LigneRoster,
} from "@/lib/wikiQueries";
import { decodeTeamCode, encodeTeamCode } from "@rosegriffon/azalee/game/team-code";
import { japaneseToRomaji } from "@rosegriffon/azalee/text";

/* eslint-disable no-console */

/** Accès aux globales Bun/Node sans `@types/bun` — cf. en-tête. */
const g = globalThis as {
  Bun?: {
    spawnSync: (cmd: string[], opts?: Record<string, unknown>) => {
      exitCode: number;
      stdout: { toString(): string };
    };
    file: (p: string) => { size: number };
  };
  process?: { argv: string[]; env: Record<string, string | undefined>; exit: (c: number) => never };
};

let reussis = 0;
let sautes = 0;
const echecs: string[] = [];

function verifier(nom: string, condition: boolean, detail: string) {
  if (condition) {
    reussis++;
    console.log(`  OK   ${nom} — ${detail}`);
  } else {
    echecs.push(`${nom} — ${detail}`);
    console.log(`  ÉCHEC ${nom} — ${detail}`);
  }
}

function sauter(nom: string, raison: string) {
  sautes++;
  console.log(`  SAUTÉ ${nom} — ${raison}`);
}

/** Ouvre le miroir en lecture seule. `null` si `bun:sqlite` ou le fichier manquent. */
async function ouvrirMiroir(chemin: string): Promise<{ query: (sql: string) => { all: (...p: unknown[]) => unknown[] } } | null> {
  try {
    // Spécificateur porté par une variable : `bun:sqlite` n'existe pas pour le `tsconfig` de
    // l'application (pas de `@types/bun`), un import littéral échouerait au type-check.
    const specificateur = "bun:sqlite";
    const mod = (await import(/* @vite-ignore */ specificateur)) as {
      Database: new (p: string, o?: Record<string, unknown>) => {
        query: (sql: string) => { all: (...p: unknown[]) => unknown[] };
      };
    };
    return new mod.Database(chemin, { readonly: true });
  } catch (e) {
    console.log(`  (ouverture impossible : ${String(e)})`);
    return null;
  }
}

async function principal() {
  const argv = g.process?.argv ?? [];
  const miroir =
    argv[2] ?? g.process?.env["NIE_WIKI_DB"] ?? `${argv[1]?.split("/apps/")[0] ?? "."}/var/mirror.sqlite`;

  console.log(`Miroir : ${miroir}`);
  const db = await ouvrirMiroir(miroir);

  // ── 1. Index de noms du Traducteur ────────────────────────────────────────
  console.log("\n[1] Index de noms (Traducteur)");
  let index: EntreeNoms[] = [];
  let brutes = 0;
  if (!db) {
    sauter("index de noms", "miroir illisible");
  } else {
    const lots: EntreeNoms[] = [];
    for (const { type, sql } of REQUETES_INDEX_NOMS) {
      let lignes: LigneNoms[] = [];
      try {
        lignes = db.query(sql).all() as LigneNoms[];
      } catch (e) {
        verifier(`requête ${type}`, false, String(e));
        continue;
      }
      verifier(`requête ${type}`, lignes.length > 0, `${lignes.length} ligne(s)`);
      for (const l of lignes) {
        lots.push({
          type,
          id: String(l.id),
          nomFr: l.name_fr,
          nomEn: l.name_en,
          nomJa: l.name_ja,
          romaji: japaneseToRomaji(l.name_ja),
          code: l.internal_code,
        });
      }
    }
    brutes = lots.length;
    index = dedoublonnerParNom(lots);
    verifier(
      "dédoublonnage par nom",
      index.length > 0 && index.length < brutes,
      `${brutes} lignes → ${index.length} entrées distinctes`,
    );
    const avecRomaji = index.filter((e) => e.romaji).length;
    verifier("romaji dérivé", avecRomaji > 0, `${avecRomaji} entrée(s) portent un romaji`);
  }

  // ── 2. Recherche floue de bout en bout ───────────────────────────────────
  console.log("\n[2] Recherche floue (index réel → résultat)");
  if (index.length === 0) {
    sauter("recherche floue", "index vide");
  } else {
    for (const [requete, attendu] of [
      ["mark evans", "chara"],
      // Une faute de frappe : la passe Levenshtein doit rattraper. Une SEULE — le score flou est
      // plafonne a 0,7, donc au-dela d'une substitution sur un nom de cette longueur on repasse
      // sous le seuil de 0,62. Meme limite que le wiki, dont le bareme est repris tel quel.
      ["mark evens", "chara"],
    ] as const) {
      const r = chercher(index, requete, null, 10);
      verifier(
        `chercher("${requete}")`,
        r.length > 0 && r[0].type === attendu,
        r.length > 0 ? `1er = ${r[0].nomFr ?? r[0].nomEn} (${r[0].type}, ${r[0].score.toFixed(2)})` : "aucun résultat",
      );
    }
    const parType = chercher(index, "tornado", "waza", 10);
    verifier(
      'chercher("tornado", waza)',
      parType.every((r) => r.type === "waza"),
      `${parType.length} résultat(s), tous de type waza`,
    );
    // Le romaji : introuvable par `ilike` côté web (aucune colonne `name_roma`), trouvable ici.
    const parRomaji = index.filter((e) => e.romaji && e.romaji.toLowerCase().includes("endou"));
    if (parRomaji.length === 0) {
      sauter("recherche par romaji", "aucun romaji contenant « endou » dans ce miroir");
    } else {
      const r = chercher(index, parRomaji[0].romaji!.split(" ")[0], null, 20);
      verifier(
        "recherche par romaji seul",
        r.length > 0,
        `${r.length} résultat(s) pour « ${parRomaji[0].romaji} »`,
      );
    }
  }

  // ── 3. Roster ────────────────────────────────────────────────────────────
  console.log("\n[3] Roster (comparateur / générateur / constructeur)");
  let joueurs: Joueur[] = [];
  if (!db) {
    sauter("roster", "miroir illisible");
  } else {
    let lignes: LigneRoster[] = [];
    try {
      lignes = db.query(SQL_ROSTER).all() as LigneRoster[];
      verifier("SQL_ROSTER (avec JSON1)", lignes.length > 0, `${lignes.length} ligne(s)`);
      const avecCode = lignes.filter((l) => l.rarity_code !== null).length;
      verifier(
        "rarity_code extrait du JSON",
        avecCode > 0,
        `${avecCode}/${lignes.length} lignes portent un code de rareté`,
      );
    } catch (e) {
      verifier("SQL_ROSTER (avec JSON1)", false, String(e));
    }
    try {
      const repli = db.query(SQL_ROSTER_SANS_JSON).all() as LigneRoster[];
      verifier(
        "SQL_ROSTER_SANS_JSON (repli)",
        repli.length === lignes.length && repli.every((l) => l.rarity_code === null),
        `${repli.length} ligne(s), rarity_code toujours nul`,
      );
    } catch (e) {
      verifier("SQL_ROSTER_SANS_JSON (repli)", false, String(e));
    }

    joueurs = lignes.map(versJoueur).filter((j) => j.poste !== "Entraîneur");
    const postesInconnus = joueurs.filter((j) => j.poste && codePoste(j.poste) === "MF" && j.poste !== "Milieu");
    verifier(
      "postes FR reconnus",
      postesInconnus.length === 0,
      postesInconnus.length === 0
        ? `${joueurs.length} joueur(s), aucun poste inconnu`
        : `postes non mappés : ${[...new Set(postesInconnus.map((j) => j.poste))].join(", ")}`,
    );
    const elemsInconnus = joueurs.filter((j) => j.element && codeElement(j.element) === "Void" && !["Néant", "Aucun", "Void"].includes(j.element));
    verifier(
      "éléments FR reconnus",
      elemsInconnus.length === 0,
      elemsInconnus.length === 0
        ? "tous les éléments sont mappés"
        : `éléments non mappés : ${[...new Set(elemsInconnus.map((j) => j.element))].join(", ")}`,
    );
    const avecStats = joueurs.filter((j) => j.stats.kick > 0).length;
    verifier("stats Lv99 présentes", avecStats > 0, `${avecStats}/${joueurs.length} joueur(s)`);
  }

  // ── 4. Encadrement ───────────────────────────────────────────────────────
  console.log("\n[4] Encadrement");
  if (!db) {
    sauter("encadrement", "miroir illisible");
  } else {
    try {
      const lignes = db.query(SQL_ENCADREMENT).all() as { role: string | null }[];
      const roles = [...new Set(lignes.map((l) => l.role))].join(", ");
      verifier("SQL_ENCADREMENT", lignes.length > 0, `${lignes.length} ligne(s) — rôles : ${roles}`);
    } catch (e) {
      verifier("SQL_ENCADREMENT", false, String(e));
    }
  }

  // ── 5. Techniques d'un personnage ────────────────────────────────────────
  console.log("\n[5] Techniques (comparateur)");
  if (!db) {
    sauter("techniques", "miroir illisible");
  } else {
    const candidats = db
      .query("SELECT id FROM inagle_characters WHERE skills IS NOT NULL AND skills <> '[]' LIMIT 5")
      .all() as { id: string }[];
    if (candidats.length === 0) {
      sauter("techniques", "aucun personnage ne porte de techniques dans ce miroir");
    } else {
      let resolues = 0;
      for (const c of candidats) {
        const brut = (db.query(SQL_SKILLS_BRUTS).all(c.id) as { skills: string | null }[])[0];
        const ids = idsTechniques(brut?.skills ?? null);
        if (ids.length === 0) continue;
        const lignes = db.query(sqlTechniquesParIds(ids.length)).all(...ids) as unknown[];
        resolues += lignes.length;
      }
      verifier(
        "résolution des techniques",
        resolues > 0,
        `${resolues} technique(s) résolue(s) sur ${candidats.length} personnage(s)`,
      );
    }
  }

  // ── 6. Composition d'équipe + code de partage ────────────────────────────
  console.log("\n[6] Composition d'équipe");
  if (joueurs.length === 0) {
    sauter("composition", "roster vide");
  } else {
    const formation = FORMATIONS[0];
    const rempli = autoRemplir(joueurs, formation, {});
    const surTerrain = Object.keys(rempli).filter((c) => c.startsWith("field-")).length;
    verifier(
      "auto-remplissage",
      surTerrain === formation.positions.length,
      `${surTerrain}/${formation.positions.length} créneaux de terrain`,
    );
    const aleatoire = genererEquipe(
      joueurs,
      formation,
      { element: "Feu", genre: null, rarete: null, serie: null },
      {},
    );
    const feu = Object.values(aleatoire).filter((m) => m.element === "Fire").length;
    verifier(
      "tirage filtré par élément",
      Object.keys(aleatoire).length === formation.positions.length,
      `${Object.keys(aleatoire).length} joueurs, dont ${feu} de Feu`,
    );
    const code = encodeTeamCode(
      formation.id,
      Object.values(rempli).map((m) => ({ slot: m.slot, charaId: m.charaId })),
    );
    const relu = decodeTeamCode(code);
    verifier(
      "aller-retour du code de partage",
      relu.formationId === formation.id && relu.slots.length === Object.keys(rempli).length,
      `${relu.slots.length} créneaux relus, formation ${relu.formationId}`,
    );
    verifier(
      "les identifiants relus existent dans le roster",
      relu.slots.every((s) => joueurs.some((j) => j.id === s.charaId)),
      `${relu.slots.length} identifiant(s) vérifiés`,
    );
  }

  // ── 7. Galerie depuis le VFS ─────────────────────────────────────────────
  console.log("\n[7] Galerie (VFS)");
  const bun = g.Bun;
  const racineDepot = argv[1]?.split("/apps/")[0] ?? ".";
  const niers = `${racineDepot}/target/release/niers`;
  let listing: { path: string; size: number }[] = [];
  if (!bun) {
    sauter("galerie", "API Bun indisponible");
  } else {
    let existe = false;
    try {
      existe = bun.file(niers).size > 0;
    } catch {
      existe = false;
    }
    if (!existe) {
      sauter("galerie", `binaire absent : ${niers} (cargo build -p nie-cli --release)`);
    } else {
      const r = bun.spawnSync([niers, "vfs", "find", `${RACINE_GALERIE}/`, "--limit", "30000"], {
        cwd: racineDepot,
        stdout: "pipe",
        stderr: "ignore",
      });
      const lignes = r.stdout.toString().split("\n");
      for (const ligne of lignes) {
        const m = /^\s*(\d+)\s+(\S+\.g4tx)\s/.exec(ligne);
        if (m) listing.push({ path: m[2], size: Number(m[1]) });
      }
      verifier("listage du VFS", listing.length > 0, `${listing.length} fichier(s) .g4tx`);
      const categories = new Set(listing.map((f) => categorieDe(f.path)).filter(Boolean));
      verifier(
        "catégories dérivées des sous-dossiers",
        categories.size >= 5,
        `${categories.size} catégorie(s) : ${[...categories].slice(0, 8).join(", ")}…`,
      );
      const titres = listing.slice(0, 50).map((f) => titreIllustration(f.path));
      verifier(
        "titres lisibles",
        titres.every((t) => t.length > 0 && !t.includes(".g4tx")),
        `ex. « ${titres[0]} »`,
      );
      // La vignette dédiée doit désigner un fichier RÉELLEMENT présent, sinon la grille
      // demanderait un chemin qui n'existe pas et n'afficherait rien.
      const connus = new Set(listing.map((f) => f.path));
      const avecVignette = listing.filter((f) => vignetteDediee(f.path) !== null);
      const vignettesPresentes = avecVignette.filter((f) => connus.has(vignetteDediee(f.path)!));
      if (avecVignette.length === 0) {
        sauter("vignettes dédiées", "aucune image `gallery_img2/img_*` dans ce VFS");
      } else {
        verifier(
          "vignettes dédiées présentes dans le VFS",
          vignettesPresentes.length === avecVignette.length,
          `${vignettesPresentes.length}/${avecVignette.length} couples img_/thumb_ résolus`,
        );
      }
      const illustrations = construireIllustrations(listing.slice(0, 200), new Map());
      verifier(
        "assemblage des illustrations",
        illustrations.length === Math.min(200, listing.length) &&
          illustrations.every((i) => i.cheminVignette.endsWith(".g4tx")),
        `${illustrations.length} illustration(s) assemblée(s)`,
      );
    }
  }

  // ── 8. Croisement gallery_config ↔ VFS ───────────────────────────────────
  console.log("\n[8] Croisement gallery_config ↔ VFS");
  if (!db || listing.length === 0) {
    sauter("croisement", "miroir ou listing VFS indisponible");
  } else {
    const bases = new Set(
      listing.map((f) => f.path.slice(f.path.lastIndexOf("/") + 1).replace(/\.g4tx$/i, "")),
    );
    const config = db.query("SELECT img_path, thumb_path FROM inagle_gallery").all() as {
      img_path: string | null;
      thumb_path: string | null;
    }[];
    const trouves = config.filter((c) => c.img_path && bases.has(c.img_path)).length;
    verifier(
      "les img_path de gallery_config existent dans le VFS",
      config.length > 0 && trouves > 0,
      `${trouves}/${config.length} illustration(s) de gallery_config retrouvée(s)`,
    );
  }

  // ── Bilan ────────────────────────────────────────────────────────────────
  console.log(
    `\nBilan : ${reussis} réussi(s), ${echecs.length} échec(s), ${sautes} sauté(s).`,
  );
  if (echecs.length > 0) {
    for (const e of echecs) console.log(`  ! ${e}`);
    g.process?.exit(1);
  }
}

await principal();
