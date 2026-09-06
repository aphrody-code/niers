/**
 * Les touches des menus du jeu, branchées sur la fenêtre.
 *
 * ## Pourquoi un crochet et pas un `onKeyDown` par composant
 *
 * Les guides de touches du jeu (« Tab Réinitialiser », « Alt Confirmer », « X Chercher par nom
 * de joueur ») sont dessinés en bas de l'écran, loin du contrôle qu'ils actionnent : ils
 * n'obtiennent pas le focus, et un `onKeyDown` posé sur eux ne recevrait jamais rien. La règle
 * du dépôt est qu'un guide de touche n'est jamais dessiné sans son gestionnaire ; ce crochet
 * EST ce gestionnaire, et `GameHintBar` refuse une touche sans action.
 *
 * ## Ce qu'il ne capture pas
 *
 * Une frappe dans un champ de saisie reste une frappe : taper « v » dans la barre de recherche
 * ne doit pas cocher « Tout ». Seule `Escape` traverse un champ — c'est la touche qui en sort.
 */
import { useEffect } from "react";

/** Une touche et ce qu'elle fait. `key` suit `KeyboardEvent.key` (`"Tab"`, `"x"`, `"Alt"`). */
export interface GameKeyBinding {
	key: string;
	onActivate: () => void;
	/** Laisse la touche agir même depuis un champ de saisie. Réservé à `Escape` en pratique. */
	fromInputs?: boolean;
}

/** La cible d'un événement est-elle un champ où l'on tape ? */
export function isEditableTarget(target: EventTarget | null): boolean {
	if (!(target instanceof HTMLElement)) return false;
	if (target.isContentEditable) return true;
	const tag = target.tagName;
	return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT";
}

/** Compare une touche pressée à une touche déclarée, sans tenir compte de la casse. */
export function keyMatches(event: KeyboardEvent | { key: string }, key: string): boolean {
	return event.key.toLowerCase() === key.toLowerCase();
}

/**
 * Écoute les touches déclarées sur `window` tant que `enabled` est vrai.
 *
 * Une touche dont l'action a été trouvée est consommée (`preventDefault`) : « Tab » ne doit pas
 * en plus déplacer le focus, ni « / » ouvrir la recherche rapide du navigateur.
 */
export function useGameKeys(bindings: readonly GameKeyBinding[], enabled = true): void {
	useEffect(() => {
		if (!enabled || bindings.length === 0) return;
		const onKeyDown = (event: KeyboardEvent) => {
			if (event.defaultPrevented || event.ctrlKey || event.metaKey) return;
			const editable = isEditableTarget(event.target);
			for (const binding of bindings) {
				if (!keyMatches(event, binding.key)) continue;
				if (editable && !binding.fromInputs) return;
				event.preventDefault();
				binding.onActivate();
				return;
			}
		};
		window.addEventListener("keydown", onKeyDown);
		return () => window.removeEventListener("keydown", onKeyDown);
	}, [bindings, enabled]);
}
