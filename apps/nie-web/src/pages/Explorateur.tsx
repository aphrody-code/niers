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
 *
 * ## Les filtres, et pourquoi ils arrivent après
 *
 * `scripts/validation/mesurer-matrice-filtres.sh` a mesuré le 2026-09-06 que le serveur servait
 * **34 filtres sur 48** et que cette page en utilisait **zéro** : l'écart n'était plus « il
 * manque du serveur », il était « le client n'utilise pas ce qui est servi ». Deux filtres sont
 * câblés ici — la sous-chaîne et l'extension — parce que ce sont ceux que `/b` applique, et
 * qu'un troisième qui ne serait pas appliqué serait pire que pas de troisième du tout.
 *
 * Trois choix qui ne sont pas cosmétiques :
 *
 * - **le filtre passe par l'URL** (`?d=…&q=…&ext=…`). Sans cela il n'est ni partageable, ni
 *   indexable, ni conservé au rechargement — et ajouter dix facettes en `useState` ne ferait
 *   que dupliquer une dette déjà relevée sur Inacord ;
 * - **la recherche se soumet**, elle ne se déclenche pas à la frappe : chaque caractère
 *   coûterait une requête au serveur pour un résultat que personne ne lit ;
 * - **le compte est celui du serveur**, pas la longueur du tableau reçu. Les deux diffèrent dès
 *   qu'une page est tronquée, et afficher la seconde en la nommant « fichiers » serait faux.
 */
import type { ContenuDossier } from "@niers/asset-source";
import { useAssetSource, useCapacites } from "@niers/inacord-ui";
import { useEffect, useMemo, useState } from "react";
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

/**
 * L'état lisible dans l'URL courante.
 *
 * Lu une fois au montage : c'est l'URL qui amorce la page, ensuite c'est la page qui écrit
 * l'URL. L'inverse — relire l'URL à chaque rendu — ferait de chaque frappe une navigation.
 */
type EtatExplorateur = {
	prefixe: string;
	q: string;
	ext: string;
	tri: "nom" | "taille";
	ordre: "asc" | "desc";
	/** Taille minimale en Mo, telle qu'elle est SAISIE. Vide = pas de borne. */
	minMo: string;
};

function etatDeLUrl(): EtatExplorateur {
	const p = new URLSearchParams(window.location.search);
	return {
		prefixe: p.get("d") ?? "",
		q: p.get("q") ?? "",
		ext: p.get("ext") ?? "",
		tri: p.get("tri") === "taille" ? "taille" : "nom",
		ordre: p.get("ordre") === "desc" ? "desc" : "asc",
		minMo: p.get("min_mo") ?? "",
	};
}

/** Un mégaoctet, en octets — l'unité que le serveur attend. */
const MO = 1024 * 1024;

/**
 * Convertit une saisie en mégaoctets vers la borne en octets que le serveur attend.
 *
 * `undefined` pour une saisie vide **ou illisible** : envoyer `NaN` ferait un `400` sur une
 * frappe en cours, et envoyer `0` filtrerait sur les fichiers de zéro octet — deux façons de
 * transformer une hésitation en résultat.
 */
function bornesEnOctets(minMo: string): number | undefined {
	const n = Number(minMo.replace(",", "."));
	return minMo.trim() && Number.isFinite(n) && n >= 0 ? Math.round(n * MO) : undefined;
}

/**
 * Écrit l'état dans l'URL sans empiler d'entrée d'historique.
 *
 * `replaceState` et non `pushState` : filtrer n'est pas naviguer, et vingt frappes ne doivent
 * pas coûter vingt appuis sur « précédent ». Le `pathname` n'est pas touché — c'est lui qui
 * porte la vue (`App.tsx:64-66`), et l'écraser ferait sortir de la page.
 */
function ecrireUrl(etat: EtatExplorateur) {
	const url = new URL(window.location.href);
	// Un DÉFAUT ne s'écrit pas : `?tri=nom&ordre=asc` donnerait deux adresses pour le même
	// écran, et casserait le partage autant que l'absence d'état.
	for (const [cle, valeur] of [
		["d", etat.prefixe],
		["q", etat.q],
		["ext", etat.ext],
		["tri", etat.tri === "nom" ? "" : etat.tri],
		["ordre", etat.ordre === "asc" ? "" : etat.ordre],
		["min_mo", etat.minMo],
	] as const) {
		if (valeur) url.searchParams.set(cle, valeur);
		else url.searchParams.delete(cle);
	}
	window.history.replaceState(window.history.state, "", url);
}

export function Explorateur() {
	const source = useAssetSource();
	const capacites = useCapacites();
	const initial = useMemo(etatDeLUrl, []);
	const [prefixe, setPrefixe] = useState(initial.prefixe);
	// `saisie` est ce qui est tapé, `q` ce qui est appliqué. Les confondre relancerait une
	// requête par caractère.
	const [saisie, setSaisie] = useState(initial.q);
	const [q, setQ] = useState(initial.q);
	const [ext, setExt] = useState(initial.ext);
	const [tri, setTri] = useState<"nom" | "taille">(initial.tri);
	const [ordre, setOrdre] = useState<"asc" | "desc">(initial.ordre);
	const [minMo, setMinMo] = useState(initial.minMo);
	const [contenu, setContenu] = useState<ContenuDossier | null>(null);
	const [erreur, setErreur] = useState(false);

	useEffect(() => {
		if (!capacites?.vfs) return;
		const ac = new AbortController();
		setErreur(false);
		ecrireUrl({ prefixe, q, ext, tri, ordre, minMo });
		source
			.parcourir(prefixe, {
				q: q || undefined,
				ext: ext || undefined,
				tri,
				ordre,
				tailleMin: bornesEnOctets(minMo),
				signal: ac.signal,
			})
			.then((c) => {
				if (!ac.signal.aborted) setContenu(c);
			})
			.catch(() => {
				if (!ac.signal.aborted) setErreur(true);
			});
		return () => ac.abort();
	}, [source, capacites?.vfs, prefixe, q, ext, tri, ordre, minMo]);

	if (!capacites) return <Note>Chargement…</Note>;
	if (!capacites.vfs) {
		return <Note>L'arborescence est en cours de préparation. Elle s'affichera dès qu'elle sera prête.</Note>;
	}
	if (erreur) {
		return <Note ton="alerte">Ce dossier n'a pas pu être ouvert. Réessayez dans un instant.</Note>;
	}

	const segments = fil(prefixe);
	const elements = contenu ? contenu.dossiers.length + contenu.fichiers.length : 0;
	// Le compte vient du SERVEUR (`total_fichiers`), pas de la longueur du tableau reçu : les
	// deux diffèrent dès qu'une page est tronquée.
	const retenus = contenu?.total ?? contenu?.fichiers.length ?? 0;
	const avantFiltre = contenu?.totalSansFiltre ?? retenus;
	const filtre = Boolean(q || ext || minMo || tri !== "nom" || ordre !== "asc");
	// Le serveur distingue « 0 résultat » de « cette extension n'existe pas ici », et cette
	// nuance est la seule qui aide : sans elle, une faute de frappe se présente comme un dossier
	// vide. On lit donc son drapeau plutôt que d'en déduire un.
	const extIntrouvable = Boolean(contenu?.filtres?.extInconnue);

	return (
		<section>
			<TitreVue appoint={contenu ? accorde(elements, "entrée") : undefined}>
				Explorer
			</TitreVue>

			{/*
			  * La recherche se SOUMET. `form` plutôt qu'un `onChange` : la touche Entrée marche
			  * sans code, et le navigateur annonce le champ comme une recherche.
			  */}
			<form
				onSubmit={(e) => {
					e.preventDefault();
					setQ(saisie.trim());
				}}
				style={{
					display: "flex",
					flexWrap: "wrap",
					gap: "var(--jeu-espace-s)",
					margin: "var(--jeu-espace-m) 0",
				}}
			>
				<input
					type="search"
					value={saisie}
					onChange={(e) => setSaisie(e.target.value)}
					placeholder="Chercher dans ce dossier"
					aria-label="Chercher dans ce dossier"
					style={{ ...champ, flex: "1 1 14rem" }}
				/>
				<input
					type="text"
					value={ext}
					onChange={(e) => setExt(e.target.value.trim().replace(/^\./, ""))}
					placeholder="Extension"
					aria-label="Filtrer par extension"
					style={{ ...champ, flex: "0 0 8rem" }}
				/>
				<button type="submit" style={bouton}>
					Chercher
				</button>
				{filtre ? (
					<button
						type="button"
						onClick={() => {
							setSaisie("");
							setQ("");
							setExt("");
							setTri("nom");
							setOrdre("asc");
							setMinMo("");
						}}
						style={{ ...bouton, fontWeight: 400 }}
					>
						Tout effacer
					</button>
				) : null}
			</form>

			{/*
			  * Trois réglages que `/b` sert déjà : le tri (nom ou taille) et la borne basse de
			  * taille. Ils sont ici parce que la mesure du 2026-09-06 a montré que le serveur
			  * servait 41 filtres et que cette page en utilisait deux.
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
				<label style={etiquette}>
					Trier par
					<select
						value={`${tri}-${ordre}`}
						onChange={(e) => {
							const [t, o] = e.target.value.split("-");
							setTri(t === "taille" ? "taille" : "nom");
							setOrdre(o === "desc" ? "desc" : "asc");
						}}
						style={champ}
					>
						<option value="nom-asc">Nom (A→Z)</option>
						<option value="nom-desc">Nom (Z→A)</option>
						<option value="taille-asc">Taille (petits d'abord)</option>
						<option value="taille-desc">Taille (gros d'abord)</option>
					</select>
				</label>
				<label style={etiquette}>
					Au moins
					<input
						type="text"
						inputMode="decimal"
						value={minMo}
						onChange={(e) => setMinMo(e.target.value)}
						placeholder="0"
						aria-label="Taille minimale en mégaoctets"
						style={{ ...champ, width: "5rem" }}
					/>
					Mo
				</label>
			</div>

			{contenu ? (
				<p style={{ margin: "0 0 var(--jeu-espace-m)", fontSize: "0.9rem", opacity: 0.8 }}>
					{filtre
						? `${accorde(retenus, "fichier")} sur ${avantFiltre}`
						: accorde(retenus, "fichier")}
					{extIntrouvable ? " — aucun fichier de ce type dans ce dossier" : ""}
				</p>
			) : null}

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
					{elements === 0 ? (
						<Note>
							{filtre
								? "Rien ne correspond ici. Effacez le filtre pour revoir le dossier."
								: "Ce dossier est vide."}
						</Note>
					) : null}
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

const etiquette: React.CSSProperties = {
	display: "inline-flex",
	alignItems: "center",
	gap: "var(--jeu-espace-xs)",
	fontWeight: 700,
};

const champ: React.CSSProperties = {
	padding: "var(--jeu-espace-xs) var(--jeu-espace-s)",
	border: "1px solid var(--jeu-tuile-bord)",
	borderRadius: "var(--jeu-rayon-s, 4px)",
	font: "inherit",
	minWidth: 0,
};

const bouton: React.CSSProperties = {
	padding: "var(--jeu-espace-xs) var(--jeu-espace-m)",
	border: "1px solid var(--jeu-tuile-bord)",
	borderRadius: "var(--jeu-rayon-s, 4px)",
	background: "var(--jeu-tuile-haut, transparent)",
	color: "inherit",
	font: "inherit",
	fontWeight: 700,
	cursor: "pointer",
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
