/**
 * État partagé entre le shell interactif (`azalee shell`) et les commandes.
 *
 * Deux besoins croisés, et deux seulement :
 *
 * 1. **Mode REPL** — en shell, une commande ne doit pas appeler `process.exit`
 *    (elle tuerait le shell) ; hors shell, l'exit est ce qui libère les
 *    connexions Postgres/Redis encore ouvertes.
 * 2. **Sélection différée** — quand une recherche renvoie plusieurs
 *    correspondances, le shell affiche une liste numérotée et attend un choix ;
 *    la commande dépose ici les candidats, le shell les consomme.
 *
 * Ce module est volontairement le seul point de mutation globale du CLI.
 */

import type { Interface as ReadlineInterface } from "node:readline";

/** Familles d'entités pour lesquelles une sélection différée existe. */
export type PendingSelectionType = "chara" | "skill" | "item" | "team";

/** Candidats en attente d'un choix numéroté dans le shell. */
export interface PendingSelection {
	type: PendingSelectionType;
	// Formes hétérogènes issues d'inagle (personnage, technique, objet, équipe).
	matches: any[];
}

let replMode = false;
let pendingSelection: PendingSelection | null = null;
let activeReadline: ReadlineInterface | null = null;

/** Vrai lorsqu'une commande s'exécute depuis `azalee shell`. */
export function isReplMode(): boolean {
	return replMode;
}

export function setReplMode(value: boolean): void {
	replMode = value;
}

/**
 * Termine le processus avec `code`, **sauf** en mode REPL où l'on rend
 * simplement la main au shell. Remplace tous les `if (!isReplMode) process.exit()`.
 */
export function exitUnlessRepl(code = 0): void {
	if (!replMode) process.exit(code);
}

export function getPendingSelection(): PendingSelection | null {
	return pendingSelection;
}

export function setPendingSelection(selection: PendingSelection | null): void {
	pendingSelection = selection;
}

/** Interface readline du shell, ou `null` hors shell. */
export function getActiveReadline(): ReadlineInterface | null {
	return activeReadline;
}

export function setActiveReadline(rl: ReadlineInterface | null): void {
	activeReadline = rl;
}
