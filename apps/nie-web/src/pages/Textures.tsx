/**
 * `/textures` — le catalogue des textures du jeu, porté du wiki vers Aphrody.
 *
 * ## Ce qui change par rapport à la page d'origine
 *
 * L'ancienne (`legacy/app/textures/[[...path]]/page.tsx`, 337 lignes) parlait au VFS par la
 * couche `cpk/live` du wiki, elle-même adossée au disque du VPS. Celle-ci ne connaît que le
 * contrat : elle demande une page de catalogue à `AssetSource`, et l'hôte décide d'où elle
 * vient. Aphrody la sert par `/api/v1/textures`, Inacord la servirait par sa recherche native.
 *
 * ## La vue est un FILTRE, jamais un dossier
 *
 * `textures` ne désigne pas un répertoire du jeu : c'est un filtre enregistré sur l'espace VFS,
 * qui retient les extensions d'image (amendement A3). Le `chemin` de chaque élément est donc son
 * adresse complète et verbatim — c'est lui qu'on passe à `urlFichier()` ou `vignette()`, jamais
 * un chemin reconstruit à partir du nom.
 */
import type { EntreeVfs } from "@niers/asset-source";
import { Badge, Callout, TitleBand, useAssetSource, useCapacites } from "@niers/inacord-ui";
import { useEffect, useState } from "react";

/** Taille de page. Le serveur borne à 200 ; 60 tient dans une grille sans peser. */
const PAR_PAGE = 60;

/** Formate une taille en octets pour l'affichage. */
function taille(octets: number): string {
	if (octets < 1024) return `${octets} o`;
	if (octets < 1024 * 1024) return `${(octets / 1024).toFixed(1)} ko`;
	return `${(octets / (1024 * 1024)).toFixed(1)} Mo`;
}

export function Textures() {
	const source = useAssetSource();
	const capacites = useCapacites();
	const [page, setPage] = useState(1);
	const [elements, setElements] = useState<EntreeVfs[]>([]);
	const [total, setTotal] = useState(0);
	const [pages, setPages] = useState(0);
	const [erreur, setErreur] = useState<string | null>(null);
	const [charge, setCharge] = useState(false);

	useEffect(() => {
		// `catalogue` est OPTIONNEL dans le contrat : un hôte qui ne sait pas paginer sur un jeu
		// d'extensions ne l'expose pas. On teste sa présence plutôt que de supposer.
		if (!capacites?.vfs || !source.catalogue) return;
		const ac = new AbortController();
		setCharge(false);
		setErreur(null);
		source
			.catalogue("textures", { page, parPage: PAR_PAGE, signal: ac.signal })
			.then((p) => {
				if (ac.signal.aborted) return;
				setElements(p.elements);
				setTotal(p.total);
				setPages(p.pages);
				setCharge(true);
			})
			.catch((e: unknown) => {
				if (!ac.signal.aborted) setErreur(e instanceof Error ? e.message : String(e));
			});
		return () => ac.abort();
	}, [source, capacites?.vfs, page]);

	if (!capacites) return <Callout>Mesure des capacités…</Callout>;
	if (!capacites.vfs) {
		return <Callout>L'index du VFS n'est pas monté : le catalogue n'est pas encore lisible.</Callout>;
	}
	if (!source.catalogue) {
		return <Callout ton="alerte">Cet hôte ne sait pas paginer les catalogues.</Callout>;
	}
	if (erreur) return <Callout ton="alerte">Catalogue illisible : {erreur}</Callout>;

	return (
		<section>
			<TitleBand>
				Textures {total ? <Badge>{total.toLocaleString("fr")}</Badge> : null}
			</TitleBand>

			{!charge ? (
				<Callout>Chargement…</Callout>
			) : (
				<ul
					style={{
						display: "grid",
						gridTemplateColumns: "repeat(auto-fill, minmax(160px, 1fr))",
						gap: "var(--jeu-espace-m)",
						listStyle: "none",
						margin: "var(--jeu-espace-l) 0",
						padding: 0,
					}}
				>
					{elements.map((t) => (
						<li key={t.chemin}>
							<a
								href={source.urlFichier(t.chemin)}
								style={{
									display: "block",
									background: "var(--jeu-fond-nuit)",
									border: "1px solid rgb(99 216 252 / 25%)",
									borderRadius: "var(--jeu-rayon)",
									color: "var(--jeu-texte-vif)",
									textDecoration: "none",
									overflow: "hidden",
								}}
							>
								{/* La vignette est produite par l'hôte : URL HTTP ici, `data:` sur le
								    desktop. `loading="lazy"` évite de décoder 60 images d'un coup. */}
								{source.urlTexture ? (
									<img
										src={source.urlTexture(t.chemin)}
										alt=""
										loading="lazy"
										decoding="async"
										style={{
											width: "100%",
											aspectRatio: "1",
											objectFit: "contain",
											background: "var(--jeu-fond-abysse)",
											imageRendering: "pixelated",
										}}
									/>
								) : null}
								<div style={{ padding: "var(--jeu-espace-s)" }}>
									{/* Le NOM, pas le chemin : celui-ci fait souvent plus de 80 caractères. */}
									<div
										style={{
											fontSize: "0.8rem",
											overflow: "hidden",
											textOverflow: "ellipsis",
											whiteSpace: "nowrap",
										}}
										title={t.chemin}
									>
										{t.nom}
									</div>
									<div style={{ fontSize: "0.7rem", color: "var(--jeu-surface-cendre)" }}>
										{taille(t.taille)}
									</div>
								</div>
							</a>
						</li>
					))}
				</ul>
			)}

			{pages > 1 ? (
				<nav
					aria-label="Pagination"
					style={{ display: "flex", alignItems: "center", gap: "var(--jeu-espace-m)" }}
				>
					<button type="button" disabled={page <= 1} onClick={() => setPage((p) => p - 1)}>
						Précédent
					</button>
					<span aria-live="polite">
						Page {page} sur {pages.toLocaleString("fr")}
					</span>
					<button type="button" disabled={page >= pages} onClick={() => setPage((p) => p + 1)}>
						Suivant
					</button>
				</nav>
			) : null}
		</section>
	);
}
