// Registre des vues — **la** déclaration de ce que l'application contient comme pages.
//
// ## Pourquoi ce fichier existe
//
// Chaque vue était déclarée quatre fois, dans quatre fichiers, sans que rien ne les relie :
//
//  1. `App.tsx` → `sections` : l'entrée de barre latérale (id, libellé, icône, groupe) ;
//  2. `App.tsx` → `tabLabels` : le libellé, une seconde fois, pour le menu Affichage ;
//  3. `AppMenu.tsx` → `VIEW_TABS` : l'ordre des vues et leurs accélérateurs `Ctrl+1…9` ;
//  4. `App.tsx` → `<TabsContent>` : le rendu.
//
// Une vue ajoutée aux trois premiers mais pas au quatrième s'affiche vide ; ajoutée aux deux
// premiers seulement, elle est inatteignable au clavier ; et sept libellés vivaient en dur, donc
// restaient français dans les trois locales. Rien de tout cela ne se voyait à la compilation.
//
// Ici : un tableau, dans l'ordre d'affichage. La barre latérale, le menu Affichage, les
// accélérateurs, la palette de commandes et le tableau de bord le lisent. Seul le RENDU reste
// dans `App.tsx` : les vues prennent des props trop différentes (état d'éditeur, onglets
// d'explorateur, chemin externe) pour tenir dans une table.
import type { TFn } from "@/lib/i18n";

/** Groupe de la barre latérale. `principal` n'a pas d'intitulé. */
export type GroupeVue = "principal" | "donnees" | "outils";

export interface Vue {
  id: string;
  /** Clé i18n du libellé (`tab.<id>`, présente dans les trois dictionnaires). */
  cle: string;
  /** Nom d'icône, tel que le résout `ui/Icon` (jeu Material → lucide). */
  icone: string;
  groupe: GroupeVue;
  /**
   * Ce que la vue fait, en une ligne : info-bulle de la barre latérale, sous-titre de la palette
   * de commandes. Volontairement NON traduite — c'est de l'aide contextuelle, pas un libellé
   * d'interface, et une phrase fausse dans deux langues vaut moins qu'une phrase juste dans une.
   */
  description: string;
  /**
   * Vue accessible depuis la barre latérale. `settings` vaut `false` : elle a son propre bouton,
   * en bas, séparé — mais elle reste une vue à part entière pour le menu et la palette.
   */
  barreLaterale: boolean;
  /**
   * Outil de spécialiste, masqué par défaut — reverse-engineering, mémoire du jeu lancé,
   * scripts, dump Criware.
   *
   * **Ce n'est pas une suppression** : la vue reste entière, atteignable par la palette de
   * commandes et par le réglage « Outils avancés ». Elle quitte simplement la barre latérale,
   * où quinze entrées dont la moitié ne parlent qu'à une personne rendaient les cinq qui
   * comptent — le Cinéma, l'Explorateur, les Données, la Galerie, les Mods — indistinctes des
   * autres.
   */
  avancee?: boolean;
}

/** Les vues, dans l'ordre d'affichage — celui de la barre latérale ET des accélérateurs. */
export const VUES: readonly Vue[] = [
  {
    // Les deux espaces de travail s'ouvrent en premier : l'application sert d'abord à parcourir
    // et modifier les assets du jeu. Les catalogues et outils restent disponibles par Ctrl+K.
    id: "editor",
    cle: "tab.editor",
    icone: "view_in_ar",
    groupe: "principal",
    description: "Visionneuse 3D et édition de propriétés des modèles.",
    barreLaterale: true,
  },
  {
    id: "explorer",
    cle: "tab.explorer",
    icone: "folder_open",
    groupe: "principal",
    description: "Arborescence complète du VFS, à onglets.",
    barreLaterale: true,
  },
  {
    id: "cinema",
    cle: "tab.cinema",
    icone: "movie",
    groupe: "principal",
    description: "La médiathèque : les dix saisons de la série et les cinématiques du jeu.",
    // Cinéma est une destination secondaire : la palette et le menu Affichage l'exposent sans
    // concurrencer les deux surfaces de travail dans la barre latérale.
    barreLaterale: false,
  },
  {
    id: "dashboard",
    cle: "tab.dashboard",
    icone: "dashboard",
    groupe: "principal",
    description: "L'état mesuré des quatre sources et de chaque onglet.",
    barreLaterale: false,
  },
  {
    id: "search",
    cle: "tab.search",
    icone: "search",
    groupe: "principal",
    description: "Recherche par chemin, extension et code interne.",
    // Hors barre latérale : trois entrées y proposaient la même intention — cette vue, le champ
    // « Rechercher… Ctrl+K » de la barre du haut, et la palette de commandes. Chercher un fichier
    // est d'ailleurs déjà ce que fait l'Explorateur, juste au-dessus. La vue reste entière et
    // s'ouvre par Ctrl+K.
    barreLaterale: false,
  },
  {
    id: "data",
    cle: "tab.data",
    // `menu_book` et non `database` : la vue montre des techniques, des objets et des quêtes,
    // pas des tables. Un cylindre de base de données annonçait de la plomberie là où il y a un
    // catalogue de jeu.
    icone: "menu_book",
    groupe: "donnees",
    description: "Techniques, objets, quêtes, boutiques, formations.",
    barreLaterale: true,
  },
  {
    id: "gallery",
    cle: "tab.gallery",
    icone: "image",
    groupe: "donnees",
    description: "Planches de textures et images, en aperçu direct.",
    barreLaterale: true,
  },
  {
    id: "cpk",
    cle: "tab.cpk",
    icone: "deployed_code",
    groupe: "donnees",
    description: "Ouverture brute d'une archive CPK, hors montage VFS.",
    barreLaterale: true,
    // « Hors montage VFS » dit tout : on ouvre le conteneur au lieu de parcourir les fichiers du
    // jeu. C'est l'Explorateur qu'on veut dans 99 % des cas ; celle-ci sert quand le VFS ne monte
    // plus, donc à qui sait déjà pourquoi il la cherche.
    avancee: true,
  },
  {
    id: "save",
    cle: "tab.save",
    icone: "save",
    groupe: "donnees",
    description: "Lecture d'une sauvegarde Steam Cloud.",
    barreLaterale: true,
  },
  {
    id: "tools",
    cle: "tab.tools",
    icone: "build",
    groupe: "outils",
    description: "Traducteur, comparateur, équipe aléatoire, constructeur.",
    barreLaterale: true,
  },
  {
    id: "mods",
    cle: "tab.mods",
    icone: "extension",
    groupe: "outils",
    description: "Registre local des mods, mise en scène et export CPK.",
    barreLaterale: true,
  },
  {
    id: "re",
    cle: "tab.re",
    icone: "memory",
    groupe: "outils",
    description: "Fonctions, classes RTTI, xrefs, et la forge.",
    barreLaterale: true,
    avancee: true,
  },
  {
    id: "viola",
    cle: "tab.viola",
    icone: "hard_drive",
    groupe: "outils",
    description: "Dump, pack et crypto Criware.",
    barreLaterale: true,
    avancee: true,
  },
  {
    id: "livemod",
    cle: "tab.livemod",
    icone: "bolt",
    groupe: "outils",
    description: "Lecture de la mémoire vivante du jeu lancé.",
    barreLaterale: true,
    avancee: true,
  },
  {
    id: "lua",
    cle: "tab.lua",
    icone: "edit_note",
    groupe: "outils",
    description: "Scripts du jeu — désassemblage et session d'exécution.",
    barreLaterale: true,
    avancee: true,
  },
  {
    id: "settings",
    cle: "tab.settings",
    icone: "settings",
    groupe: "outils",
    description: "Dossier du jeu, bases, apparence, MCP.",
    barreLaterale: false,
  },
] as const;

/** Intitulés des groupes de la barre latérale. `principal` n'en a pas. */
export const LIBELLE_GROUPE: Record<GroupeVue, string | null> = {
  principal: null,
  donnees: "Données",
  outils: "Outils",
};

/** Index par identifiant — évite un `find` linéaire à chaque rendu. */
const PAR_ID = new Map(VUES.map((v) => [v.id, v]));

/** La vue portant cet identifiant, ou `undefined` si l'identifiant n'en désigne aucune. */
export function vue(id: string): Vue | undefined {
  return PAR_ID.get(id);
}

/** Tous les libellés, pour le menu Affichage (`AppMenuActions.tabLabels`). */
export function libellesVues(t: TFn): Record<string, string> {
  return Object.fromEntries(VUES.map((v) => [v.id, t(v.cle)]));
}

/** Les vues d'un groupe, dans l'ordre — barre latérale seulement. */
export function vuesDuGroupe(groupe: GroupeVue): Vue[] {
  return VUES.filter((v) => v.groupe === groupe && v.barreLaterale);
}

/** Identifiants dans l'ordre — le menu Affichage et ses accélérateurs `Ctrl+1…9`. */
export const IDS_VUES: readonly string[] = VUES.map((v) => v.id);
