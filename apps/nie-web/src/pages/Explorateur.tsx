/**
 * L'explorateur du VFS — la navigation par dossiers, qui manquait à Aphrody.
 *
 * Les catalogues (`Catalogue.tsx`) montrent des filtres : tout ce qui est une texture, tout ce
 * qui est un son. Ils ne disent rien de la STRUCTURE. Or les fichiers du jeu sont rangés, et
 * ce rangement porte du sens — `data/dx11/chr/` n'est pas `data/common/sound/`.
 *
 * Cette page utilise `parcourir()`, la seule méthode du contrat que les deux hôtes implémentent
 * de la même façon : Aphrody par `/b/<préfixe>`, Inacord par sa commande `ls`. C'est donc la
 * surface la plus portable du contrat, et celle qui marche même sans catalogue paginé.
 *
 * Les chemins du jeu sont le CONTENU de cette page, et à ce titre ils s'affichent. Ce qui n'y a
 * pas sa place, et en a disparu, c'est le vocabulaire d'implémentation — index, montage, hôte,
 * message d'erreur du transport : le lecteur explore des fichiers, il n'exploite pas un service.
 */
import type { ContenuDossier } from "@niers/asset-source";
import { useAssetSource, useCapacites } from "@niers/inacord-ui";
import { useEffect, useState } from "react";
import { accorde, Note, TitreVue } from "./Ecran";

/** Formate une taille en octets. */
function taille(octets: number): string {
	if (octets < 1024) return `${octets} o`;
	if (octets < 1024 * 1024) return `${(octets / 1024).toFixed(1)} ko`;
	return `${(octets / (1024 * 1024)).toFixed(1)} Mo`;
}

/**
 * Découpe un préfixe en segments cliquables.
 *
 * Chaque segment porte le chemin CUMULÉ, pas son seul nom : c'est ce qui permet de remonter
 * d'un cran sans reconstruire l'adresse — et un chemin VFS reconstruit est presque toujours
 * faux, les fichiers du jeu portant un numéro de version.
 */
function fil(prefixe: string): { nom: string; chemin: string }[] {
	const segments = prefixe.split("/").filter(Boolean);
	return segments.map((nom, i) => ({ nom, chemin: segments.slice(0, i + 1).join("/") }));
}

export function Explorateur() {
	const source = useAssetSource();
	const capacites = useCapacites();
	const [prefixe, setPrefixe] = useState("");
	const [contenu, setContenu] = useState<ContenuDossier | null>(null);
	const [erreur, setErreur] = useState(false);

	useEffect(() => {
		if (!capacites?.vfs) return;
		const ac = new AbortController();
		setErreur(false);
		source
			.parcourir(prefixe, ac.signal)
			.then((c) => {
				if (!ac.signal.aborted) setContenu(c);
			})
			.catch(() => {
				if (!ac.signal.aborted) setErreur(true);
			});
		return () => ac.abort();
	}, [source, capacites?.vfs, prefixe]);

	if (!capacites) return <Note>Chargement…</Note>;
	if (!capacites.vfs) {
		return <Note>L'arborescence est en cours de préparation. Elle s'affichera dès qu'elle sera prête.</Note>;
	}
	if (erreur) {
		return <Note ton="alerte">Ce dossier n'a pas pu être ouvert. Réessayez dans un instant.</Note>;
	}

	const segments = fil(prefixe);
	const elements = contenu ? contenu.dossiers.length + contenu.fichiers.length : 0;

	return (
		<section>
			<TitreVue appoint={contenu ? accorde(elements, "entrée") : undefined}>
				Explorer
			</TitreVue>

			{/* Fil d'Ariane. `nav` + `aria-label` pour que la remontée soit annoncée comme telle. */}
			<nav aria-label="Chemin" style={{ margin: "var(--jeu-espace-m) 0", fontSize: "0.9rem" }}>
				<button type="button" onClick={() => setPrefixe("")} style={lien}>
					Racine
				</button>
				{segments.map((s) => (
					<span key={s.chemin}>
						<span aria-hidden="true" style={{ opacity: 0.5 }}> / </span>
						<button type="button" onClick={() => setPrefixe(s.chemin)} style={lien}>
							{s.nom}
						</button>
					</span>
				))}
			</nav>

			{!contenu ? (
				<Note>Chargement…</Note>
			) : (
				<ul style={{ listStyle: "none", margin: 0, padding: 0 }}>
					{/*
					  * `dossiers` porte des chemins COMPLETS (`data/common`), pas des noms relatifs.
					  * Mesure du 2026-09-05 : `/b/data` rend `["data/common", "data/dx11"]`. Les
					  * concatener au prefixe courant produirait `data/data/common` — un 404 que rien
					  * n'expliquerait, et le genre de defaut qu'on impute au serveur avant de
					  * regarder son propre code. On navigue donc vers la valeur telle quelle, et on
					  * n'affiche que son dernier segment.
					  */}
					{contenu.dossiers.map((d) => (
						<li key={d}>
							<button
								type="button"
								onClick={() => setPrefixe(d)}
								style={{ ...ligne, fontWeight: 800 }}
							>
								<IconeDossier /> {d.split("/").filter(Boolean).at(-1) ?? d}
							</button>
						</li>
					))}
					{contenu.fichiers.map((f) => (
						<li key={f.chemin}>
							{/* Le chemin VERBATIM sert d'adresse : extension du jeu conservée. */}
							<a href={source.urlFichier(f.chemin)} style={{ ...ligne, textDecoration: "none" }}>
								<span style={{ flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis" }}>
									{f.nom}
								</span>
								<span style={{ color: "var(--jeu-tuile-bas)", fontSize: "0.8rem" }}>
									{taille(f.taille)}
								</span>
							</a>
						</li>
					))}
					{elements === 0 ? <Note>Ce dossier est vide.</Note> : null}
				</ul>
			)}
		</section>
	);
}

/**
 * Le pictogramme d'un dossier — un tracé, pas une émoji.
 *
 * `📁` dépend d'une police d'émojis installée sur la machine du lecteur : là où elle manque, le
 * caractère se rend en rectangle vide, et rien ne le signale. Un `svg` se dessine partout, prend
 * la couleur du texte et suit sa taille.
 */
function IconeDossier() {
	return (
		<svg
			width="15"
			height="15"
			viewBox="0 0 24 24"
			fill="none"
			stroke="currentColor"
			strokeWidth="2"
			strokeLinecap="round"
			strokeLinejoin="round"
			aria-hidden="true"
			focusable="false"
		>
			<path d="M4 5h6l2 2h8v12H4z" />
		</svg>
	);
}

const lien: React.CSSProperties = {
	background: "none",
	border: "none",
	color: "var(--jeu-tuile-bas)",
	cursor: "pointer",
	font: "inherit",
	fontWeight: 700,
	padding: 0,
	textDecoration: "underline",
};

const ligne: React.CSSProperties = {
	display: "flex",
	alignItems: "center",
	gap: "var(--jeu-espace-s)",
	width: "100%",
	padding: "var(--jeu-espace-xs) var(--jeu-espace-s)",
	background: "none",
	border: "none",
	borderBottom: "1px solid var(--jeu-tuile-bord)",
	color: "var(--jeu-nuit-profonde)",
	font: "inherit",
	textAlign: "left",
	cursor: "pointer",
};
