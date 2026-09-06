/**
 * `/recherche` — chercher dans les 255 308 entrées, avec tout ce que l'index sait faire.
 *
 * ## Pourquoi cet écran existe séparément de l'explorateur
 *
 * L'explorateur répond à « qu'y a-t-il **ici** » : il montre un dossier, et son filtre porte
 * sur ce dossier. Cette page répond à « où est **ceci** » : elle traverse l'arbre entier. Ce
 * sont deux questions, et les fondre en une seule aurait donné deux façons de dire la même
 * chose sur le même écran — un préfixe qu'on navigue *et* un préfixe qu'on tape.
 *
 * C'est aussi le seul endroit où les cinq derniers filtres servis ont un sens : le **préfixe**
 * (restreindre à un sous-arbre sans y naviguer), le **motif glob**, le **pack CPK** d'origine,
 * et les deux bornes de taille. `scripts/validation/mesurer-matrice-filtres.sh` les mesurait
 * `SERVI` depuis le 2026-09-06 sans qu'aucune commande n'y mène.
 *
 * ## Ce que la page n'invente pas
 *
 * Le compte affiché, l'ordre, et **ce qui a été appliqué** viennent tous de la réponse
 * (`filtres`), jamais de ce qui a été demandé. Les deux diffèrent : une extension inconnue de
 * l'index ressort avec `ext_inconnue`, un pack inconnu avec `cpk_inconnu`, un glob qui ne
 * filtre rien avec `glob_vide`. Sans ces trois drapeaux, une faute de frappe se présente comme
 * un corpus vide — et le lecteur corrige sa question au lieu de sa graphie.
 */
import { useEffect, useMemo, useState } from "react";
import { accorde, Note, TitreVue } from "./Ecran";

/** Un fichier, tel que l'index le rend. */
interface Fichier {
	chemin: string;
	nom: string;
	taille: number;
	cpk?: string | null;
}

/** Ce que le serveur dit avoir appliqué. */
interface FiltresAppliques {
	q: string | null;
	prefixe: string | null;
	glob: string | null;
	ext: string | null;
	cpk: string | null;
	taille_min: number | null;
	taille_max: number | null;
	tri: string;
	ordre: string;
	glob_vide: boolean;
	ext_inconnue: boolean;
	cpk_inconnu: boolean;
}

interface Resultat {
	fichiers: Fichier[];
	total: number;
	total_sans_filtre: number;
	page: number;
	per_page: number;
	filtres: FiltresAppliques;
}

const TAILLES_PAGE = [25, 50, 100, 200] as const;
const MO = 1024 * 1024;

type Etat = {
	q: string;
	prefixe: string;
	glob: string;
	ext: string;
	cpk: string;
	minMo: string;
	maxMo: string;
	tri: "nom" | "taille";
	ordre: "asc" | "desc";
	parPage: number;
	page: number;
};

const VIDE: Etat = {
	q: "",
	prefixe: "",
	glob: "",
	ext: "",
	cpk: "",
	minMo: "",
	maxMo: "",
	tri: "nom",
	ordre: "asc",
	parPage: 50,
	page: 1,
};

function etatDeLUrl(): Etat {
	const p = new URLSearchParams(window.location.search);
	const parPage = Number(p.get("par_page"));
	const page = Number(p.get("page"));
	return {
		...VIDE,
		q: p.get("q") ?? "",
		prefixe: p.get("prefixe") ?? "",
		glob: p.get("glob") ?? "",
		ext: p.get("ext") ?? "",
		cpk: p.get("cpk") ?? "",
		minMo: p.get("min_mo") ?? "",
		maxMo: p.get("max_mo") ?? "",
		tri: p.get("tri") === "taille" ? "taille" : "nom",
		ordre: p.get("ordre") === "desc" ? "desc" : "asc",
		parPage: TAILLES_PAGE.includes(parPage as (typeof TAILLES_PAGE)[number]) ? parPage : 50,
		page: Number.isFinite(page) && page >= 1 ? page : 1,
	};
}

/**
 * Convertit des mégaoctets saisis en octets.
 *
 * `undefined` pour une saisie vide **ou illisible** : `NaN` ferait un `400` sur une frappe en
 * cours. `0` est en revanche une borne légitime — il existe des fichiers de zéro octet — donc
 * le test porte sur la lisibilité, jamais sur la vérité de la valeur.
 */
function octets(mo: string): number | undefined {
	const n = Number(mo.replace(",", "."));
	return mo.trim() && Number.isFinite(n) && n >= 0 ? Math.round(n * MO) : undefined;
}

function query(e: Etat, pourLUrl: boolean): URLSearchParams {
	const p = new URLSearchParams();
	const poser = (cle: string, v: string) => {
		if (v.trim()) p.set(cle, v.trim());
	};
	poser("q", e.q);
	poser("prefixe", e.prefixe);
	poser("glob", e.glob);
	poser("ext", e.ext.replace(/^\./, ""));
	poser("cpk", e.cpk);
	if (e.tri !== "nom") p.set("tri", e.tri);
	if (e.ordre !== "asc") p.set("ordre", e.ordre);
	if (e.page !== 1) p.set("page", String(e.page));
	if (pourLUrl) {
		poser("min_mo", e.minMo);
		poser("max_mo", e.maxMo);
		if (e.parPage !== 50) p.set("par_page", String(e.parPage));
	} else {
		const min = octets(e.minMo);
		const max = octets(e.maxMo);
		if (min !== undefined) p.set("taille_min", String(min));
		if (max !== undefined) p.set("taille_max", String(max));
		p.set("per_page", String(e.parPage));
	}
	return p;
}

function taille(o: number): string {
	if (o < 1024) return `${o} o`;
	if (o < MO) return `${(o / 1024).toFixed(1)} ko`;
	return `${(o / MO).toFixed(1)} Mo`;
}

export function Recherche() {
	const initial = useMemo(etatDeLUrl, []);
	// `brouillon` est ce qui est tapé, `etat` ce qui a été envoyé. Sept champs qui partiraient à
	// chaque frappe feraient sept requêtes sur 255 308 chemins pour un résultat que personne ne
	// lit ; la recherche se soumet.
	const [brouillon, setBrouillon] = useState<Etat>(initial);
	const [etat, setEtat] = useState<Etat>(initial);
	const [resultat, setResultat] = useState<Resultat | null>(null);
	const [erreur, setErreur] = useState<string | null>(null);

	useEffect(() => {
		const url = new URL(window.location.href);
		url.search = query(etat, true).toString();
		window.history.replaceState(window.history.state, "", url);

		const ac = new AbortController();
		setErreur(null);
		fetch(`/api/v1/recherche?${query(etat, false)}`, {
			signal: ac.signal,
			headers: { accept: "application/json" },
		})
			.then((r) => (r.ok ? r.json() : Promise.reject(new Error(String(r.status)))))
			.then((r: Resultat) => {
				if (!ac.signal.aborted) setResultat(r);
			})
			.catch((e: Error) => {
				if (ac.signal.aborted) return;
				setResultat(null);
				setErreur(
					e.message === "400"
						? "Une des bornes n'est pas un nombre que l'index accepte."
						: "La recherche n'a pas pu aboutir. Réessayez dans un instant.",
				);
			});
		return () => ac.abort();
	}, [etat]);

	const f = resultat?.filtres;
	// Trois avertissements que seul le serveur peut donner : la valeur est syntaxiquement
	// valide, mais ne désigne rien dans l'index.
	const avertissements = [
		f?.ext_inconnue ? `l'extension « ${f.ext} » n'existe nulle part dans l'index` : null,
		f?.cpk_inconnu ? `le pack « ${f.cpk} » n'est pas un pack indexé` : null,
		f?.glob_vide ? "ce motif ne filtre rien : il a été ignoré" : null,
	].filter(Boolean) as string[];

	const champ = (
		cle: keyof Etat,
		libelle: string,
		aide: string,
		largeur = "12rem",
	) => (
		<label style={ETIQUETTE}>
			<span style={{ fontSize: "0.8rem", opacity: 0.75 }}>{libelle}</span>
			<input
				type="text"
				value={String(brouillon[cle])}
				onChange={(e) => setBrouillon((v) => ({ ...v, [cle]: e.target.value }))}
				placeholder={aide}
				aria-label={libelle}
				style={{ ...CHAMP, width: largeur }}
			/>
		</label>
	);

	return (
		<section>
			<TitreVue
				appoint={
					resultat
						? `${accorde(resultat.total, "fichier")} sur ${resultat.total_sans_filtre.toLocaleString("fr")}`
						: undefined
				}
			>
				Rechercher
			</TitreVue>

			<form
				onSubmit={(e) => {
					e.preventDefault();
					setEtat({ ...brouillon, page: 1 });
				}}
				style={{
					display: "flex",
					flexWrap: "wrap",
					alignItems: "flex-end",
					gap: "var(--jeu-espace-m)",
					margin: "var(--jeu-espace-m) 0",
				}}
			>
				{champ("q", "Contient", "chara_base", "16rem")}
				{champ("prefixe", "Sous le dossier", "data/dx11/menu", "16rem")}
				{champ("glob", "Motif", "data/**/*.g4tx,!**/movie/**", "18rem")}
				{champ("ext", "Extension", "g4tx", "8rem")}
				{champ("cpk", "Pack", "nom du .cpk", "12rem")}
				{champ("minMo", "Au moins (Mo)", "0", "7rem")}
				{champ("maxMo", "Au plus (Mo)", "∞", "7rem")}
				<label style={ETIQUETTE}>
					<span style={{ fontSize: "0.8rem", opacity: 0.75 }}>Trier par</span>
					<select
						value={`${brouillon.tri}-${brouillon.ordre}`}
						onChange={(e) => {
							const [t, o] = e.target.value.split("-");
							setBrouillon((v) => ({
								...v,
								tri: t === "taille" ? "taille" : "nom",
								ordre: o === "desc" ? "desc" : "asc",
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
					<span style={{ fontSize: "0.8rem", opacity: 0.75 }}>Par page</span>
					<select
						value={String(brouillon.parPage)}
						onChange={(e) =>
							setBrouillon((v) => ({ ...v, parPage: Number(e.target.value) }))
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
				<button type="submit" style={{ ...CHAMP, cursor: "pointer", fontWeight: 700 }}>
					Chercher
				</button>
				<button
					type="button"
					onClick={() => {
						setBrouillon(VIDE);
						setEtat(VIDE);
					}}
					style={{ ...CHAMP, cursor: "pointer" }}
				>
					Tout effacer
				</button>
			</form>

			{avertissements.length > 0 ? (
				<Note ton="alerte">{avertissements.join(" · ")}</Note>
			) : null}
			{erreur ? <Note ton="alerte">{erreur}</Note> : null}

			{!resultat ? (
				erreur ? null : (
					<Note>Chargement…</Note>
				)
			) : resultat.fichiers.length === 0 ? (
				<Note>Aucun fichier ne répond à cette question.</Note>
			) : (
				<>
					<ul style={{ listStyle: "none", margin: 0, padding: 0 }}>
						{resultat.fichiers.map((x) => (
							<li key={x.chemin}>
								{/* Le chemin VERBATIM sert d'adresse : extension du jeu conservée. */}
								<a href={`/f/${x.chemin}`} style={LIGNE}>
									<span
										style={{
											flex: 1,
											minWidth: 0,
											overflow: "hidden",
											textOverflow: "ellipsis",
											whiteSpace: "nowrap",
											direction: "rtl",
											textAlign: "left",
										}}
									>
										{x.chemin}
									</span>
									<span style={{ opacity: 0.7, fontSize: "0.8rem", whiteSpace: "nowrap" }}>
										{taille(x.taille)}
									</span>
								</a>
							</li>
						))}
					</ul>
					{resultat.total > resultat.per_page ? (
						<nav
							aria-label="Pagination"
							style={{
								display: "flex",
								alignItems: "center",
								gap: "var(--jeu-espace-m)",
								marginTop: "var(--jeu-espace-m)",
							}}
						>
							<button
								type="button"
								disabled={etat.page <= 1}
								onClick={() => setEtat((v) => ({ ...v, page: v.page - 1 }))}
								style={CHAMP}
							>
								Précédent
							</button>
							<span aria-live="polite" style={{ fontWeight: 700 }}>
								Page {etat.page} sur{" "}
								{Math.ceil(resultat.total / resultat.per_page).toLocaleString("fr")}
							</span>
							<button
								type="button"
								disabled={etat.page >= Math.ceil(resultat.total / resultat.per_page)}
								onClick={() => setEtat((v) => ({ ...v, page: v.page + 1 }))}
								style={CHAMP}
							>
								Suivant
							</button>
						</nav>
					) : null}
				</>
			)}
		</section>
	);
}

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
	flexDirection: "column",
	gap: 2,
	fontWeight: 700,
};

const LIGNE: React.CSSProperties = {
	display: "flex",
	alignItems: "center",
	gap: "var(--jeu-espace-s)",
	width: "100%",
	padding: "var(--jeu-espace-xs) var(--jeu-espace-s)",
	borderBottom: "1px solid var(--jeu-tuile-bord)",
	color: "var(--jeu-nuit-profonde)",
	textDecoration: "none",
};
