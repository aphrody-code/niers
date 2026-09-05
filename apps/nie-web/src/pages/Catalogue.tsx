/**
 * Les quatre catalogues du jeu — textures, modèles, sons, vidéos — portés du wiki vers Aphrody.
 *
 * ## Une page pour quatre vues, et pourquoi
 *
 * Les quatre pages d'origine (`legacy/app/{textures,modeles,sons,videos}`, ~1 500 lignes à
 * elles quatre) faisaient la même chose : lister un filtre du VFS, paginer, afficher une
 * vignette. Elles divergeaient sur des détails d'affichage et sur rien d'autre — quatre copies
 * d'une même logique, qui dérivaient chacune de leur côté.
 *
 * Ici, la vue est un PARAMÈTRE. Ce qui diffère vraiment entre un son et une texture — la
 * présence d'un aperçu visuel — se lit dans les capacités de l'hôte, pas dans quatre fichiers.
 *
 * ## Ce qui change par rapport aux pages d'origine
 *
 * Elles parlaient au VFS par la couche `cpk/live` du wiki, adossée au disque du VPS. Celle-ci
 * ne connaît que le contrat : elle demande une page de catalogue à `AssetSource`, et l'hôte
 * décide d'où elle vient. Aphrody la sert par `/api/v1/<vue>`, Inacord par sa recherche
 * native.
 *
 * ## La vue est un FILTRE, jamais un dossier
 *
 * `textures` ne désigne pas un répertoire du jeu : c'est un filtre enregistré sur l'espace VFS,
 * qui retient les extensions d'image (amendement A3). Le `chemin` de chaque élément est donc son
 * adresse complète et verbatim — c'est lui qu'on passe à `urlFichier()` ou `vignette()`, jamais
 * un chemin reconstruit à partir du nom.
 */
import type { EntreeVfs, VueCatalogue } from "@niers/asset-source";
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

/** Libellés affichés, au singulier près — le code du jeu ne les porte pas. */
const LIBELLES: Record<VueCatalogue, string> = {
	textures: "Textures",
	modeles: "Modèles",
	sons: "Sons",
	videos: "Vidéos",
};

export function Catalogue({ vue }: { vue: VueCatalogue }) {
	const source = useAssetSource();
	const capacites = useCapacites();
	const [page, setPage] = useState(1);
	// Changer de vue ramene a la page 1 : garder la page 900 en passant d'un catalogue de 904
	// pages a un catalogue de 4 afficherait un vide que rien n'expliquerait.
	useEffect(() => {
		setPage(1);
		setSaisie("");
		setFiltre("");
	}, [vue]);
	const [elements, setElements] = useState<EntreeVfs[]>([]);
	const [total, setTotal] = useState(0);
	const [pages, setPages] = useState(0);
	const [erreur, setErreur] = useState<string | null>(null);
	const [charge, setCharge] = useState(false);
	// `saisie` suit le champ, `filtre` ce qui a ete envoye : sans ce decalage, chaque frappe
	// declencherait une requete sur 143 246 chemins.
	const [saisie, setSaisie] = useState("");
	const [filtre, setFiltre] = useState("");

	useEffect(() => {
		// `catalogue` est OPTIONNEL dans le contrat : un hôte qui ne sait pas paginer sur un jeu
		// d'extensions ne l'expose pas. On teste sa présence plutôt que de supposer.
		if (!capacites?.vfs || !source.catalogue) return;
		const ac = new AbortController();
		setCharge(false);
		setErreur(null);
		source
			.catalogue(vue, { page, parPage: PAR_PAGE, q: filtre, signal: ac.signal })
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
	}, [source, capacites?.vfs, page, vue, filtre]);

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
				{LIBELLES[vue]} {total ? <Badge>{total.toLocaleString("fr")}</Badge> : null}
			</TitleBand>

			<form
				onSubmit={(e) => {
					e.preventDefault();
					setPage(1);
					setFiltre(saisie);
				}}
				style={{ display: "flex", gap: "var(--jeu-espace-s)", margin: "var(--jeu-espace-m) 0" }}
			>
				<input
					type="search"
					value={saisie}
					onChange={(e) => setSaisie(e.target.value)}
					placeholder="Chercher dans les chemins…"
					aria-label={`Chercher dans ${LIBELLES[vue]}`}
					style={{
						flex: 1,
						padding: "var(--jeu-espace-s)",
						background: "var(--jeu-fond-abysse)",
						border: "1px solid rgb(99 216 252 / 30%)",
						borderRadius: "var(--jeu-rayon)",
						color: "var(--jeu-texte-vif)",
						font: "inherit",
					}}
				/>
				<button type="submit">Chercher</button>
				{filtre ? (
					<button
						type="button"
						onClick={() => {
							setSaisie("");
							setFiltre("");
							setPage(1);
						}}
					>
						Effacer
					</button>
				) : null}
			</form>

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
							{/* Pour les sons, le conteneur n'est PAS un lien : un clic sur « lire »
							    declencherait la navigation au lieu de la lecture. */}
							<a
								href={vue === "sons" || vue === "videos" ? undefined : source.urlFichier(t.chemin)}
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
								{vue === "videos" && source.urlVideo ? (
									// Meme raison que pour l'audio : `preload="none"`, sinon ouvrir la page
									// tirerait 60 videos. `playsInline` evite que Safari mobile ne passe en
									// plein ecran des la lecture, ce qui sort l'utilisateur du catalogue.
									// biome-ignore lint/a11y/useMediaCaption: une cinematique du jeu n'a pas
									// de piste de sous-titres separee, et en inventer une serait faux.
									<video
										controls
										preload="none"
										playsInline
										src={source.urlVideo(t.chemin)}
										style={{ width: "100%", aspectRatio: "16/9", background: "var(--jeu-fond-abysse)" }}
									/>
								) : vue === "sons" && source.urlAudio ? (
									// `preload="none"` : 60 lecteurs sur une page ne doivent pas declencher
									// 60 telechargements. Le navigateur n'ira chercher les octets qu'au
									// premier clic sur « lire ».
									// biome-ignore lint/a11y/useMediaCaption: un effet sonore du jeu n'a pas
									// de piste de sous-titres, et en inventer une serait faux.
									<audio
										controls
										preload="none"
										src={source.urlAudio(t.chemin)}
										style={{ width: "100%", height: 32 }}
									/>
								) : vue === "textures" && source.urlTexture ? (
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
