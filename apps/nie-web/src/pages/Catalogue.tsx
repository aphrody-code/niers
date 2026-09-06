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
 *
 * ## L'habillage suit celui de l'accueil
 *
 * Fond clair, titres en bandeau biseauté, cartes blanches cerclées de bleu : les mêmes formes
 * que le menu principal. La page était auparavant rendue sur fond noir, avec ses propres
 * bandeaux et ses propres pastilles — un second thème pour le même site.
 */
import type { EntreeVfs, VueCatalogue } from "@niers/asset-source";
import { useAssetSource, useCapacites } from "@niers/inacord-ui";
import { useEffect, useMemo, useRef, useState } from "react";
import { libelleEntree } from "../entrees";
import { accorde, Note, TitreVue } from "./Ecran";
import { Modeles3D } from "./Modeles3D";

/**
 * Tailles de page proposées. Le serveur borne à **200** (`config.rs:27`) : proposer davantage
 * ferait promettre au lecteur un réglage que le serveur ramènerait en silence.
 */
const TAILLES_PAGE = [60, 100, 200] as const;

/** Taille de page par défaut : 60 tient dans une grille sans peser. */
const PAR_PAGE = 60;

/**
 * L'état de filtre de cette page, tel qu'il vit dans l'URL.
 *
 * Il y vit parce que sinon il ne se partage pas, ne survit pas au rechargement et n'est pas
 * indexable — et parce que la mesure du 2026-09-06 a montré que le serveur servait **41 filtres
 * sur 48** dont la page n'utilisait qu'un seul.
 */
type EtatFiltre = {
	q: string;
	ext: string;
	tri: "nom" | "taille";
	ordre: "asc" | "desc";
	parPage: number;
	page: number;
};

/** Lit l'état depuis l'URL courante. Une valeur illisible retombe sur son défaut. */
function etatDeLUrl(): EtatFiltre {
	const p = new URLSearchParams(window.location.search);
	const parPage = Number(p.get("par_page"));
	const page = Number(p.get("page"));
	return {
		q: p.get("q") ?? "",
		ext: p.get("ext") ?? "",
		tri: p.get("tri") === "taille" ? "taille" : "nom",
		ordre: p.get("ordre") === "desc" ? "desc" : "asc",
		// `includes` sur la liste servie, jamais la valeur brute : un `par_page=100000` tapé
		// dans la barre d'adresse ne doit pas devenir une promesse que le serveur rabotera.
		parPage: TAILLES_PAGE.includes(parPage as (typeof TAILLES_PAGE)[number])
			? parPage
			: PAR_PAGE,
		page: Number.isFinite(page) && page >= 1 ? page : 1,
	};
}

/**
 * Écrit l'état dans l'URL, sans empiler d'entrée d'historique.
 *
 * `replaceState` : filtrer n'est pas naviguer. Le `pathname` n'est pas touché — c'est lui qui
 * porte la vue (`App.tsx:64-66`).
 */
function ecrireUrl(e: EtatFiltre) {
	const url = new URL(window.location.href);
	const paires: [string, string][] = [
		["q", e.q],
		["ext", e.ext],
		["tri", e.tri === "nom" ? "" : e.tri],
		["ordre", e.ordre === "asc" ? "" : e.ordre],
		["par_page", e.parPage === PAR_PAGE ? "" : String(e.parPage)],
		["page", e.page === 1 ? "" : String(e.page)],
	];
	// Un défaut ne s'écrit pas dans l'URL : `?tri=nom&ordre=asc&page=1` est du bruit qui rend
	// deux adresses différentes pour le même écran, et casse le partage autant que l'absence.
	for (const [cle, valeur] of paires) {
		if (valeur) url.searchParams.set(cle, valeur);
		else url.searchParams.delete(cle);
	}
	window.history.replaceState(window.history.state, "", url);
}

/**
 * L'aiguillage des quatre vues — et la seule qui ne soit pas un filtre d'extensions.
 *
 * `modeles` a quitté ce fichier : le filtre `.g4md`/`.g4mg`/`.g4sk`/`.g4mt`/`.g4pk` liste des
 * PIÈCES, pas des modèles. Un `.g4mg` seul n'est qu'un tampon de géométrie — ni texture, ni
 * squelette, ni recette — et la grille n'en montrait donc qu'un nom et une taille : 143 000
 * fichiers dont aucun ne s'affichait. `Modeles3D` liste les 6 191 **codes assemblables** que
 * publie `/api/v1/3d`, avec le rendu réel de chacun.
 *
 * L'aiguillage est un composant à part, sans le moindre hook, et ce n'est pas un détail : un
 * `if` posé au milieu de `CatalogueVfs` changerait le nombre de hooks appelés d'un rendu à
 * l'autre en passant de `textures` à `modeles`, ce que React refuse. Ici, changer de vue
 * démonte un composant et en monte un autre.
 */
export function Catalogue({ vue }: { vue: VueCatalogue }) {
	if (vue === "modeles") return <Modeles3D />;
	return <CatalogueVfs vue={vue} />;
}

/** Formate une taille en octets pour l'affichage. */
function taille(octets: number): string {
	if (octets < 1024) return `${octets} o`;
	if (octets < 1024 * 1024) return `${(octets / 1024).toFixed(1)} ko`;
	return `${(octets / (1024 * 1024)).toFixed(1)} Mo`;
}

/** Un champ de la barre de réglages. */
const CHAMP: React.CSSProperties = {
	padding: "var(--jeu-espace-xs) var(--jeu-espace-s)",
	background: "#fff",
	border: "2px solid var(--jeu-tuile-bord)",
	borderRadius: "var(--jeu-rayon)",
	color: "var(--jeu-nuit-profonde)",
	font: "inherit",
};

/** Une étiquette de réglage — un `label` réel, pas un texte posé à côté. */
const ETIQUETTE: React.CSSProperties = {
	display: "inline-flex",
	alignItems: "center",
	gap: "var(--jeu-espace-xs)",
	fontWeight: 700,
};

function CatalogueVfs({ vue }: { vue: VueCatalogue }) {
	const source = useAssetSource();
	const capacites = useCapacites();
	const initial = useMemo(etatDeLUrl, []);
	const [etat, setEtat] = useState<EtatFiltre>(initial);
	const { page, q: filtre, ext, tri, ordre, parPage } = etat;
	// Changer de vue ramene a la page 1 : garder la page 900 en passant d'un catalogue de 904
	// pages a un catalogue de 4 afficherait un vide que rien n'expliquerait. Les filtres, eux,
	// sont remis a zero pour la meme raison — `ext=dds` n'a aucun sens sur les sons.
	const premier = useRef(true);
	useEffect(() => {
		if (premier.current) {
			premier.current = false;
			return;
		}
		setSaisie("");
		setEtat({ q: "", ext: "", tri: "nom", ordre: "asc", parPage: PAR_PAGE, page: 1 });
	}, [vue]);
	const [elements, setElements] = useState<EntreeVfs[]>([]);
	const [total, setTotal] = useState(0);
	const [pages, setPages] = useState(0);
	const [erreur, setErreur] = useState(false);
	const [charge, setCharge] = useState(false);
	// `saisie` suit le champ, `etat.q` ce qui a ete envoye : sans ce decalage, chaque frappe
	// declencherait une requete sur 143 246 chemins.
	const [saisie, setSaisie] = useState(initial.q);

	useEffect(() => {
		// `catalogue` est OPTIONNEL dans le contrat : un hôte qui ne sait pas paginer sur un jeu
		// d'extensions ne l'expose pas. On teste sa présence plutôt que de supposer.
		if (!capacites?.vfs || !source.catalogue) return;
		const ac = new AbortController();
		setCharge(false);
		setErreur(false);
		ecrireUrl(etat);
		source
			.catalogue(vue, { page, parPage, q: filtre, ext, tri, ordre, signal: ac.signal })
			.then((p) => {
				if (ac.signal.aborted) return;
				setElements(p.elements);
				setTotal(p.total);
				setPages(p.pages);
				setCharge(true);
			})
			.catch(() => {
				// Le message d'erreur du transport ne s'affiche pas : « Failed to fetch » ou un
				// code HTTP ne dit rien à qui consulte la page, et le seul geste utile ne dépend
				// pas de lui.
				if (!ac.signal.aborted) setErreur(true);
			});
		return () => ac.abort();
	}, [source, capacites?.vfs, vue, etat, page, filtre, ext, tri, ordre, parPage]);

	const titre = libelleEntree(vue);

	if (!capacites) return <Note>Chargement…</Note>;
	if (!capacites.vfs || !source.catalogue) {
		return <Note>Le catalogue est en cours de préparation. Il s'affichera dès qu'il sera prêt.</Note>;
	}
	if (erreur) {
		return <Note ton="alerte">Ce catalogue n'a pas pu être chargé. Réessayez dans un instant.</Note>;
	}

	return (
		<section>
			<TitreVue appoint={total ? accorde(total, "élément") : undefined}>
				{titre}
			</TitreVue>

			<form
				onSubmit={(e) => {
					e.preventDefault();
					setEtat((v) => ({ ...v, q: saisie.trim(), page: 1 }));
				}}
				style={{ display: "flex", gap: "var(--jeu-espace-s)", margin: "var(--jeu-espace-m) 0" }}
			>
				<input
					type="search"
					value={saisie}
					onChange={(e) => setSaisie(e.target.value)}
					placeholder={`Chercher dans les ${titre.toLowerCase()}…`}
					aria-label={`Chercher dans ${titre}`}
					style={{
						flex: 1,
						padding: "var(--jeu-espace-s) var(--jeu-espace-m)",
						background: "#fff",
						border: "2px solid var(--jeu-tuile-bord)",
						borderRadius: "var(--jeu-rayon)",
						color: "var(--jeu-nuit-profonde)",
						font: "inherit",
					}}
				/>
				<button type="submit" style={BOUTON}>
					Chercher
				</button>
				{filtre || ext || tri !== "nom" || ordre !== "asc" ? (
					<button
						type="button"
						onClick={() => {
							setSaisie("");
							setEtat((e) => ({
								...e,
								q: "",
								ext: "",
								tri: "nom",
								ordre: "asc",
								page: 1,
							}));
						}}
						style={BOUTON}
					>
						Effacer
					</button>
				) : null}
			</form>

			{/*
			  * Les trois réglages que le serveur sert déjà et que cette page n'utilisait pas :
			  * l'extension (le catalogue en couvre jusqu'à six par vue), le tri, et la taille de
			  * page — plafonnée à 200 côté serveur, d'où une liste close plutôt qu'un champ
			  * libre qui promettrait ce que le serveur raboterait.
			  */}
			<div
				style={{
					display: "flex",
					flexWrap: "wrap",
					alignItems: "center",
					gap: "var(--jeu-espace-m)",
					margin: "0 0 var(--jeu-espace-m)",
					fontSize: "0.9rem",
				}}
			>
				<label style={ETIQUETTE}>
					Extension
					<input
						type="text"
						value={ext}
						onChange={(e) =>
							setEtat((v) => ({
								...v,
								ext: e.target.value.trim().replace(/^\./, ""),
								page: 1,
							}))
						}
						placeholder="toutes"
						style={{ ...CHAMP, width: "7rem" }}
					/>
				</label>
				<label style={ETIQUETTE}>
					Trier par
					<select
						value={`${tri}-${ordre}`}
						onChange={(e) => {
							const [t, o] = e.target.value.split("-");
							setEtat((v) => ({
								...v,
								tri: t === "taille" ? "taille" : "nom",
								ordre: o === "desc" ? "desc" : "asc",
								page: 1,
							}));
						}}
						style={CHAMP}
					>
						<option value="nom-asc">Nom (A→Z)</option>
						<option value="nom-desc">Nom (Z→A)</option>
						<option value="taille-asc">Taille (petits d'abord)</option>
						<option value="taille-desc">Taille (gros d'abord)</option>
					</select>
				</label>
				<label style={ETIQUETTE}>
					Par page
					<select
						value={String(parPage)}
						onChange={(e) =>
							setEtat((v) => ({ ...v, parPage: Number(e.target.value), page: 1 }))
						}
						style={CHAMP}
					>
						{TAILLES_PAGE.map((n) => (
							<option key={n} value={n}>
								{n}
							</option>
						))}
					</select>
				</label>
			</div>

			{!charge ? (
				<Note>Chargement…</Note>
			) : elements.length === 0 ? (
				<Note>Aucun élément ne correspond à cette recherche.</Note>
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
									background: "#fff",
									border: "2px solid var(--jeu-tuile-bord)",
									borderRadius: "var(--jeu-rayon)",
									color: "var(--jeu-nuit-profonde)",
									textDecoration: "none",
									overflow: "hidden",
									boxShadow: "var(--jeu-ombre-tuile)",
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
										style={{ width: "100%", aspectRatio: "16/9", background: "var(--jeu-nuit-profonde)" }}
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
											background: "var(--jeu-ciel-clair)",
											imageRendering: "pixelated",
										}}
									/>
								) : null}
								<div style={{ padding: "var(--jeu-espace-s)" }}>
									{/* Le NOM, pas le chemin : celui-ci fait souvent plus de 80 caractères. */}
									<div
										style={{
											fontSize: "0.8rem",
											fontWeight: 700,
											overflow: "hidden",
											textOverflow: "ellipsis",
											whiteSpace: "nowrap",
										}}
										title={t.chemin}
									>
										{t.nom}
									</div>
									<div style={{ fontSize: "0.7rem", color: "var(--jeu-tuile-bas)" }}>
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
					<button
						type="button"
						disabled={page <= 1}
						onClick={() => setEtat((e) => ({ ...e, page: e.page - 1 }))}
						style={BOUTON}
					>
						Précédent
					</button>
					<span aria-live="polite" style={{ fontWeight: 700 }}>
						Page {page} sur {pages.toLocaleString("fr")}
					</span>
					<button
						type="button"
						disabled={page >= pages}
						onClick={() => setEtat((e) => ({ ...e, page: e.page + 1 }))}
						style={BOUTON}
					>
						Suivant
					</button>
				</nav>
			) : null}
		</section>
	);
}

/** Les boutons de la page, dans la teinte des tuiles du menu. */
const BOUTON: React.CSSProperties = {
	padding: "var(--jeu-espace-s) var(--jeu-espace-l)",
	border: 0,
	borderRadius: "var(--jeu-rayon)",
	background: "linear-gradient(180deg, var(--jeu-tuile-haut), var(--jeu-tuile-bas))",
	color: "var(--jeu-texte-vif)",
	font: "inherit",
	fontWeight: 800,
	cursor: "pointer",
};
