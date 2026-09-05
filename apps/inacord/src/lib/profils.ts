// Les profils de la médiathèque — « Qui regarde ? ».
//
// ## Pourquoi des profils dans une application locale
//
// La vue Cinéma garde trois choses par personne : les épisodes vus, les positions de reprise, et
// la liste des titres mis de côté. Tant qu'il n'y avait qu'un seul jeu de clés `localStorage`,
// deux personnes devant la même fenêtre se marchaient dessus — celle qui reprend la série à
// l'épisode 40 effaçait la progression de l'autre sans rien dire. C'est exactement le problème
// que les profils résolvent chez Netflix et Disney+, et il se pose ici pour la même raison.
//
// ## Le cloisonnement, sans migration
//
// Le premier profil porte l'identifiant `principal` et lit les clés HISTORIQUES
// (`nie-explorer:cinema:vus`, `…:reprise`) : la progression déjà accumulée lui appartient
// telle quelle, sans code de migration à écrire ni à maintenir. Tout profil créé ensuite écrit
// dans `<clé>:<id>`. Supprimer un profil supprime ses clés — c'est la seule opération destructrice
// du module, et elle ne touche jamais celles d'un autre.
//
// ## Ce qui n'est PAS ici
//
// Aucune authentification, aucun secret : un profil est une préférence d'affichage, pas un
// compte. Le profil « jeunesse » masque la fiche technique (codec, octets, chemin VFS) parce
// qu'elle n'intéresse personne à cet âge-là — il ne protège rien et ne prétend pas le faire.

/** Identifiant du profil qui hérite des clés d'avant les profils. */
export const PROFIL_PRINCIPAL = "principal";

/** Clé de la liste des profils. */
const CLE_PROFILS = "nie-explorer:cinema:profils";

/** Clé du profil choisi pour LA SESSION en cours (`sessionStorage`, cf. `lireProfilActif`). */
const CLE_ACTIF = "nie-explorer:cinema:profil-actif";

/** Clé de « ma liste » — cf. la section du même nom, plus bas. */
const CLE_LISTE = "nie-explorer:cinema:liste";

/** Un profil de lecture. */
export interface Profil {
  id: string;
  nom: string;
  /** Index dans `PALETTE`. */
  couleur: number;
  /** Nom d'icône de `ui/Icon` — l'emblème affiché sur l'avatar. */
  embleme: string;
  /** Profil jeunesse : la fiche technique est masquée, les rubriques du jeu restent lisibles. */
  jeunesse?: boolean;
}

/**
 * Les dégradés d'avatar, en valeurs littérales.
 *
 * Elles ne passent PAS par des classes Tailwind : une classe construite à l'exécution
 * (`from-${x}`) n'est pas dans la sortie du compilateur, donc l'avatar serait transparent en
 * production alors qu'il s'affiche en développement. Le `style` en ligne n'a pas ce piège.
 */
export const PALETTE: readonly { nom: string; de: string; vers: string }[] = [
  { nom: "Foudre", de: "#fbbf24", vers: "#b45309" },
  { nom: "Océan", de: "#38bdf8", vers: "#1d4ed8" },
  { nom: "Flamme", de: "#fb7185", vers: "#be123c" },
  { nom: "Forêt", de: "#4ade80", vers: "#047857" },
  { nom: "Néant", de: "#c084fc", vers: "#6d28d9" },
  { nom: "Cendre", de: "#cbd5e1", vers: "#475569" },
];

/** Les emblèmes proposés — tous vérifiés présents dans la table de `ui/Icon`, qui rend `null`
 * (donc rien du tout, sans erreur) sur un nom qu'elle ne connaît pas. */
export const EMBLEMES: readonly string[] = [
  "sports_soccer",
  "bolt",
  "local_fire_department",
  "shield",
  "swords",
  "stars",
  "stadium",
  "pets",
  "favorite",
  "auto_awesome",
  "person",
  "groups",
];

/** Dégradé CSS d'un profil. */
export function degrade(profil: Profil): string {
  const p = PALETTE[profil.couleur % PALETTE.length] ?? PALETTE[0]!;
  return `linear-gradient(135deg, ${p.de} 0%, ${p.vers} 100%)`;
}

function lireJson<T>(cle: string, defaut: T): T {
  try {
    const brut = localStorage.getItem(cle);
    return brut === null ? defaut : (JSON.parse(brut) as T);
  } catch {
    return defaut;
  }
}

function ecrireJson(cle: string, valeur: unknown): void {
  try {
    localStorage.setItem(cle, JSON.stringify(valeur));
  } catch {
    // Quota plein : un profil est un confort, comme la progression qu'il porte.
  }
}

/**
 * Clé de stockage cloisonnée par profil.
 *
 * `principal` garde la clé nue — c'est ce qui rend la progression d'avant les profils lisible
 * sans migration. Cf. l'en-tête du module.
 */
export function clePourProfil(base: string, profilId: string): string {
  return profilId === PROFIL_PRINCIPAL ? base : `${base}:${profilId}`;
}

/** Les profils enregistrés. Une liste vide signifie « jamais configuré » — l'appelant montre
 * alors l'écran de choix, qui propose d'en créer un. */
export function lireProfils(): Profil[] {
  const brut = lireJson<unknown>(CLE_PROFILS, []);
  if (!Array.isArray(brut)) return [];
  return brut.filter(
    (p): p is Profil =>
      typeof p === "object" &&
      p !== null &&
      typeof (p as Profil).id === "string" &&
      typeof (p as Profil).nom === "string",
  );
}

export function ecrireProfils(profils: readonly Profil[]): void {
  ecrireJson(CLE_PROFILS, profils);
}

/**
 * L'identifiant du profil actif, ou `null` — auquel cas il faut demander qui regarde.
 *
 * Le choix vit dans `sessionStorage`, PAS dans `localStorage` : il dure ce que dure la fenêtre.
 * L'application redemande donc « Qui regarde ? » à chaque lancement — c'est ce que font les deux
 * plateformes de référence, et c'est la seule façon qu'un profil veuille dire quelque chose sur
 * une machine partagée. Changer de vue et revenir au Cinéma ne le redemande pas : la session,
 * elle, n'a pas été interrompue.
 */
export function lireProfilActif(): string | null {
  try {
    return sessionStorage.getItem(CLE_ACTIF);
  } catch {
    return null;
  }
}

export function ecrireProfilActif(id: string | null): void {
  try {
    if (id === null) sessionStorage.removeItem(CLE_ACTIF);
    else sessionStorage.setItem(CLE_ACTIF, id);
    // La clé homonyme de `localStorage` date de la première version, qui gardait le choix d'un
    // lancement à l'autre. La retirer évite qu'une valeur morte survive dans le stockage sans
    // que plus rien ne la lise.
    localStorage.removeItem(CLE_ACTIF);
  } catch {
    // Ignoré volontairement.
  }
}

/** Le profil que l'on crée au premier passage — il hérite des clés historiques. */
export function profilPrincipal(nom = "Moi"): Profil {
  return { id: PROFIL_PRINCIPAL, nom, couleur: 0, embleme: "sports_soccer" };
}

/** Un identifiant stable et lisible, sans collision avec un profil existant. */
export function nouvelId(existants: readonly Profil[]): string {
  const pris = new Set(existants.map((p) => p.id));
  for (let n = 2; ; n++) {
    const id = `profil-${n}`;
    if (!pris.has(id)) return id;
  }
}

/**
 * Efface toutes les données d'un profil.
 *
 * Le profil `principal` fait exception : ses clés sont les clés historiques, et les effacer
 * emporterait la progression de quelqu'un qui n'a peut-être jamais créé de deuxième profil. On
 * le vide donc de la même façon que les autres, mais l'appelant ne propose pas de le supprimer —
 * cf. `ChoixProfil`.
 */
export function oublierProfil(id: string): void {
  for (const base of ["nie-explorer:cinema:vus", "nie-explorer:cinema:reprise", CLE_LISTE]) {
    try {
      localStorage.removeItem(clePourProfil(base, id));
    } catch {
      // Ignoré volontairement.
    }
  }
}

// ── Ma liste ──────────────────────────────────────────────────────────────────
//
// Le `+` des deux plateformes de référence. La clé d'un élément est celle du catalogue unifié —
// chemin VFS pour une cinématique, identifiant YouTube pour un épisode — donc une seule liste
// mélange les deux sources sans avoir à distinguer laquelle.

/** Les clés d'éléments mis de côté par un profil. */
export function lireListe(profilId: string): Set<string> {
  const brut = lireJson<unknown>(clePourProfil(CLE_LISTE, profilId), []);
  return new Set(Array.isArray(brut) ? brut.filter((x): x is string => typeof x === "string") : []);
}

export function ecrireListe(profilId: string, cles: ReadonlySet<string>): void {
  ecrireJson(clePourProfil(CLE_LISTE, profilId), [...cles]);
}
