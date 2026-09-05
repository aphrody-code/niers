import { useEffect, useState } from "react";
import { type SanteApi, sante } from "./nie-site";

/**
 * Coquille d'Aphrody. L'interface viendra de `packages/inacord-ui`, partagee
 * avec Inacord et montee ici par la source web ; pour l'instant cette page
 * n'affiche que ce que le serveur DECLARE savoir servir — un compte, jamais une
 * intention.
 */
export function App() {
	const [etat, setEtat] = useState<SanteApi | null>(null);
	const [erreur, setErreur] = useState<string | null>(null);

	useEffect(() => {
		const ac = new AbortController();
		sante(ac.signal)
			.then(setEtat)
			.catch((e: unknown) => {
				if (!ac.signal.aborted) setErreur(e instanceof Error ? e.message : String(e));
			});
		return () => ac.abort();
	}, []);

	if (erreur) return <main role="alert">nie-site injoignable : {erreur}</main>;
	if (!etat) return <main aria-busy="true">Mesure en cours…</main>;

	return (
		<main>
			<h1>Aphrody</h1>
			<p>
				{etat.service} {etat.version} — API {etat.api}
			</p>
			<dl>
				<dt>VFS</dt>
				<dd>
					{etat.capacites.vfs} ({etat.capacites.vfs_entrees.toLocaleString("fr")} entrees
					{etat.capacites.vfs_dump ? ", montage dump" : ""})
				</dd>
				<dt>Gisement</dt>
				<dd>{etat.capacites.gisement ? "ouvert" : "absent"}</dd>
			</dl>
			<ul>
				{etat.vues.map((v) => (
					<li key={v.nom}>
						{v.nom} : {v.total === null ? "index en cours" : v.total.toLocaleString("fr")}
					</li>
				))}
			</ul>
		</main>
	);
}
