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
 */
import type { ContenuDossier } from "@niers/asset-source";
import { Badge, Callout, TitleBand, useAssetSource, useCapacites } from "@niers/inacord-ui";
import { useEffect, useState } from "react";

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
	const [erreur, setErreur] = useState<string | null>(null);

	useEffect(() => {
		if (!capacites?.vfs) return;
		const ac = new AbortController();
		setErreur(null);
		source
			.parcourir(prefixe, ac.signal)
			.then((c) => {
				if (!ac.signal.aborted) setContenu(c);
			})
			.catch((e: unknown) => {
				if (!ac.signal.aborted) setErreur(e instanceof Error ? e.message : String(e));
			});
		return () => ac.abort();
	}, [source, capacites?.vfs, prefixe]);

	if (!capacites) return <Callout>Mesure des capacités…</Callout>;
	if (!capacites.vfs) return <Callout>L'index du VFS n'est pas monté.</Callout>;
	if (erreur) return <Callout ton="alerte">Dossier illisible : {erreur}</Callout>;

	const segments = fil(prefixe);

	return (
		<section>
			<TitleBand>
				Explorateur{" "}
				{contenu ? (
					<Badge>
						{contenu.dossiers.length + contenu.fichiers.length}
					</Badge>
				) : null}
			</TitleBand>

			{/* Fil d'Ariane. `nav` + `aria-label` pour que la remontée soit annoncée comme telle. */}
			<nav aria-label="Chemin" style={{ margin: "var(--jeu-espace-m) 0", fontSize: "0.85rem" }}>
				<button type="button" onClick={() => setPrefixe("")} style={lien}>
					racine
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
				<Callout>Chargement…</Callout>
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
								style={{ ...ligne, color: "var(--jeu-accent-cyan)" }}
							>
								<span aria-hidden="true">📁</span> {d.split("/").filter(Boolean).at(-1) ?? d}
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
								<span style={{ color: "var(--jeu-surface-cendre)", fontSize: "0.8rem" }}>
									{taille(f.taille)}
								</span>
							</a>
						</li>
					))}
					{contenu.dossiers.length === 0 && contenu.fichiers.length === 0 ? (
						<Callout>Ce dossier est vide.</Callout>
					) : null}
				</ul>
			)}
		</section>
	);
}

const lien: React.CSSProperties = {
	background: "none",
	border: "none",
	color: "var(--jeu-texte-doux)",
	cursor: "pointer",
	font: "inherit",
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
	borderBottom: "1px solid rgb(99 216 252 / 12%)",
	color: "var(--jeu-texte-vif)",
	font: "inherit",
	textAlign: "left",
	cursor: "pointer",
};
