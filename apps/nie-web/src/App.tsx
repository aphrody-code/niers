import {
	AssetSourceProvider,
	useCapacites,
	useErreurSource,
} from "@niers/inacord-ui";
import { creerWebSource } from "@niers/asset-source";
import { useEffect, useMemo, useState } from "react";
import { type SanteApi, sante } from "@niers/asset-source/nie-site";

/**
 * Coquille d'Aphrody.
 *
 * L'hôte n'a qu'un rôle : construire sa source et la monter. Tout le reste vient de
 * `@niers/inacord-ui`, la même interface que celle d'Inacord — c'est le point de la manœuvre.
 */
export function App() {
	// La source ne dépend d'aucun état : la mémoriser évite de relancer la mesure des
	// capacités à chaque rendu.
	const source = useMemo(() => creerWebSource(), []);
	return (
		<AssetSourceProvider source={source}>
			<Accueil />
		</AssetSourceProvider>
	);
}

/**
 * Ce que le serveur déclare savoir servir, ici et maintenant.
 *
 * L'index du VFS se monte en tâche de fond : l'interface distingue donc « on ne sait pas
 * encore » de « rien ne marche », au lieu d'afficher des vues vides pendant la première
 * seconde.
 */
function Accueil() {
	const capacites = useCapacites();
	const erreur = useErreurSource();
	const [etat, setEtat] = useState<SanteApi | null>(null);

	useEffect(() => {
		const ac = new AbortController();
		sante(ac.signal)
			.then(setEtat)
			.catch(() => {
				/* l'erreur est déjà portée par le fournisseur */
			});
		return () => ac.abort();
	}, []);

	if (erreur) return <main role="alert">nie-site injoignable : {erreur}</main>;
	if (!capacites) return <main aria-busy="true">Mesure des capacités…</main>;

	return (
		<main>
			<h1>Aphrody</h1>
			{etat ? (
				<p>
					{etat.service} {etat.version} — API {etat.api}
				</p>
			) : null}
			<dl>
				<dt>VFS</dt>
				<dd>
					{capacites.vfs ? "prêt" : "absent"}
					{etat ? ` (${etat.capacites.vfs_entrees.toLocaleString("fr")} entrées)` : ""}
				</dd>
				<dt>Gisement</dt>
				<dd>{capacites.wiki ? "ouvert" : "absent"}</dd>
			</dl>
			{etat ? (
				<ul>
					{etat.vues.map((v) => (
						<li key={v.nom}>
							{v.nom} : {v.total === null ? "index en cours" : v.total.toLocaleString("fr")}
						</li>
					))}
				</ul>
			) : null}
		</main>
	);
}
