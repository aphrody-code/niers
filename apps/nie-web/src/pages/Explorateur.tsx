/**
 * L'explorateur — **la seule page**, décidé par l'utilisateur le 2026-09-06.
 *
 * Trois écrans séparés (parcourir, chercher, données) faisaient trois destinations pour un
 * seul geste : *où est ce fichier, et que sait-on de lui*. Ils sont réunis en une page à trois
 * zones, et la troisième est ce qui manquait — la donnée n'est plus une page, c'est le
 * **contexte** de ce qu'on regarde.
 *
 * ```
 * ┌──────────────────────────────────────────────┬────────────────────┐
 * │ recherche · extension · tri · taille         │                    │
 * ├──────────────────────────────────────────────┤   panneau de       │
 * │ fil d'Ariane                                 │   données          │
 * │ dossiers…                                    │                    │
 * │ fichiers…                                    │   dossier courant  │
 * │                                              │   OU asset choisi  │
 * └──────────────────────────────────────────────┴────────────────────┘
 * ```
 *
 * ## Deux portées pour une seule barre de recherche
 *
 * `/b` filtre le dossier **direct** ; `/api/v1/recherche` traverse les 255 308 entrées et
 * accepte `prefixe=`. Une case bascule de l'un à l'autre **sans changer d'écran** : c'est la
 * même question posée plus ou moins loin. En mode « partout », le dossier courant reste le
 * préfixe — sinon le lecteur perdrait sa place en cherchant.
 *
 * ## Ce que le panneau n'invente pas
 *
 * Le format d'un asset vient de `/api/v1/formats/decode`, qui lit le **magic** ; sa taille et
 * son pack viennent de l'index. Rien n'est déduit de l'extension — c'est précisément la
 * confusion qui avait fait classer `bloqué` 3 600 fichiers que le dépôt savait lire.
 */
import type { ContenuDossier, EntreeVfs } from "@niers/asset-source";
import { useAssetSource, useCapacites } from "@niers/inacord-ui";
import { useEffect, useMemo, useState } from "react";
import { PanneauDonnees } from "./Donnees";
import { accorde, Note, TitreVue } from "./Ecran";

/** Formate une taille en octets. */
function taille(octets: number): string {
	if (octets < 1024) return `${octets} o`;
	if (octets < 1024 * 1024) return `${(octets / 1024).toFixed(1)} ko`;
	return `${(octets / (1024 * 1024)).toFixed(1)} Mo`;
}

/**
 * Le code d'un asset : sa feuille, sans extension **ni numéro de version**.
 *
 * `chara_param_1.03.66.00.cfg.bin` donne `chara_param`. C'est ce qui rapproche un fichier d'une
 * ligne de table : le jeu nomme les deux par le même code, et la version n'existe que sur le
 * disque. Découper au premier point suffit — les noms du jeu ne portent pas de point ailleurs.
 */
function codeDe(chemin: string): string {
	const feuille = chemin.split("/").pop() ?? chemin;
	const sansVersion = feuille.replace(/_\d+(\.\d+)*(?=\.)/, "");
	const point = sansVersion.indexOf(".");
	return point > 0 ? sansVersion.slice(0, point) : sansVersion;
}

/** L'extension d'un chemin, lue sur la FEUILLE : `data/x.y/z` n'en a pas. */
function extensionDe(chemin: string): string | null {
	const feuille = chemin.split("/").pop() ?? chemin;
	const point = feuille.lastIndexOf(".");
	return point > 0 ? feuille.slice(point + 1).toLowerCase() : null;
}

/**
 * Découpe un préfixe en segments cliquables, chacun portant le chemin CUMULÉ.
 *
 * Un chemin VFS reconstruit est presque toujours faux — les fichiers du jeu portent un numéro
 * de version — donc on ne recompose jamais une adresse à partir d'un nom.
 */
function fil(prefixe: string): { nom: string; chemin: string }[] {
	const segments = prefixe.split("/").filter(Boolean);
	return segments.map((nom, i) => ({ nom, chemin: segments.slice(0, i + 1).join("/") }));
}

type Etat = {
	prefixe: string;
	q: string;
	ext: string;
	tri: "nom" | "taille";
	ordre: "asc" | "desc";
	minMo: string;
	/** Borne haute de taille, en Mo saisis. */
	maxMo: string;
	/** Motif glob du jeu — n'a de sens qu'en portée « partout ». */
	glob: string;
	/** Pack CPK d'origine — idem : l'index le connaît, un dossier non. */
	cpk: string;
	/** `true` : la recherche traverse tout le VFS sous le dossier courant. */
	partout: boolean;
	/** Chemin de l'asset dont le panneau parle. */
	choisi: string;
	/** Liste ou grille — repris de l'explorateur d'Inacord (`ExplorerView.tsx:174`). */
	vue: "liste" | "grille";
	/** Côté d'une vignette en pixels, comme `gridSize` côté desktop (`:178`). */
	cote: number;
};

const VIDE: Etat = {
	prefixe: "",
	q: "",
	ext: "",
	tri: "nom",
	ordre: "asc",
	minMo: "",
	maxMo: "",
	glob: "",
	cpk: "",
	partout: false,
	choisi: "",
	vue: "liste",
	cote: 96,
};

/**
 * Combien de fichiers sont rendus d'un coup, et par quel pas on augmente.
 *
 * Repris tel quel d'`ExplorerView.tsx:75` (`PAGE_FICHIERS = 300`). Un dossier du jeu monte à
 * 40 000 entrées : les rendre toutes fige l'onglet plusieurs secondes, et personne ne lit la
 * 3 000ᵉ. Le serveur, lui, a déjà rendu la page — c'est le DOM qu'on rationne, pas la requête.
 */
const PAR_PALIER = 300;

/** Les trois tailles de vignette, comme le curseur de l'explorateur desktop. */
const COTES = [64, 96, 144] as const;

/**
 * Au-delà de ce poids, la grille affiche un pictogramme plutôt que la texture.
 *
 * Ce n'est pas une préférence esthétique : le proxy rend la texture **pleine**, il n'existe
 * aucune route de vignette en amont, et aucun paramètre de taille n'y est honoré (mesuré le
 * 2026-09-06 : `?w=`, `?size=`, `?cote=` rendent tous 12 176 145 o). Cinquante tuiles feraient
 * 600 Mo. Le desktop n'a pas ce problème — il décode et réduit en natif (`textureThumbB64`).
 */
const POIDS_VIGNETTE = 512 * 1024;

const MO = 1024 * 1024;

function etatDeLUrl(): Etat {
	const p = new URLSearchParams(window.location.search);
	return {
		...VIDE,
		prefixe: p.get("d") ?? "",
		q: p.get("q") ?? "",
		ext: p.get("ext") ?? "",
		tri: p.get("tri") === "taille" ? "taille" : "nom",
		ordre: p.get("ordre") === "desc" ? "desc" : "asc",
		minMo: p.get("min_mo") ?? "",
		maxMo: p.get("max_mo") ?? "",
		glob: p.get("glob") ?? "",
		cpk: p.get("cpk") ?? "",
		partout: p.get("partout") === "1",
		choisi: p.get("a") ?? "",
		vue: p.get("vue") === "grille" ? "grille" : "liste",
		// Une valeur hors de la liste servie retombe sur le défaut : un `cote=100000` tapé dans
		// la barre d'adresse ne doit pas devenir une grille d'une seule tuile.
		cote: COTES.includes(Number(p.get("cote")) as (typeof COTES)[number])
			? Number(p.get("cote"))
			: 96,
	};
}

/**
 * Écrit l'état dans l'URL sans empiler d'historique.
 *
 * `replaceState` : filtrer n'est pas naviguer, et vingt frappes ne doivent pas coûter vingt
 * « précédent ». Un DÉFAUT ne s'écrit pas — deux adresses pour le même écran cassent le partage
 * autant que l'absence d'état.
 */
function ecrireUrl(e: Etat) {
	const url = new URL(window.location.href);
	for (const [cle, valeur] of [
		["d", e.prefixe],
		["q", e.q],
		["ext", e.ext],
		["tri", e.tri === "nom" ? "" : e.tri],
		["ordre", e.ordre === "asc" ? "" : e.ordre],
		["min_mo", e.minMo],
		["max_mo", e.maxMo],
		["glob", e.glob],
		["cpk", e.cpk],
		["partout", e.partout ? "1" : ""],
		["a", e.choisi],
		["vue", e.vue === "liste" ? "" : e.vue],
		["cote", e.cote === 96 ? "" : String(e.cote)],
	] as const) {
		if (valeur) url.searchParams.set(cle, valeur);
		else url.searchParams.delete(cle);
	}
	window.history.replaceState(window.history.state, "", url);
}

/**
 * Convertit des mégaoctets saisis en octets.
 *
 * `undefined` pour une saisie vide **ou illisible** : `NaN` ferait un `400` sur une frappe en
 * cours. `0` reste une borne légitime — il existe des fichiers de zéro octet — donc le test
 * porte sur la lisibilité, jamais sur la vérité de la valeur.
 */
function octetsDe(mo: string): number | undefined {
	const n = Number(mo.replace(",", "."));
	return mo.trim() && Number.isFinite(n) && n >= 0 ? Math.round(n * MO) : undefined;
}

/** Ce que le serveur dit avoir décodé — jamais ce que l'extension laisse croire. */
interface Decodage {
	format?: string | null;
	octets?: number | null;
	chemin?: string;
}

export function Explorateur() {
	const source = useAssetSource();
	const capacites = useCapacites();
	const initial = useMemo(etatDeLUrl, []);
	const [etat, setEtat] = useState<Etat>(initial);
	// `saisie` suit le champ, `etat.q` ce qui a été envoyé : sans ce décalage, chaque frappe
	// interrogerait 255 308 chemins pour un résultat que personne ne lit.
	const [saisie, setSaisie] = useState(initial.q);
	const [contenu, setContenu] = useState<ContenuDossier | null>(null);
	const [globaux, setGlobaux] = useState<{ fichiers: EntreeVfs[]; total: number } | null>(null);
	const [erreur, setErreur] = useState(false);
	const { prefixe, q, ext, tri, ordre, minMo, maxMo, glob, cpk, partout, choisi, vue, cote } =
		etat;
	const modifier = (p: Partial<Etat>) => setEtat((v) => ({ ...v, ...p }));
	// Combien de fichiers sont rendus. Remis à zéro à chaque changement de contenu — garder
	// « 900 visibles » en arrivant dans un dossier de 12 en afficherait 12 et mentirait sur le
	// bouton « en voir plus ».
	const [visibles, setVisibles] = useState(PAR_PALIER);
	// Le curseur du clavier prime sur la sélection : les flèches le déplacent, et il peut être
	// posé sur un DOSSIER, que la sélection n'accepte pas (`ExplorerView.tsx:658`).
	const [curseur, setCurseur] = useState<string | null>(null);

	useEffect(() => {
		if (!capacites?.vfs) return;
		const ac = new AbortController();
		setErreur(false);
		ecrireUrl(etat);

		if (partout && (q.trim() || glob.trim() || cpk.trim())) {
			// Portée « partout » : l'index entier, borné au dossier courant. `prefixe=` existe
			// pour ça — sans lui, chercher ferait perdre sa place au lecteur.
			const p = new URLSearchParams({ per_page: "200", tri, ordre });
			if (q.trim()) p.set("q", q.trim());
			if (prefixe) p.set("prefixe", prefixe);
			if (ext.trim()) p.set("ext", ext.trim().replace(/^\./, ""));
			// Glob et pack n'existent QUE sur l'index : un dossier ne connaît ni motif de
			// chemin ni CPK d'origine. Les envoyer à `/b` serait les faire avaler en silence.
			if (glob.trim()) p.set("glob", glob.trim());
			if (cpk.trim()) p.set("cpk", cpk.trim());
			const min = octetsDe(minMo);
			if (min !== undefined) p.set("taille_min", String(min));
			const max = octetsDe(maxMo);
			if (max !== undefined) p.set("taille_max", String(max));
			fetch(`/api/v1/recherche?${p}`, { signal: ac.signal, headers: { accept: "application/json" } })
				.then((r) => (r.ok ? r.json() : Promise.reject(new Error(String(r.status)))))
				.then((r: { fichiers: EntreeVfs[]; total: number }) => {
					if (!ac.signal.aborted) {
						setGlobaux({ fichiers: r.fichiers, total: r.total });
						setContenu(null);
					}
				})
				.catch(() => {
					if (!ac.signal.aborted) setErreur(true);
				});
			return () => ac.abort();
		}

		setGlobaux(null);
		source
			.parcourir(prefixe, {
				q: q.trim() || undefined,
				ext: ext.trim() || undefined,
				tri,
				ordre,
				tailleMin: octetsDe(minMo),
				tailleMax: octetsDe(maxMo),
				// Le plafond du serveur. Au-delà, c'est le palier local qui rationne le DOM —
				// mais il faut d'abord AVOIR les entrées : à 50 par défaut, un dossier de 373
				// se présentait comme un dossier de 50, avec le bon total imprimé à côté.
				parPage: 200,
				signal: ac.signal,
			})
			.then((c) => {
				if (!ac.signal.aborted) setContenu(c);
			})
			.catch(() => {
				if (!ac.signal.aborted) setErreur(true);
			});
		return () => ac.abort();
	}, [source, capacites?.vfs, etat, prefixe, q, ext, tri, ordre, minMo, maxMo, glob, cpk, partout]);

	// Le palier repart à zéro quand le contenu change : garder « 900 visibles » en arrivant dans
	// un dossier de 12 n'en montrerait que 12, et le bouton « en voir plus » aurait disparu sans
	// que rien ne le dise.
	useEffect(() => {
		setVisibles(PAR_PALIER);
		setCurseur(null);
	}, [prefixe, q, ext, partout, glob, cpk]);

	if (!capacites) return <Note>Chargement…</Note>;
	if (!capacites.vfs) {
		return <Note>L'arborescence est en cours de préparation. Elle s'affichera dès qu'elle sera prête.</Note>;
	}

	const fichiers = globaux?.fichiers ?? contenu?.fichiers ?? [];
	const dossiers = globaux ? [] : (contenu?.dossiers ?? []);
	const retenus = globaux?.total ?? contenu?.total ?? fichiers.length;
	const avantFiltre = contenu?.totalSansFiltre ?? retenus;
	const filtreActif = Boolean(
		q || ext || minMo || maxMo || glob || cpk || tri !== "nom" || ordre !== "asc",
	);
	const extIntrouvable = Boolean(contenu?.filtres?.extInconnue);
	const segments = fil(prefixe);
	// La liste à plat que le clavier parcourt : dossiers d'abord, comme à l'écran. Sans elle,
	// les flèches sauteraient les dossiers ou les mettraient en fin de course.
	const aPlat: { chemin: string; dossier: boolean }[] = [
		...dossiers.map((d) => ({ chemin: d, dossier: true })),
		...fichiers.slice(0, visibles).map((f) => ({ chemin: f.chemin, dossier: false })),
	];

	/** Flèches, Entrée, Retour arrière — le jeu de touches de l'explorateur desktop. */
	function auClavier(e: React.KeyboardEvent) {
		if (aPlat.length === 0) return;
		const i = aPlat.findIndex((x) => x.chemin === (curseur ?? choisi));
		if (e.key === "ArrowDown" || e.key === "ArrowUp") {
			e.preventDefault();
			// Sans curseur ni sélection, la première flèche prend la première entrée quel que
			// soit son sens : il faut bien entrer dans la liste par un bout.
			const suivant =
				i === -1
					? 0
					: e.key === "ArrowDown"
						? Math.min(i + 1, aPlat.length - 1)
						: Math.max(i - 1, 0);
			const cible = aPlat[suivant];
			if (!cible) return;
			setCurseur(cible.chemin);
			if (!cible.dossier) modifier({ choisi: cible.chemin });
		} else if (e.key === "Enter" && i >= 0) {
			const cible = aPlat[i];
			if (cible?.dossier) modifier({ prefixe: cible.chemin, choisi: "" });
			else if (cible) modifier({ choisi: cible.chemin });
		} else if (e.key === "Backspace") {
			e.preventDefault();
			modifier({ prefixe: segments.slice(0, -1).map((s) => s.nom).join("/"), choisi: "" });
		}
	}

	return (
		<section>
			<TitreVue appoint={contenu || globaux ? accorde(retenus, "fichier") : undefined}>
				Explorer
			</TitreVue>

			{/* ── La barre : une recherche, deux portées, trois filtres ───────────────────── */}
			<form
				onSubmit={(e) => {
					e.preventDefault();
					modifier({ q: saisie.trim(), choisi: "" });
				}}
				style={{
					display: "flex",
					flexWrap: "wrap",
					alignItems: "center",
					gap: "var(--jeu-espace-s)",
					margin: "var(--jeu-espace-m) 0",
				}}
			>
				<input
					type="search"
					value={saisie}
					onChange={(e) => setSaisie(e.target.value)}
					placeholder={partout ? "Chercher dans tout le VFS" : "Chercher dans ce dossier"}
					aria-label="Chercher"
					style={{ ...CHAMP, flex: "1 1 16rem" }}
				/>
				<button type="submit" style={{ ...CHAMP, cursor: "pointer", fontWeight: 700 }}>
					Chercher
				</button>
				<label style={{ ...ETIQUETTE, flexDirection: "row" }}>
					<input
						type="checkbox"
						checked={partout}
						onChange={(e) => modifier({ partout: e.target.checked, choisi: "" })}
					/>
					Partout, sous ce dossier
				</label>
				<input
					type="text"
					value={ext}
					onChange={(e) => modifier({ ext: e.target.value.trim().replace(/^\./, ""), choisi: "" })}
					placeholder="Extension"
					aria-label="Filtrer par extension"
					style={{ ...CHAMP, width: "8rem" }}
				/>
				<select
					value={`${tri}-${ordre}`}
					onChange={(e) => {
						const [t, o] = e.target.value.split("-");
						modifier({ tri: t === "taille" ? "taille" : "nom", ordre: o === "desc" ? "desc" : "asc" });
					}}
					aria-label="Trier"
					style={CHAMP}
				>
					<option value="nom-asc">Nom (A→Z)</option>
					<option value="nom-desc">Nom (Z→A)</option>
					<option value="taille-asc">Taille (petits d'abord)</option>
					<option value="taille-desc">Taille (gros d'abord)</option>
				</select>
				<label style={{ ...ETIQUETTE, flexDirection: "row" }}>
					Au moins
					<input
						type="text"
						inputMode="decimal"
						value={minMo}
						onChange={(e) => modifier({ minMo: e.target.value })}
						placeholder="0"
						aria-label="Taille minimale en mégaoctets"
						style={{ ...CHAMP, width: "4.5rem" }}
					/>
					Mo
				</label>
				{filtreActif ? (
					<button
						type="button"
						onClick={() => {
							setSaisie("");
							modifier({
								q: "",
								ext: "",
								minMo: "",
								maxMo: "",
								glob: "",
								cpk: "",
								tri: "nom",
								ordre: "asc",
								partout: false,
							});
						}}
						style={{ ...CHAMP, cursor: "pointer" }}
					>
						Tout effacer
					</button>
				) : null}
			</form>

			{/*
			  * Les trois filtres qui n'ont de sens QUE sur l'index : un motif de chemin, un pack
			  * d'origine, une borne haute. Repliés, parce qu'ils répondent à une question plus
			  * rare — et parce qu'une barre de neuf champs ne se lit plus.
			  */}
			<details open={Boolean(glob || cpk || maxMo)} style={{ margin: "0 0 var(--jeu-espace-m)" }}>
				<summary style={{ cursor: "pointer", fontWeight: 700, fontSize: "0.9rem" }}>
					Filtres de l'index
				</summary>
				<div
					style={{
						display: "flex",
						flexWrap: "wrap",
						gap: "var(--jeu-espace-m)",
						margin: "var(--jeu-espace-s) 0 0",
					}}
				>
					<label style={ETIQUETTE}>
						<span style={{ fontSize: "0.8rem", opacity: 0.75 }}>Motif</span>
						<input
							type="text"
							value={glob}
							onChange={(e) => modifier({ glob: e.target.value, partout: true, choisi: "" })}
							placeholder="data/**/*.g4tx,!**/movie/**"
							aria-label="Motif glob"
							style={{ ...CHAMP, width: "18rem" }}
						/>
					</label>
					<label style={ETIQUETTE}>
						<span style={{ fontSize: "0.8rem", opacity: 0.75 }}>Pack</span>
						<input
							type="text"
							value={cpk}
							onChange={(e) => modifier({ cpk: e.target.value, partout: true, choisi: "" })}
							placeholder="nom du .cpk"
							aria-label="Pack CPK d'origine"
							style={{ ...CHAMP, width: "14rem" }}
						/>
					</label>
					<label style={ETIQUETTE}>
						<span style={{ fontSize: "0.8rem", opacity: 0.75 }}>Au plus (Mo)</span>
						<input
							type="text"
							inputMode="decimal"
							value={maxMo}
							onChange={(e) => modifier({ maxMo: e.target.value, choisi: "" })}
							placeholder="∞"
							aria-label="Taille maximale en mégaoctets"
							style={{ ...CHAMP, width: "6rem" }}
						/>
					</label>
				</div>
			</details>

			{/*
			  * Vue et taille de vignette — les deux dernières lignes que la matrice des filtres
			  * classait « côté client », et qu'Inacord avait depuis toujours
			  * (`ExplorerView.tsx:174` et `:178`).
			  */}
			<div
				style={{
					display: "flex",
					alignItems: "center",
					gap: "var(--jeu-espace-m)",
					margin: "0 0 var(--jeu-espace-s)",
					fontSize: "0.9rem",
				}}
			>
				<div role="group" aria-label="Affichage" style={{ display: "flex", gap: 2 }}>
					{(["liste", "grille"] as const).map((v) => (
						<button
							key={v}
							type="button"
							aria-pressed={vue === v}
							onClick={() => modifier({ vue: v })}
							style={{
								...CHAMP,
								cursor: "pointer",
								fontWeight: vue === v ? 800 : 600,
								background: vue === v ? "var(--jeu-tuile-bord)" : "#fff",
							}}
						>
							{v === "liste" ? "Liste" : "Grille"}
						</button>
					))}
				</div>
				{vue === "grille" ? (
					<label style={{ ...ETIQUETTE, flexDirection: "row" }}>
						Vignettes
						<select
							value={String(cote)}
							onChange={(e) => modifier({ cote: Number(e.target.value) })}
							style={CHAMP}
						>
							{COTES.map((c) => (
								<option key={c} value={c}>
									{c} px
								</option>
							))}
						</select>
					</label>
				) : null}
			</div>

			<nav aria-label="Chemin" style={{ margin: "0 0 var(--jeu-espace-s)", fontSize: "0.9rem" }}>
				<button type="button" onClick={() => modifier({ prefixe: "", choisi: "" })} style={LIEN}>
					Racine
				</button>
				{segments.map((s) => (
					<span key={s.chemin}>
						<span aria-hidden="true" style={{ opacity: 0.5 }}> / </span>
						<button type="button" onClick={() => modifier({ prefixe: s.chemin, choisi: "" })} style={LIEN}>
							{s.nom}
						</button>
					</span>
				))}
			</nav>

			<p style={{ margin: "0 0 var(--jeu-espace-m)", fontSize: "0.9rem", opacity: 0.8 }}>
				{globaux
					? `${accorde(retenus, "fichier")} dans tout le sous-arbre`
					: filtreActif
						? `${accorde(retenus, "fichier")} sur ${avantFiltre}`
						: accorde(retenus, "fichier")}
				{extIntrouvable ? " — aucun fichier de ce type dans ce dossier" : ""}
			</p>

			{erreur ? (
				<Note ton="alerte">Ce dossier n'a pas pu être ouvert. Réessayez dans un instant.</Note>
			) : null}

			{/* ── Les deux colonnes ──────────────────────────────────────────────────────── */}
			<div style={{ display: "flex", flexWrap: "wrap", gap: "var(--jeu-espace-l)", alignItems: "flex-start" }}>
				<div style={{ flex: "1 1 22rem", minWidth: 0 }}>
					{!contenu && !globaux ? (
						<Note>Chargement…</Note>
					) : dossiers.length + fichiers.length === 0 ? (
						<Note>
							{filtreActif
								? "Rien ne correspond ici. Effacez le filtre, ou cherchez partout."
								: "Ce dossier est vide."}
						</Note>
					) : (
						/*
						  * `tabIndex` + `onKeyDown` sur le conteneur, comme côté desktop : la liste
						  * est UN arrêt de tabulation qu'on parcourt aux flèches, et non 300
						  * arrêts qu'il faudrait franchir un par un pour atteindre le panneau.
						  */
						// biome-ignore lint/a11y/noNoninteractiveTabindex: la liste EST le contrôle.
						<div tabIndex={0} onKeyDown={auClavier} style={{ outline: "none" }}>
							<ul
								style={
									vue === "grille"
										? {
												listStyle: "none",
												margin: 0,
												padding: 0,
												display: "grid",
												gridTemplateColumns: `repeat(auto-fill, minmax(${cote}px, 1fr))`,
												gap: "var(--jeu-espace-s)",
											}
										: { listStyle: "none", margin: 0, padding: 0 }
								}
							>
								{/*
								  * `dossiers` porte des chemins COMPLETS (`data/common`), pas des noms
								  * relatifs : les concaténer au préfixe courant produirait
								  * `data/data/common`, un 404 que rien n'expliquerait. On navigue donc
								  * vers la valeur telle quelle, et on n'affiche que son dernier segment.
								  */}
								{dossiers.map((d) => (
									<li key={d}>
										<button
											type="button"
											onClick={() => modifier({ prefixe: d, choisi: "" })}
											style={{
												...(vue === "grille" ? TUILE(cote) : LIGNE),
												fontWeight: 800,
												outline: curseur === d ? "2px solid var(--jeu-nuit-profonde)" : "none",
											}}
										>
											<IconeDossier /> {d.split("/").filter(Boolean).at(-1) ?? d}
										</button>
									</li>
								))}
								{fichiers.slice(0, visibles).map((f) => (
									<li key={f.chemin}>
										{/*
										  * Un clic SÉLECTIONNE au lieu de télécharger : c'est ce qui fait
										  * du panneau de droite un contexte plutôt qu'une page de plus.
										  * Le téléchargement reste un geste explicite, dans le panneau.
										  */}
										<button
											type="button"
											aria-pressed={choisi === f.chemin}
											onClick={() => {
												setCurseur(f.chemin);
												modifier({ choisi: choisi === f.chemin ? "" : f.chemin });
											}}
											style={{
												...(vue === "grille" ? TUILE(cote) : LIGNE),
												background: choisi === f.chemin ? "var(--jeu-tuile-bord)" : "none",
												outline:
													curseur === f.chemin ? "2px solid var(--jeu-nuit-profonde)" : "none",
											}}
										>
											{vue === "grille" ? (
											<Vignette chemin={f.chemin} cote={cote} poids={f.taille} />
										) : null}
											<span
												style={{
													flex: vue === "grille" ? "none" : 1,
													minWidth: 0,
													width: vue === "grille" ? "100%" : undefined,
													overflow: "hidden",
													textOverflow: "ellipsis",
													whiteSpace: "nowrap",
													fontSize: vue === "grille" ? "0.75rem" : undefined,
												}}
											>
												{globaux && vue === "liste" ? f.chemin : f.nom}
											</span>
											<span style={{ color: "var(--jeu-tuile-bas)", fontSize: "0.75rem" }}>
												{taille(f.taille)}
											</span>
										</button>
									</li>
								))}
							</ul>
							{/*
							  * Le palier, repris d'`ExplorerView.tsx:345`. Un dossier du jeu monte à
							  * 40 000 entrées : les rendre toutes fige l'onglet, et personne ne lit
							  * la 3 000ᵉ. C'est le DOM qu'on rationne — la page est déjà reçue.
							  */}
							{fichiers.length > visibles ? (
								<button
									type="button"
									onClick={() => setVisibles((n) => n + PAR_PALIER)}
									style={{ ...CHAMP, cursor: "pointer", margin: "var(--jeu-espace-m) 0" }}
								>
									En voir {Math.min(PAR_PALIER, fichiers.length - visibles)} de plus (
									{(fichiers.length - visibles).toLocaleString("fr")} restants)
								</button>
							) : retenus > fichiers.length ? (
								/*
								  * Le serveur plafonne à 200 par requête : au-delà, ce n'est plus le
								  * DOM qu'il faut rationner mais une page de plus qu'il faut demander.
								  * Le dire est la moitié qui manquait — un « 373 fichiers » au-dessus
								  * d'une liste de 200 laisse croire à une liste tronquée sans raison.
								  */
								<p style={{ fontSize: "0.85rem", opacity: 0.75 }}>
									{fichiers.length.toLocaleString("fr")} des{" "}
									{retenus.toLocaleString("fr")} affichés — affinez le filtre pour voir
									les suivants.
								</p>
							) : null}
						</div>
					)}
				</div>

				<aside
					aria-label="Données"
					style={{
						flex: "0 1 22rem",
						minWidth: "16rem",
						position: "sticky",
						top: "var(--jeu-espace-m)",
						padding: "var(--jeu-espace-m)",
						background: "#fff",
						border: "2px solid var(--jeu-tuile-bord)",
						borderRadius: "var(--jeu-rayon)",
					}}
				>
					{choisi ? (
						<PanneauAsset
							chemin={choisi}
							fichier={fichiers.find((f) => f.chemin === choisi) ?? null}
						/>
					) : (
						<PanneauDossier prefixe={prefixe} dossiers={dossiers} fichiers={fichiers} total={retenus} />
					)}

					{/*
					  * Le second volet du panneau : les 224 tables des deux gisements, avec leurs
					  * routes API inchangées. Replié par défaut — il répond à une question qu'on
					  * ne pose pas à chaque clic — et pré-rempli par l'asset sélectionné : un
					  * fichier du jeu et la ligne qui le décrit portent souvent le même code.
					  */}
					<details style={{ marginTop: "var(--jeu-espace-m)" }}>
						<summary style={{ cursor: "pointer", fontWeight: 800 }}>Tables de données</summary>
						<PanneauDonnees contexte={choisi ? codeDe(choisi) : undefined} />
					</details>
				</aside>
			</div>
		</section>
	);
}

/**
 * Le panneau quand rien n'est sélectionné : ce que dit le dossier courant.
 *
 * La répartition est calculée sur ce qui est **affiché**, et le dit. La déduire du total du
 * serveur serait présenter comme une mesure du dossier ce qui n'est qu'une mesure de la page.
 */
function PanneauDossier({
	prefixe,
	dossiers,
	fichiers,
	total,
}: {
	prefixe: string;
	dossiers: string[];
	fichiers: EntreeVfs[];
	total: number;
}) {
	const parExtension = useMemo(() => {
		const compte = new Map<string, { n: number; poids: number }>();
		for (const f of fichiers) {
			const e = extensionDe(f.chemin) ?? "(sans extension)";
			const c = compte.get(e) ?? { n: 0, poids: 0 };
			compte.set(e, { n: c.n + 1, poids: c.poids + f.taille });
		}
		return [...compte.entries()].sort((a, b) => b[1].n - a[1].n).slice(0, 12);
	}, [fichiers]);
	const poids = fichiers.reduce((s, f) => s + f.taille, 0);

	return (
		<>
			<h2 style={TITRE_PANNEAU}>{prefixe || "Racine du VFS"}</h2>
			<dl style={{ margin: 0 }}>
				<Ligne cle="Sous-dossiers" valeur={String(dossiers.length)} />
				<Ligne cle="Fichiers retenus" valeur={total.toLocaleString("fr")} />
				<Ligne cle="Poids affiché" valeur={taille(poids)} />
			</dl>
			{parExtension.length > 0 ? (
				<>
					<h3 style={{ ...TITRE_PANNEAU, fontSize: "0.9rem", marginTop: "var(--jeu-espace-m)" }}>
						Formats affichés
					</h3>
					<ul style={{ listStyle: "none", margin: 0, padding: 0, fontSize: "0.85rem" }}>
						{parExtension.map(([e, c]) => (
							<li key={e} style={{ display: "flex", justifyContent: "space-between", gap: "var(--jeu-espace-s)" }}>
								<span>.{e}</span>
								<span style={{ opacity: 0.7 }}>
									{c.n} · {taille(c.poids)}
								</span>
							</li>
						))}
					</ul>
				</>
			) : (
				<p style={{ fontSize: "0.85rem", opacity: 0.75 }}>
					Choisissez un fichier pour voir ce que le serveur en sait.
				</p>
			)}
		</>
	);
}

/**
 * Le panneau quand un asset est choisi.
 *
 * Le format vient de `/api/v1/formats/decode`, qui lit le **magic** du fichier. Le déduire de
 * l'extension est exactement la confusion qui avait fait classer « bloqués » 3 600 fichiers que
 * ce dépôt savait lire.
 */
function PanneauAsset({ chemin, fichier }: { chemin: string; fichier: EntreeVfs | null }) {
	const source = useAssetSource();
	const [decodage, setDecodage] = useState<Decodage | null>(null);
	const [refus, setRefus] = useState<string | null>(null);
	const ext = extensionDe(chemin);
	const estTexture = ext === "g4tx" || ext === "dds" || ext === "png";

	useEffect(() => {
		const ac = new AbortController();
		setDecodage(null);
		setRefus(null);
		fetch(`/api/v1/formats/decode/${chemin}`, {
			signal: ac.signal,
			headers: { accept: "application/json" },
		})
			.then((r) => (r.ok ? r.json() : Promise.reject(new Error(String(r.status)))))
			.then((d: Decodage) => {
				if (!ac.signal.aborted) setDecodage(d);
			})
			.catch(() => {
				// Un format que le serveur ne sait pas lire n'est pas une panne : c'est une
				// information, et elle se dit.
				if (!ac.signal.aborted) setRefus("Ce format n'est pas encore décodé.");
			});
		return () => ac.abort();
	}, [chemin]);

	return (
		<>
			<h2 style={{ ...TITRE_PANNEAU, wordBreak: "break-all" }}>
				{chemin.split("/").pop()}
			</h2>
			{estTexture ? (
				<img
					src={source.urlTexture?.(chemin) ?? ""}
					alt=""
					loading="lazy"
					style={{
						width: "100%",
						height: "auto",
						margin: "0 0 var(--jeu-espace-s)",
						background: "var(--jeu-tuile-bord)",
						borderRadius: "var(--jeu-rayon)",
					}}
				/>
			) : null}
			<dl style={{ margin: 0 }}>
				<Ligne cle="Chemin" valeur={chemin} coupe />
				{fichier ? <Ligne cle="Taille" valeur={taille(fichier.taille)} /> : null}
				{fichier?.cpk ? <Ligne cle="Pack" valeur={fichier.cpk} coupe /> : null}
				<Ligne
					cle="Format lu"
					valeur={decodage?.format ?? (refus ? "—" : "…")}
				/>
				{decodage?.octets ? <Ligne cle="Octets lus" valeur={decodage.octets.toLocaleString("fr")} /> : null}
			</dl>
			{refus ? <p style={{ fontSize: "0.85rem", opacity: 0.75 }}>{refus}</p> : null}
			<p style={{ margin: "var(--jeu-espace-m) 0 0" }}>
				{/* Le chemin VERBATIM sert d'adresse : extension du jeu conservée. */}
				<a href={`/f/${chemin}`} style={{ fontWeight: 700 }}>
					Télécharger
				</a>
			</p>
		</>
	);
}

/** Une ligne de définition du panneau. */
function Ligne({ cle, valeur, coupe }: { cle: string; valeur: string; coupe?: boolean }) {
	return (
		<div style={{ display: "flex", gap: "var(--jeu-espace-s)", fontSize: "0.85rem", padding: "2px 0" }}>
			<dt style={{ opacity: 0.7, flex: "0 0 7rem" }}>{cle}</dt>
			<dd
				style={{
					margin: 0,
					flex: 1,
					minWidth: 0,
					wordBreak: coupe ? "break-all" : "normal",
				}}
			>
				{valeur}
			</dd>
		</div>
	);
}

/**
 * La vignette d'un fichier, en vue grille.
 *
 * Port de `FileThumbnail` (`ExplorerView.tsx:89`), avec la même règle : un format que l'hôte ne
 * sait pas rendre affiche un **pictogramme**, jamais une image cassée. Côté desktop la texture
 * est décodée en natif ; ici c'est `urlTexture`, servi par le proxy `/assets`.
 *
 * `loading="lazy"` fait le travail que `useThumbnail` faisait avec un `IntersectionObserver` :
 * une grille de 300 tuiles ne demande que ce qui est à l'écran.
 */
function Vignette({ chemin, cote, poids }: { chemin: string; cote: number; poids: number }) {
	const source = useAssetSource();
	const ext = extensionDe(chemin);
	const rendable = ext === "g4tx" || ext === "dds" || ext === "png";
	// Le proxy rend la texture PLEINE : mesuré le 2026-09-06, `img_chronicle_artwork_0010.png`
	// pèse 12,2 Mo, et une grille en affiche cinquante. L'amont n'a aucune route de vignette et
	// ignore tout paramètre de taille (`?w=`, `?size=`… : 200, 12 176 145 o à chaque fois).
	// Faute de miniature, on ne charge que ce qui EST déjà une miniature — les atlas d'interface
	// tiennent sous ce seuil, les illustrations pleine page non. Le pictogramme dit la même
	// chose qu'une image blanche, en 600 Mo de moins.
	const url = rendable && poids <= POIDS_VIGNETTE ? source.urlTexture?.(chemin) : undefined;
	if (!url) {
		return (
			<span
				aria-hidden="true"
				style={{
					display: "flex",
					alignItems: "center",
					justifyContent: "center",
					width: "100%",
					height: cote * 0.7,
					background: "var(--jeu-tuile-bord)",
					borderRadius: "var(--jeu-rayon)",
					opacity: 0.5,
				}}
			>
				<IconeDossier />
			</span>
		);
	}
	return (
		<img
			src={url}
			alt=""
			loading="lazy"
			style={{
				width: "100%",
				height: cote * 0.7,
				objectFit: "contain",
				background: "var(--jeu-tuile-bord)",
				borderRadius: "var(--jeu-rayon)",
			}}
		/>
	);
}

/**
 * Le pictogramme d'un dossier — un tracé, pas une émoji.
 *
 * `📁` dépend d'une police d'émojis installée sur la machine du lecteur : là où elle manque, le
 * caractère se rend en rectangle vide, et rien ne le signale.
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

const LIEN: React.CSSProperties = {
	background: "none",
	border: "none",
	color: "var(--jeu-tuile-bas)",
	cursor: "pointer",
	font: "inherit",
	fontWeight: 700,
	padding: 0,
	textDecoration: "underline",
};

const CHAMP: React.CSSProperties = {
	padding: "var(--jeu-espace-xs) var(--jeu-espace-s)",
	background: "#fff",
	border: "2px solid var(--jeu-tuile-bord)",
	borderRadius: "var(--jeu-rayon)",
	color: "var(--jeu-nuit-profonde)",
	font: "inherit",
	minWidth: 0,
};

const ETIQUETTE: React.CSSProperties = {
	display: "inline-flex",
	alignItems: "center",
	gap: "var(--jeu-espace-xs)",
	fontWeight: 700,
	fontSize: "0.9rem",
};

const TITRE_PANNEAU: React.CSSProperties = {
	margin: "0 0 var(--jeu-espace-s)",
	fontSize: "1rem",
	fontWeight: 800,
};

/** Une tuile de la vue grille. Fonction et non constante : son côté est réglable. */
const TUILE = (cote: number): React.CSSProperties => ({
	display: "flex",
	flexDirection: "column",
	alignItems: "center",
	gap: 2,
	width: "100%",
	padding: "var(--jeu-espace-xs)",
	background: "none",
	border: "1px solid var(--jeu-tuile-bord)",
	borderRadius: "var(--jeu-rayon)",
	color: "var(--jeu-nuit-profonde)",
	font: "inherit",
	fontSize: "0.75rem",
	textAlign: "center",
	cursor: "pointer",
	minHeight: cote,
});

const LIGNE: React.CSSProperties = {
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
