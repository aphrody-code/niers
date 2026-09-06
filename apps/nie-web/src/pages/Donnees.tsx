/**
 * `/donnees` — les 224 tables des deux gisements, avec les filtres que le serveur sert déjà.
 *
 * ## Pourquoi cette page existe
 *
 * `scripts/validation/mesurer-matrice-filtres.sh` a mesuré le 2026-09-06 que
 * `/api/v1/entites/{table}` servait la recherche, le tri, l'égalité sur **toute colonne**, les
 * intervalles, les tests de présence et l'export CSV — sur **224 tables**, et que **rien** dans
 * l'interface n'y menait. Le retard n'était pas dans le serveur ; il était ici.
 *
 * ## Ce que la page ne fait pas
 *
 * - **Elle n'invente aucune colonne, aucun libellé, aucune facette.** Le schéma vient de
 *   `/api/v1/entites`, qui le mesure sur `sqlite_master` et `PRAGMA table_info`. Une colonne
 *   affichée est une colonne de la base — c'est la règle anti-hallucination du dépôt, tenue par
 *   construction plutôt que par vigilance.
 * - **Elle ne joint pas les deux gisements.** Le jeu et la série n'ont aucune clé commune ; la
 *   page affiche donc le gisement de chaque table et s'arrête là.
 * - **Elle ne devine pas un type.** Les bornes numériques ne sont proposées que sur les
 *   colonnes que le serveur déclare non textuelles — parce que lui refuse une fourchette sur du
 *   texte, et qu'une commande à l'écran qui mène à un `400` est pire qu'une commande absente.
 */
import { useEffect, useMemo, useState } from "react";
import { accorde, Note, TitreVue } from "./Ecran";

/** Une colonne, telle que `/api/v1/entites` la mesure. */
interface Colonne {
	nom: string;
	type_sql: string;
	texte: boolean;
}

/** Une table servable, avec son schéma et son compte de lignes. */
interface TableServie {
	gisement: string;
	nom: string;
	cle: string;
	colonnes: Colonne[];
	lignes: number;
}

/** Une page de lignes, telle que `/api/v1/entites/{table}` la rend. */
interface PageLignes {
	elements: Record<string, unknown>[];
	total: number;
	page: number;
	pages: number;
	gisement: string;
	table: string;
	cle: string;
}

/** Les tailles de page proposées — le serveur plafonne à 200. */
const TAILLES_PAGE = [25, 50, 100, 200] as const;

/** Le jeton qui demande les lignes où une colonne est renseignée. */
const PRESENT = "__present__";

/**
 * L'état de la page, tel qu'il vit dans l'URL.
 *
 * `filtres` porte les paires colonne→valeur brutes, exactement comme elles partiront en query :
 * les traduire en un modèle plus riche obligerait à les retraduire pour l'URL, et c'est là que
 * les deux formes divergent.
 */
type Etat = {
	table: string;
	q: string;
	tri: string;
	ordre: "asc" | "desc";
	parPage: number;
	page: number;
	filtres: [string, string][];
};

/** Les clés que la page se réserve : tout le reste de l'URL est un filtre de colonne. */
const RESERVES = new Set(["vue", "table", "q", "tri", "ordre", "par_page", "page"]);

function etatDeLUrl(): Etat {
	const p = new URLSearchParams(window.location.search);
	const parPage = Number(p.get("par_page"));
	const page = Number(p.get("page"));
	return {
		table: p.get("table") ?? "",
		q: p.get("q") ?? "",
		tri: p.get("tri") ?? "",
		ordre: p.get("ordre") === "desc" ? "desc" : "asc",
		parPage: TAILLES_PAGE.includes(parPage as (typeof TAILLES_PAGE)[number]) ? parPage : 50,
		page: Number.isFinite(page) && page >= 1 ? page : 1,
		filtres: [...p.entries()].filter(([c]) => !RESERVES.has(c)),
	};
}

/** Construit la query d'une requête — et, aux réserves près, celle de l'URL. */
function query(e: Etat, pourLUrl: boolean): URLSearchParams {
	const p = new URLSearchParams();
	if (pourLUrl && e.table) p.set("table", e.table);
	if (e.q.trim()) p.set("q", e.q.trim());
	if (e.tri.trim()) p.set("tri", e.tri.trim());
	if (e.ordre === "desc") p.set("ordre", "desc");
	// Un défaut ne s'écrit pas dans l'URL : deux adresses pour le même écran cassent le partage.
	if (!pourLUrl || e.parPage !== 50) p.set("par_page", String(e.parPage));
	if (e.page !== 1) p.set("page", String(e.page));
	for (const [colonne, valeur] of e.filtres) {
		if (valeur.trim()) p.set(colonne, valeur.trim());
	}
	return p;
}

function ecrireUrl(e: Etat) {
	const url = new URL(window.location.href);
	url.search = query(e, true).toString();
	window.history.replaceState(window.history.state, "", url);
}

/** Lit une route de l'API, en propageant l'annulation. */
async function lire<T>(chemin: string, signal: AbortSignal): Promise<T> {
	const r = await fetch(chemin, { signal, headers: { accept: "application/json" } });
	if (!r.ok) throw new Error(String(r.status));
	return (await r.json()) as T;
}

/** Rend une valeur JSON en une cellule lisible, sans jamais l'inventer. */
function cellule(v: unknown): string {
	if (v === null || v === undefined) return "—";
	if (typeof v === "string") return v;
	// `JSON.stringify` plutôt que `String` : `String({})` rend « [object Object] », qui n'est
	// pas une donnée mais un artefact de langage.
	return typeof v === "object" ? JSON.stringify(v) : String(v);
}

export function Donnees() {
	const initial = useMemo(etatDeLUrl, []);
	const [etat, setEtat] = useState<Etat>(initial);
	const [tables, setTables] = useState<TableServie[] | null>(null);
	const [page, setPage] = useState<PageLignes | null>(null);
	const [erreur, setErreur] = useState<string | null>(null);
	const [saisie, setSaisie] = useState(initial.q);

	// Le catalogue, une fois. `par_page=200` est le plafond du serveur : les 224 tables tiennent
	// en deux appels, et le second n'est fait que s'il y a une suite.
	useEffect(() => {
		const ac = new AbortController();
		(async () => {
			try {
				const p1 = await lire<{ elements: TableServie[]; pages: number }>(
					"/api/v1/entites?per_page=200",
					ac.signal,
				);
				// Le nombre de pages est connu dès la première réponse : les suivantes partent
				// ensemble. Les enchaîner ferait attendre un aller-retour par page pour une
				// dépendance qui n'existe pas.
				const suites = await Promise.all(
					Array.from({ length: Math.max(0, p1.pages - 1) }, (_, i) =>
						lire<{ elements: TableServie[] }>(
							`/api/v1/entites?per_page=200&page=${i + 2}`,
							ac.signal,
						),
					),
				);
				const toutes = [...p1.elements, ...suites.flatMap((s) => s.elements)];
				if (!ac.signal.aborted) setTables(toutes);
			} catch {
				if (!ac.signal.aborted) setErreur("Le catalogue des données n'est pas disponible.");
			}
		})();
		return () => ac.abort();
	}, []);

	const table = tables?.find((t) => t.nom === etat.table) ?? null;

	useEffect(() => {
		ecrireUrl(etat);
		if (!etat.table) {
			setPage(null);
			return;
		}
		const ac = new AbortController();
		setErreur(null);
		lire<PageLignes>(`/api/v1/entites/${etat.table}?${query(etat, false)}`, ac.signal)
			.then((p) => {
				if (!ac.signal.aborted) setPage(p);
			})
			.catch((e: Error) => {
				if (ac.signal.aborted) return;
				setPage(null);
				// Un 400 vient d'un filtre que la table ne comprend pas ; le dire évite de
				// laisser croire à une panne.
				setErreur(
					e.message === "400"
						? "Un des filtres ne s'applique pas à cette table."
						: "Ces lignes n'ont pas pu être chargées.",
				);
			});
		return () => ac.abort();
	}, [etat]);

	/** Pose ou retire un filtre de colonne, et revient à la première page. */
	const poserFiltre = (colonne: string, valeur: string) => {
		setEtat((e) => ({
			...e,
			page: 1,
			filtres: [
				...e.filtres.filter(([c]) => c !== colonne),
				...(valeur ? ([[colonne, valeur]] as [string, string][]) : []),
			],
		}));
	};
	const valeurDe = (colonne: string) =>
		etat.filtres.find(([c]) => c === colonne)?.[1] ?? "";

	if (erreur && !tables) return <Note ton="alerte">{erreur}</Note>;
	if (!tables) return <Note>Chargement…</Note>;

	return (
		<section>
			<TitreVue appoint={accorde(tables.length, "table")}>Données</TitreVue>

			<div style={{ display: "flex", flexWrap: "wrap", gap: "var(--jeu-espace-m)", margin: "var(--jeu-espace-m) 0" }}>
				<label style={ETIQUETTE}>
					Table
					<select
						value={etat.table}
						onChange={(e) =>
							setEtat({
								table: e.target.value,
								q: "",
								tri: "",
								ordre: "asc",
								parPage: 50,
								page: 1,
								filtres: [],
							})
						}
						style={{ ...CHAMP, maxWidth: "22rem" }}
					>
						<option value="">Choisir…</option>
						{tables.map((t) => (
							<option key={t.nom} value={t.nom}>
								{t.nom} · {t.lignes.toLocaleString("fr")}
							</option>
						))}
					</select>
				</label>
				{table ? (
					<>
						<label style={ETIQUETTE}>
							Par page
							<select
								value={String(etat.parPage)}
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
						<a
							href={`/api/v1/entites/${etat.table}?${query({ ...etat, parPage: 200 }, false)}&format=csv`}
							style={{ ...ETIQUETTE, textDecoration: "underline" }}
						>
							Exporter cette page en CSV
						</a>
					</>
				) : null}
			</div>

			{!table ? (
				<Note>
					Choisissez une table. Chacune porte son gisement — les données du jeu, ou le
					catalogue de la série.
				</Note>
			) : (
				<>
					<form
						onSubmit={(e) => {
							e.preventDefault();
							setEtat((v) => ({ ...v, q: saisie.trim(), page: 1 }));
						}}
						style={{ display: "flex", gap: "var(--jeu-espace-s)", margin: "0 0 var(--jeu-espace-m)" }}
					>
						<input
							type="search"
							value={saisie}
							onChange={(e) => setSaisie(e.target.value)}
							placeholder={`Chercher dans ${table.nom}`}
							aria-label={`Chercher dans ${table.nom}`}
							style={{ ...CHAMP, flex: 1 }}
						/>
						<button type="submit" style={CHAMP}>
							Chercher
						</button>
					</form>

					{/*
					  * Une commande par colonne MESURÉE, jamais par facette devinée. Les bornes
					  * ne sont proposées que sur les colonnes non textuelles : le serveur refuse
					  * une fourchette sur du texte, et une commande qui mène à un 400 est pire
					  * qu'une commande absente.
					  */}
					<details style={{ margin: "0 0 var(--jeu-espace-m)" }}>
						<summary style={{ cursor: "pointer", fontWeight: 700 }}>
							Filtrer par colonne ({table.colonnes.length})
						</summary>
						<div
							style={{
								display: "grid",
								gridTemplateColumns: "repeat(auto-fill, minmax(220px, 1fr))",
								gap: "var(--jeu-espace-s)",
								margin: "var(--jeu-espace-s) 0",
							}}
						>
							{table.colonnes.map((c) => (
								<label key={c.nom} style={{ ...ETIQUETTE, alignItems: "stretch", flexDirection: "column", gap: 2 }}>
									<span style={{ fontSize: "0.8rem", opacity: 0.75 }}>
										{c.nom} · {c.type_sql || "?"}
									</span>
									<input
										type="text"
										value={valeurDe(c.nom)}
										onChange={(e) => poserFiltre(c.nom, e.target.value)}
										placeholder={c.texte ? "égal à…" : "égal à…"}
										style={CHAMP}
									/>
									{c.texte ? (
										<button
											type="button"
											onClick={() =>
												poserFiltre(c.nom, valeurDe(c.nom) === PRESENT ? "" : PRESENT)
											}
											style={{ ...CHAMP, cursor: "pointer" }}
										>
											{valeurDe(c.nom) === PRESENT ? "renseignée ✓" : "renseignée"}
										</button>
									) : (
										<span style={{ display: "flex", gap: 2 }}>
											<input
												type="text"
												inputMode="numeric"
												value={valeurDe(`${c.nom}__min`)}
												onChange={(e) => poserFiltre(`${c.nom}__min`, e.target.value)}
												placeholder="min"
												aria-label={`${c.nom} minimum`}
												style={{ ...CHAMP, width: "50%" }}
											/>
											<input
												type="text"
												inputMode="numeric"
												value={valeurDe(`${c.nom}__max`)}
												onChange={(e) => poserFiltre(`${c.nom}__max`, e.target.value)}
												placeholder="max"
												aria-label={`${c.nom} maximum`}
												style={{ ...CHAMP, width: "50%" }}
											/>
										</span>
									)}
								</label>
							))}
						</div>
					</details>

					{erreur ? <Note ton="alerte">{erreur}</Note> : null}

					{page ? (
						<>
							<p style={{ margin: "0 0 var(--jeu-espace-s)", fontSize: "0.9rem", opacity: 0.8 }}>
								{accorde(page.total, "ligne")} · gisement {page.gisement} · clé {page.cle}
							</p>
							<div style={{ overflowX: "auto" }}>
								<table style={{ borderCollapse: "collapse", fontSize: "0.85rem", width: "100%" }}>
									<thead>
										<tr>
											{table.colonnes.map((c) => (
												<th key={c.nom} style={CELLULE_TETE}>
													<button
														type="button"
														onClick={() =>
															setEtat((v) => ({
																...v,
																tri: c.nom,
																ordre: v.tri === c.nom && v.ordre === "asc" ? "desc" : "asc",
																page: 1,
															}))
														}
														style={{ background: "none", border: "none", font: "inherit", fontWeight: 700, cursor: "pointer", padding: 0 }}
													>
														{c.nom}
														{etat.tri === c.nom ? (etat.ordre === "asc" ? " ▲" : " ▼") : ""}
													</button>
												</th>
											))}
										</tr>
									</thead>
									<tbody>
										{page.elements.map((ligne) => (
											<tr key={String(ligne[page.cle] ?? JSON.stringify(ligne))}>
												{table.colonnes.map((c) => (
													<td key={c.nom} style={CELLULE}>
														{cellule(ligne[c.nom])}
													</td>
												))}
											</tr>
										))}
									</tbody>
								</table>
							</div>
							{page.pages > 1 ? (
								<nav aria-label="Pagination" style={{ display: "flex", alignItems: "center", gap: "var(--jeu-espace-m)", marginTop: "var(--jeu-espace-m)" }}>
									<button
										type="button"
										disabled={page.page <= 1}
										onClick={() => setEtat((v) => ({ ...v, page: v.page - 1 }))}
										style={CHAMP}
									>
										Précédent
									</button>
									<span aria-live="polite" style={{ fontWeight: 700 }}>
										Page {page.page} sur {page.pages.toLocaleString("fr")}
									</span>
									<button
										type="button"
										disabled={page.page >= page.pages}
										onClick={() => setEtat((v) => ({ ...v, page: v.page + 1 }))}
										style={CHAMP}
									>
										Suivant
									</button>
								</nav>
							) : null}
						</>
					) : erreur ? null : (
						<Note>Chargement…</Note>
					)}
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
	alignItems: "center",
	gap: "var(--jeu-espace-xs)",
	fontWeight: 700,
};

const CELLULE_TETE: React.CSSProperties = {
	textAlign: "left",
	padding: "var(--jeu-espace-xs) var(--jeu-espace-s)",
	borderBottom: "2px solid var(--jeu-tuile-bord)",
	whiteSpace: "nowrap",
};

const CELLULE: React.CSSProperties = {
	padding: "var(--jeu-espace-xs) var(--jeu-espace-s)",
	borderBottom: "1px solid var(--jeu-tuile-bord)",
	maxWidth: "28rem",
	overflow: "hidden",
	textOverflow: "ellipsis",
	whiteSpace: "nowrap",
};
