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
import { accorde, Note } from "./Ecran";

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

/** Une valeur d'une facette, telle que `?facets=` la rend. `value: null` = vide ou nul. */
interface FacetValeur {
	value: string | null;
	count: number;
}

/** Les valeurs d'une colonne et leur compte, sous les filtres en cours. */
interface Facet {
	column: string;
	distinct: number;
	truncated: boolean;
	values: FacetValeur[];
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
	/** Absent quand `?facets` l'est — le serveur ne publie pas une clé toujours vide. */
	facets?: Facet[];
}

/** Les tailles de page proposées — le serveur plafonne à 200. */
const TAILLES_PAGE = [25, 50, 100, 200] as const;

/** Le jeton qui demande les lignes où une colonne est renseignée. */
const PRESENT = "__present__";

/** Le jeton qui demande les lignes où une colonne est vide ou nulle. */
const ABSENT = "__absent__";

/**
 * Combien de colonnes on peut faceter d'un coup — la borne du serveur, recopiée ici.
 *
 * Elle est recopiée plutôt que devinée : au-delà, le serveur rend un `400` qui nomme la limite.
 * Une interface qui laisserait cocher une treizième colonne ferait échouer la requête entière,
 * donc disparaître la liste — un clic qui casse l'écran est pire qu'un bouton désactivé.
 */
const FACETS_MAX = 12;

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
	/**
	 * Les colonnes dont on veut les valeurs comptées.
	 *
	 * **Demandées, jamais devinées.** Le serveur seul sait ce que porte une colonne ; choisir
	 * ici « les colonnes qui ont l'air d'être des catégories » reviendrait à inventer un schéma
	 * — et une table de 40 colonnes rendrait 40 `GROUP BY` pour trois utiles. C'est donc un
	 * geste : on ouvre une colonne, et elle se compte.
	 */
	facettes: string[];
};

/** L'état d'une table fraîchement choisie : tout est remis à zéro sauf le nom. */
function etatNeuf(table: string): Etat {
	return { table, q: "", tri: "", ordre: "asc", parPage: 50, page: 1, filtres: [], facettes: [] };
}

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
		// Les colonnes déjà filtrées s'ouvrent d'office : une adresse partagée montre alors
		// POURQUOI la liste est réduite, et quelles autres valeurs existaient.
		facettes: [...p.entries()]
			.map(([c]) => c)
			.filter((c) => !RESERVES.has(c) && !c.endsWith("__min") && !c.endsWith("__max"))
			.slice(0, FACETS_MAX),
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
	// Jamais dans l'URL : `facets` ne change pas ce qui est affiché dans la liste, seulement ce
	// que le serveur compte à côté. L'écrire ferait deux adresses pour le même écran.
	if (!pourLUrl && e.facettes.length > 0) p.set("facets", e.facettes.join(","));
	return p;
}

/**
 * Écrit la seule clé que ce panneau revendique dans l'URL : la table lue.
 *
 * Depuis la fusion, l'adresse est celle de l'explorateur — elle porte déjà `d`, `q`, `ext`,
 * `tri`… Y écrire aussi les filtres du panneau ferait deux `q` pour deux corpus différents, et
 * le premier des deux écraserait l'autre en silence. La table, elle, ne collisionne avec rien
 * et suffit à retrouver l'écran.
 */
function ecrireUrl(e: Etat) {
	const url = new URL(window.location.href);
	if (e.table) url.searchParams.set("table", e.table);
	else url.searchParams.delete("table");
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

/**
 * Le bloc « Données » de l'explorateur.
 *
 * Ce n'est plus une page depuis la fusion du 2026-09-06 : c'est le second volet du panneau de
 * droite, monté à côté du contexte du dossier et de l'asset. Les **routes API n'ont pas
 * bougé** — `/api/v1/entites` et `/api/v1/entites/{table}`, avec leurs filtres, leur tri et
 * leur export.
 *
 * `contexte` est le nom de l'asset sélectionné, sans extension : il pré-remplit la recherche.
 * Un fichier du jeu et la ligne qui le décrit portent souvent le même code, et c'est
 * exactement le rapprochement qu'on venait chercher en ouvrant les deux écrans côte à côte.
 */
export function PanneauDonnees({ contexte }: { contexte?: string }) {
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

	/** Ouvre ou referme le comptage des valeurs d'une colonne. */
	const basculerFacette = (colonne: string) => {
		setEtat((e) => ({
			...e,
			facettes: e.facettes.includes(colonne)
				? e.facettes.filter((c) => c !== colonne)
				: [...e.facettes, colonne].slice(0, FACETS_MAX),
		}));
	};

	// L'asset sélectionné dans la liste devient la recherche du panneau. Le geste qu'on faisait
	// à la main entre deux onglets — copier un code de fichier, le coller dans une table — est
	// désormais le comportement par défaut.
	useEffect(() => {
		if (!contexte) return;
		setSaisie(contexte);
		setEtat((e) => ({ ...e, q: contexte, page: 1 }));
	}, [contexte]);

	if (erreur && !tables) return <Note ton="alerte">{erreur}</Note>;
	if (!tables) return <Note>Chargement…</Note>;

	return (
		<section>
			<h3 style={{ margin: "0 0 var(--jeu-espace-s)", fontSize: "1rem", fontWeight: 800 }}>
				Données · {accorde(tables.length, "table")}
			</h3>

			<div style={{ display: "flex", flexWrap: "wrap", gap: "var(--jeu-espace-m)", margin: "var(--jeu-espace-m) 0" }}>
				<label style={ETIQUETTE}>
					Table
					<select
						value={etat.table}
						onChange={(e) => setEtat(etatNeuf(e.target.value))}
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
							href={`/api/v1/entites/${etat.table}?${query({ ...etat, parPage: 200, facettes: [] }, false)}&format=csv`}
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
					  * Les valeurs d'une colonne, comptées par le serveur sous les filtres en
					  * cours. C'est la différence entre un filtre qu'on devine et un filtre
					  * qu'on lit : sans les comptes, il faut connaître l'orthographe exacte de
					  * « Forêt » pour l'écrire dans un champ libre.
					  *
					  * Le compte d'une facette exclut le filtre de SA propre colonne : c'est ce
					  * qui permet d'en choisir une seconde. Sous `element=Feu`, la facette
					  * `element` continue d'afficher `Forêt 1 600`, pendant que `position` est
					  * bien recalculée sous `Feu`.
					  */}
					{(page?.facets?.length ?? 0) > 0 ? (
						<div style={{ margin: "0 0 var(--jeu-espace-m)", display: "grid", gap: "var(--jeu-espace-s)" }}>
							{page?.facets?.map((f) => (
								<div key={f.column}>
									<div style={{ display: "flex", alignItems: "baseline", gap: "var(--jeu-espace-xs)", flexWrap: "wrap" }}>
										<strong style={{ fontSize: "0.85rem" }}>{f.column}</strong>
										<span style={{ fontSize: "0.78rem", opacity: 0.7 }}>
											{f.truncated
												? `${f.values.length} des ${f.distinct.toLocaleString("fr")} valeurs, les plus fournies`
												: accorde(f.distinct, "valeur")}
										</span>
										<button
											type="button"
											onClick={() => basculerFacette(f.column)}
											style={{ ...CHAMP, cursor: "pointer", padding: "0 var(--jeu-espace-xs)", fontSize: "0.78rem" }}
											aria-label={`Masquer les valeurs de ${f.column}`}
										>
											masquer
										</button>
									</div>
									<div style={{ display: "flex", flexWrap: "wrap", gap: 4, marginTop: 4 }}>
										{f.values.map((v) => {
											// Une valeur nulle se filtre par le jeton de présence, pas par une
											// chaîne vide : le serveur refuse `?colonne=` — et il a raison, un
											// filtre vide ne veut rien dire.
											const jeton = v.value ?? ABSENT;
											const actif = valeurDe(f.column) === jeton;
											return (
												<button
													key={jeton}
													type="button"
													aria-pressed={actif}
													onClick={() => poserFiltre(f.column, actif ? "" : jeton)}
													style={{
														...CHAMP,
														cursor: "pointer",
														fontSize: "0.8rem",
														padding: "2px var(--jeu-espace-xs)",
														background: actif ? "var(--jeu-surface-glace)" : "#fff",
														borderColor: actif
															? "var(--jeu-nuit-profonde)"
															: "var(--jeu-tuile-bord)",
														fontWeight: actif ? 800 : 400,
													}}
												>
													{v.value ?? "vide"}{" "}
													<span style={{ opacity: 0.65 }}>{v.count.toLocaleString("fr")}</span>
												</button>
											);
										})}
									</div>
								</div>
							))}
						</div>
					) : null}

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
									<span style={{ display: "flex", alignItems: "baseline", gap: 4, fontSize: "0.8rem", opacity: 0.75 }}>
										<span style={{ flex: 1, minWidth: 0 }}>
											{c.nom} · {c.type_sql || "?"}
										</span>
										{/*
										  * « Valeurs » demande au serveur de compter cette colonne. Le
										  * bouton se désactive à la douzième — au-delà le serveur rend un
										  * 400 qui ferait disparaître la liste entière, et un clic qui
										  * casse l'écran est pire qu'un bouton éteint.
										  */}
										<button
											type="button"
											onClick={() => basculerFacette(c.nom)}
											disabled={
												!etat.facettes.includes(c.nom) && etat.facettes.length >= FACETS_MAX
											}
											aria-pressed={etat.facettes.includes(c.nom)}
											title={
												etat.facettes.includes(c.nom)
													? "Masquer les valeurs"
													: etat.facettes.length >= FACETS_MAX
														? `${FACETS_MAX} colonnes comptées au maximum`
														: "Compter les valeurs de cette colonne"
											}
											style={{
												border: "none",
												background: "none",
												padding: 0,
												font: "inherit",
												cursor: "pointer",
												textDecoration: "underline",
												opacity:
													!etat.facettes.includes(c.nom) && etat.facettes.length >= FACETS_MAX
														? 0.4
														: 1,
											}}
										>
											{etat.facettes.includes(c.nom) ? "valeurs ✓" : "valeurs"}
										</button>
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
