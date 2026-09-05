// Compositions d'équipe enregistrées **localement** — `tauri-plugin-sql`, table `teams` de
// `mods.db` (migration v4), dans `BaseDirectory::AppData`.
//
// ## Pourquoi une table, et pas la session du wiki
//
// Le constructeur du site sauvegarde côté serveur (`app/actions/teams.ts`, gardé par
// `getServerSession`) : sans compte connecté, le bouton « Créer » n'apparaît même pas, et tout ce
// qui reste est un `localStorage` d'un seul brouillon (`azalee-my-team`). Une application de
// bureau n'a ni compte ni session — mais elle a un disque. La contrainte du web devient donc un
// avantage : **plusieurs compositions nommées**, persistées, listées, renommées, supprimées,
// sans réseau ni authentification.
//
// Le choix de `mods.db` plutôt qu'une base neuve suit la convention déjà en place
// (`modsDb.ts`, `jobsDb.ts`, `vfsIndexDb.ts` partagent ce fichier) : une seule base à migrer, une
// seule à sauvegarder.
//
// Ce qui n'est PAS ici : le format d'échange. Une composition se partage par le code de
// `@rosegriffon/azalee/game/team-code`, identique à celui des URLs du wiki — cf. `equipe.ts`.
import Database from "@tauri-apps/plugin-sql";

import type { TeamMember } from "@rosegriffon/azalee/game/team-types";

/** Une composition enregistrée, telle que la table la stocke. */
export interface LigneEquipe {
  id: string;
  name: string;
  formation_id: string;
  /** JSON : `Record<créneau, TeamMember>`. */
  members: string;
  created_at: string;
  updated_at: string;
}

/** Une composition décodée, prête à charger dans le constructeur. */
export interface EquipeEnregistree {
  id: string;
  nom: string;
  formationId: string;
  membres: Record<string, TeamMember>;
  misAJourLe: string;
}

let promesseDb: Promise<Database> | null = null;
function db(): Promise<Database> {
  return (promesseDb ??= Database.load("sqlite:mods.db"));
}

/**
 * Décode la colonne `members`. Une composition illisible (base éditée à la main, migration
 * partielle) rend une composition VIDE plutôt que de faire tomber la liste entière : perdre une
 * équipe est ennuyeux, perdre l'accès aux autres l'est davantage.
 */
function decoderMembres(json: string): Record<string, TeamMember> {
  try {
    const parse: unknown = JSON.parse(json);
    return parse && typeof parse === "object" ? (parse as Record<string, TeamMember>) : {};
  } catch {
    return {};
  }
}

function versEquipe(l: LigneEquipe): EquipeEnregistree {
  return {
    id: l.id,
    nom: l.name,
    formationId: l.formation_id,
    membres: decoderMembres(l.members),
    misAJourLe: l.updated_at,
  };
}

export const teamsDb = {
  /** Toutes les compositions, la plus récemment modifiée d'abord. */
  async lister(): Promise<EquipeEnregistree[]> {
    const d = await db();
    const lignes = await d.select<LigneEquipe[]>(
      "SELECT * FROM teams ORDER BY updated_at DESC, name ASC",
    );
    return lignes.map(versEquipe);
  },

  /** Une composition par identifiant, `null` si elle n'existe plus. */
  async lire(id: string): Promise<EquipeEnregistree | null> {
    const d = await db();
    const lignes = await d.select<LigneEquipe[]>("SELECT * FROM teams WHERE id = $1", [id]);
    return lignes[0] ? versEquipe(lignes[0]) : null;
  },

  /**
   * Crée une composition et rend son identifiant.
   *
   * `crypto.randomUUID()` comme `modsDb.newId()` — même convention d'identifiant dans toute la
   * base.
   */
  async creer(
    nom: string,
    formationId: string,
    membres: Record<string, TeamMember>,
  ): Promise<string> {
    const d = await db();
    const id = crypto.randomUUID();
    await d.execute(
      "INSERT INTO teams (id, name, formation_id, members) VALUES ($1, $2, $3, $4)",
      [id, nom, formationId, JSON.stringify(membres)],
    );
    return id;
  },

  /** Met à jour une composition existante (nom, formation et membres d'un coup). */
  async mettreAJour(
    id: string,
    nom: string,
    formationId: string,
    membres: Record<string, TeamMember>,
  ): Promise<void> {
    const d = await db();
    await d.execute(
      `UPDATE teams SET name = $2, formation_id = $3, members = $4, updated_at = datetime('now')
       WHERE id = $1`,
      [id, nom, formationId, JSON.stringify(membres)],
    );
  },

  /** Supprime une composition. */
  async supprimer(id: string): Promise<void> {
    const d = await db();
    await d.execute("DELETE FROM teams WHERE id = $1", [id]);
  },
};
