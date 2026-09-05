/**
 * Le point d'injection de l'hôte dans l'interface partagée.
 *
 * Les composants de ce paquet n'appellent jamais Tauri ni `fetch` en direct : ils demandent
 * leur source par `useAssetSource()`, et l'hôte la fournit au montage. C'est ce qui permet aux
 * MÊMES composants de tourner dans Inacord (Tauri) et dans Aphrody (navigateur).
 *
 * ## Pourquoi les capacités sont un état, pas une constante
 *
 * Ce que l'hôte sait faire se MESURE : le VFS de `nie-site` s'indexe en tâche de fond, le
 * miroir peut être absent, le serveur peut être injoignable. Le fournisseur interroge donc la
 * source une fois monté, et l'interface connaît trois moments — on ne sait pas encore, on sait,
 * on a échoué — au lieu d'un booléen qui mentirait pendant la première seconde.
 */
import {
	type AssetSource,
	AUCUNE_CAPACITE,
	type CapacitesSource,
} from "@niers/asset-source";
import { createContext, type ReactNode, useContext, useEffect, useMemo, useState } from "react";

/** Ce que le contexte transporte : la source, et ce qu'elle sait faire. */
export interface ContexteSource {
	source: AssetSource;
	/** `null` tant que la mesure n'a pas abouti — à distinguer de « rien ne marche ». */
	capacites: CapacitesSource | null;
	/** Message d'erreur si la mesure a échoué, `null` sinon. */
	erreur: string | null;
}

const Contexte = createContext<ContexteSource | null>(null);

/**
 * Monte une source pour tout le sous-arbre.
 *
 * @param source l'implémentation de l'hôte — `creerWebSource()` pour Aphrody, l'enveloppe des
 * liaisons `tauri-specta` pour Inacord.
 */
export function AssetSourceProvider({
	source,
	children,
}: {
	source: AssetSource;
	children: ReactNode;
}) {
	const [capacites, setCapacites] = useState<CapacitesSource | null>(null);
	const [erreur, setErreur] = useState<string | null>(null);

	useEffect(() => {
		let vivant = true;
		setCapacites(null);
		setErreur(null);
		source
			.capacites()
			.then((c) => {
				if (vivant) setCapacites(c);
			})
			.catch((e: unknown) => {
				if (!vivant) return;
				// Une source qui ne répond pas ne sait rien faire — et le dire explicitement vaut
				// mieux que laisser chaque composant découvrir l'absence par son propre échec.
				setCapacites({ ...AUCUNE_CAPACITE });
				setErreur(e instanceof Error ? e.message : String(e));
			});
		return () => {
			vivant = false;
		};
	}, [source]);

	const valeur = useMemo<ContexteSource>(
		() => ({ source, capacites, erreur }),
		[source, capacites, erreur],
	);
	return <Contexte.Provider value={valeur}>{children}</Contexte.Provider>;
}

/**
 * La source montée par l'hôte.
 *
 * @throws si aucun `AssetSourceProvider` n'englobe l'appelant. L'erreur est volontairement
 * bruyante : un composant sans source rendrait du vide en silence, ce qui est le mode d'échec
 * le plus coûteux d'une interface — la page s'affiche, elle est simplement inutile.
 */
export function useAssetSource(): AssetSource {
	const ctx = useContext(Contexte);
	if (!ctx) {
		throw new Error(
			"useAssetSource() hors d'un <AssetSourceProvider>. L'hôte doit monter sa source : " +
				"creerWebSource() pour Aphrody, l'enveloppe Tauri pour Inacord.",
		);
	}
	return ctx.source;
}

/**
 * Ce que l'hôte sait faire, pour masquer ce qu'il ne peut pas.
 *
 * Rend `null` tant que la mesure court. Un composant qui affiche un bouton doit attendre ce
 * résultat plutôt que de supposer : sur 147 commandes de l'hôte desktop, 81 n'existeront jamais
 * dans un navigateur.
 */
export function useCapacites(): CapacitesSource | null {
	const ctx = useContext(Contexte);
	if (!ctx) throw new Error("useCapacites() hors d'un <AssetSourceProvider>.");
	return ctx.capacites;
}

/** L'erreur de mesure, s'il y en a une. Sert aux bandeaux d'état dégradé. */
export function useErreurSource(): string | null {
	const ctx = useContext(Contexte);
	if (!ctx) throw new Error("useErreurSource() hors d'un <AssetSourceProvider>.");
	return ctx.erreur;
}
