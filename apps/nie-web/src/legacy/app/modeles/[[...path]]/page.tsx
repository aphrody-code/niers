/**
 * Le corpus 3D du jeu, en une seule page.
 *
 * Avant : `/modeles` servait techniques, objets et animaux **filtrés par un manifeste statique**
 * (509 codes) et `/keshin` les esprits et armures depuis le miroir — deux pages, deux sources,
 * une fraction du corpus. Mesuré sur le VFS et le miroir, le jeu compte **6 236 modèles** :
 * 5 526 personnages, 272 techniques, 247 objets, 100 esprits, 89 armures, 2 animaux.
 *
 * Trois vues, une route :
 *   /modeles                    les familles et leur volume
 *   /modeles/<famille>          la grille paginée des codes
 *   /modeles/<famille>/<code>   la fiche : modèle 3D assemblé live + ses textures
 *
 * Aucun gate statique. Ce qui existe dans le jeu est listé ; la route d'assemblage répond 404
 * pour ce qu'elle ne sait pas produire, et la fiche le dit — plutôt que d'enfermer le corpus
 * dans une liste écrite à la main qui vieillit en silence.
 */
import type { Metadata } from "next";
import Link from "next/link";
import { MediaBack, MediaCount, MediaEmpty, MediaHeader } from "@/components/wiki/MediaShell";
import InlineModelViewer from "@/components/wiki/InlineModelViewer";
import { ModelToolbar, type ModelView } from "@/components/wiki/ModelToolbar";
import { cpkExplorerHref, lsOrNull } from "@rosegriffon/azalee/cpk/live";
import { cpkThumbUrl } from "@rosegriffon/azalee/cpk/shared";
import {
	MODEL_FAMILIES,
	modelFamily,
	modelGlbUrl,
	modelHref,
	modelTextureDir,
	type ModelFamily,
} from "@rosegriffon/azalee/cpk/models";
import { type ChrModelSub, getChrModelTextureUrl } from "@rosegriffon/azalee/images";
import { filtrer, type ModelEntry, modelesDe } from "@/lib/cpk/model-corpus";

export const dynamic = "force-dynamic";
export const runtime = "nodejs";

/**
 * Codes par page, selon la disposition.
 *
 * La grille monte un vrai `<model-viewer>` par carte visible et chaque GLB est **assemblé à la
 * volée** par le service : on en tient moins par page que dans une liste de texte, qui ne coûte
 * qu'un lien. 120 cartes 3D d'un coup, c'est autant d'assemblages demandés en rafale.
 */
const PAR_PAGE: Record<ModelView, number> = { grid: 48, list: 120 };

/** Rôle d'une extension dans un dossier de modèle — le contexte que porte le fichier. */
const ROLE_EXT: Record<string, string> = {
	g4md: "définition du modèle",
	g4mg: "maillage",
	g4mt: "animation",
	g4pk: "pack",
	g4pkm: "pack de matériaux",
	g4sk: "squelette",
	g4tx: "textures",
	objbin: "table d'objets",
};

/**
 * URL de la texture d'un modèle, quand elle est connue.
 *
 * Le maillage `/model-chr` n'embarque pas sa texture : la carte l'affiche à part. Le nom du
 * fichier n'est PAS toujours celui du dossier (variantes `_10`/`_20`, objets sans texture),
 * d'où le passage par la carte relevée sur l'index CPK plutôt que par un chemin forgé — 38 cas
 * rendaient un 404. Ce relevé ne couvre que `waza`/`item`/`animal` ; ailleurs, `null`.
 *
 * C'est un BONUS, jamais un filtre : un modèle sans texture connue reste listé et visualisable.
 */
function textureDe(famille: ModelFamily, code: string): string | null {
	if (famille !== "waza" && famille !== "item" && famille !== "animal") return null;
	return getChrModelTextureUrl(famille satisfies ChrModelSub, code);
}

/** Poids lisible — un octet brut ne dit rien à l'œil. */
function poids(o: number): string {
	if (o < 1024) return `${o} o`;
	if (o < 1024 * 1024) return `${(o / 1024).toFixed(0)} Kio`;
	return `${(o / (1024 * 1024)).toFixed(1)} Mio`;
}

export async function generateMetadata({
	params,
}: {
	params: Promise<{ path?: string[] }>;
}): Promise<Metadata> {
	const { path } = await params;
	const [famille, code] = path ?? [];
	const def = famille ? modelFamily(famille) : undefined;
	const titre = code ? `${code} — ${def?.label ?? ""}` : (def?.label ?? "Tous les modèles");
	return {
		alternates: { canonical: `/modeles${path?.length ? `/${path.join("/")}` : ""}` },
		description: `Modèles 3D d'Inazuma Eleven: Victory Road assemblés live depuis les fichiers du jeu — ${titre}.`,
		title: `Modèles 3D — ${titre} | Azalée`,
	};
}

export default async function ModelesPage({
	params,
	searchParams,
}: {
	params: Promise<{ path?: string[] }>;
	searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
	const [{ path }, sp] = await Promise.all([params, searchParams]);
	const segments = (path ?? []).filter((s) => s !== ".." && s !== ".");
	const [familleId, code] = segments;
	const def = familleId ? modelFamily(familleId) : undefined;

	if (familleId && !def) {
		return (
			<div className="w-full space-y-5">
				<EnTete titre="Modèles 3D" />
				<MediaEmpty>Famille de modèles inconnue : « {familleId} ».</MediaEmpty>
			</div>
		);
	}

	if (def && code) return <Fiche famille={def.id} code={code} />;
	if (def) {
		const pageParam = typeof sp.page === "string" ? Number.parseInt(sp.page, 10) : 1;
		const page = Number.isFinite(pageParam) && pageParam > 0 ? pageParam : 1;
		const q = typeof sp.q === "string" ? sp.q : "";
		const view: ModelView = sp.view === "list" ? "list" : "grid";
		return <Grille famille={def.id} page={page} q={q} view={view} />;
	}
	return <Familles />;
}

/** En-tête commun aux trois vues. */
function EnTete({ titre, description }: { titre: string; description?: string }) {
	return (
		<MediaHeader
			title={titre}
			active="/modeles"
			description={
				description ??
				"Tous les modèles 3D du jeu, assemblés à la volée depuis les fichiers d'origine (maillage, squelette et textures)."
			}
		/>
	);
}

/** Vue d'accueil : les familles et leur volume. */
async function Familles() {
	const comptes = await Promise.all(
		MODEL_FAMILIES.map(async (f) => (await modelesDe(f.id)).length),
	);
	const total = comptes.reduce((a, b) => a + b, 0);

	return (
		<div className="w-full space-y-5">
			<EnTete titre="Modèles 3D" />
			<MediaCount left={`${total.toLocaleString("fr")} modèles au total`} />
			<ul className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
				{MODEL_FAMILIES.map((f, i) => (
					<li key={f.id}>
						<Link
							href={`/modeles/${f.id}`}
							className="block rounded-2xl border border-outline-variant bg-surface-container-low p-4 transition hover:border-primary/50 hover:bg-surface-container"
						>
							<span className="flex items-baseline justify-between gap-2">
								<span className="font-medium text-on-surface">{f.label}</span>
								<span className="tabular-nums text-sm text-on-surface-variant">
									{(comptes[i] ?? 0).toLocaleString("fr")}
								</span>
							</span>
							<span className="mt-0.5 block text-xs text-on-surface-variant">{f.hint}</span>
						</Link>
					</li>
				))}
			</ul>
		</div>
	);
}

/**
 * Une carte de la grille : le modèle 3D **réellement rendu**, pas une pastille de texte.
 *
 * Le `<model-viewer>` n'est monté que lorsque la carte entre dans le champ et démonté quand elle
 * en sort — sans quoi une page de 48 cartes ouvrirait 48 contextes WebGL, là où le navigateur en
 * tolère une quinzaine.
 */
function Carte({ famille, m }: { famille: ModelFamily; m: ModelEntry }) {
	const titre = m.name ?? m.code;
	const glb = modelGlbUrl(famille, m.code);
	const tex = textureDe(famille, m.code);
	return (
		<li className="group overflow-hidden rounded-2xl border border-outline-variant/40 bg-surface-container-low transition hover:border-primary/50">
			<Link href={modelHref(famille, m.code)} className="block">
				<span className="relative block aspect-[3/4] bg-surface-container-high">
					<InlineModelViewer glbUrl={glb} name={titre} />
					{tex && (
						// eslint-disable-next-line @next/next/no-img-element
						<img
							src={`${tex}?w=160&format=webp`}
							alt={`Texture de ${titre}`}
							loading="lazy"
							decoding="async"
							className="absolute bottom-2 right-2 size-11 rounded-lg border border-outline-variant/30 bg-surface-container object-cover shadow [image-rendering:pixelated]"
						/>
					)}
				</span>
			</Link>
			<div className="flex items-center gap-2 border-t border-outline-variant/20 px-2.5 py-2">
				<Link href={modelHref(famille, m.code)} className="min-w-0 flex-1">
					<span className="block truncate text-sm text-on-surface" title={titre}>
						{titre}
					</span>
					<span className="block truncate text-[11px] text-on-surface-variant">
						{m.name && <span className="font-mono">{m.code}</span>}
						{m.name && m.files ? " · " : null}
						{m.files ? `${m.files} fichier${m.files > 1 ? "s" : ""}` : null}
					</span>
				</Link>
				<a
					href={glb}
					download={`${m.code}.glb`}
					title="Télécharger le modèle (GLB)"
					className="shrink-0 rounded-full bg-surface-container px-2.5 py-1 text-[11px] font-medium uppercase text-on-surface-variant transition hover:bg-surface-container-high hover:text-on-surface"
				>
					glb
				</a>
				{tex && (
					<a
						href={tex}
						download={`${m.code}.png`}
						title="Télécharger la texture (PNG)"
						className="shrink-0 rounded-full bg-surface-container px-2.5 py-1 text-[11px] font-medium uppercase text-on-surface-variant transition hover:bg-surface-container-high hover:text-on-surface"
					>
						png
					</a>
				)}
			</div>
		</li>
	);
}

/** Une ligne de la vue liste : dense, lisible, sans coût 3D. */
function Ligne({ famille, m }: { famille: ModelFamily; m: ModelEntry }) {
	const titre = m.name ?? m.code;
	const tex = textureDe(famille, m.code);
	return (
		<li className="flex items-center gap-3 border-b border-outline-variant/20 px-3 py-2 last:border-0 hover:bg-surface-container-low">
			<Link href={modelHref(famille, m.code)} className="min-w-0 flex-1">
				<span className="block truncate text-sm text-on-surface" title={titre}>
					{titre}
				</span>
				{m.name && (
					<span className="block truncate font-mono text-[11px] text-on-surface-variant">
						{m.code}
					</span>
				)}
			</Link>
			{m.files ? (
				<span className="shrink-0 tabular-nums text-xs text-on-surface-variant">
					{m.files} fichier{m.files > 1 ? "s" : ""}
				</span>
			) : null}
			<a
				href={modelGlbUrl(famille, m.code)}
				download={`${m.code}.glb`}
				title="Télécharger le modèle (GLB)"
				className="shrink-0 rounded-full bg-surface-container px-2.5 py-1 text-[11px] font-medium uppercase text-on-surface-variant transition hover:bg-surface-container-high hover:text-on-surface"
			>
				glb
			</a>
			{tex && (
				<a
					href={tex}
					download={`${m.code}.png`}
					title="Télécharger la texture (PNG)"
					className="shrink-0 rounded-full bg-surface-container px-2.5 py-1 text-[11px] font-medium uppercase text-on-surface-variant transition hover:bg-surface-container-high hover:text-on-surface"
				>
					png
				</a>
			)}
		</li>
	);
}

/** Galerie paginée d'une famille — grille 3D ou liste, filtrable. */
async function Grille({
	famille,
	page,
	q,
	view,
}: {
	famille: ModelFamily;
	page: number;
	q: string;
	view: ModelView;
}) {
	const def = modelFamily(famille);
	const tous = await modelesDe(famille);
	const trouves = filtrer(tous, q);
	const parPage = PAR_PAGE[view];
	const pages = Math.max(1, Math.ceil(trouves.length / parPage));
	// Une recherche peut rendre la page courante hors bornes ; on la ramène dans le domaine
	// plutôt que d'afficher une tranche vide sans rien dire.
	const p = Math.min(page, pages);
	const tranche = trouves.slice((p - 1) * parPage, p * parPage);

	/** Conserve recherche et disposition en tournant les pages. */
	const hrefPage = (n: number) => {
		const sp = new URLSearchParams();
		if (q) sp.set("q", q);
		if (view === "list") sp.set("view", "list");
		if (n > 1) sp.set("page", String(n));
		const s = sp.toString();
		return `/modeles/${famille}${s ? `?${s}` : ""}`;
	};

	return (
		<div className="w-full space-y-5">
			<EnTete titre={`Modèles 3D — ${def?.label ?? famille}`} description={def?.hint} />

			<MediaBack href="/modeles" label="Toutes les familles" />

			<nav className="flex flex-wrap gap-2">
				{MODEL_FAMILIES.filter((f) => f.id !== famille).map((f) => (
					<Link
						key={f.id}
						href={`/modeles/${f.id}`}
						className="rounded-full border border-outline-variant bg-surface-container-low px-3 py-1.5 text-sm text-on-surface-variant transition hover:border-primary/50 hover:text-on-surface"
					>
						{f.label}
					</Link>
				))}
			</nav>

			<ModelToolbar base={`/modeles/${famille}`} q={q} view={view} />

			{tous.length === 0 ? (
				<MediaEmpty>
					Aucun modèle listé pour cette famille — la source est momentanément injoignable.
				</MediaEmpty>
			) : trouves.length === 0 ? (
				<MediaEmpty>
					Aucun modèle ne correspond à « {q} » parmi les {tous.length.toLocaleString("fr")} de
					cette famille.
				</MediaEmpty>
			) : (
				<>
					<MediaCount
						left={
							q
								? `${trouves.length.toLocaleString("fr")} sur ${tous.length.toLocaleString("fr")} modèles`
								: `${tous.length.toLocaleString("fr")} modèle${tous.length > 1 ? "s" : ""}`
						}
						right={pages > 1 ? `Page ${p} sur ${pages}` : undefined}
					/>

					{view === "grid" ? (
						<ul className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-6">
							{tranche.map((m) => (
								<Carte key={m.code} famille={famille} m={m} />
							))}
						</ul>
					) : (
						<ul className="overflow-hidden rounded-2xl border border-outline-variant bg-surface-container-low">
							{tranche.map((m) => (
								<Ligne key={m.code} famille={famille} m={m} />
							))}
						</ul>
					)}

					{pages > 1 && (
						<nav className="flex items-center justify-center gap-3 pt-2 text-sm">
							{p > 1 && (
								<Link
									href={hrefPage(p - 1)}
									className="rounded-full bg-surface-container px-4 py-2 text-on-surface transition hover:bg-surface-container-high"
								>
									Précédent
								</Link>
							)}
							<span className="text-on-surface-variant">
								{p} / {pages}
							</span>
							{p < pages && (
								<Link
									href={hrefPage(p + 1)}
									className="rounded-full bg-surface-container px-4 py-2 text-on-surface transition hover:bg-surface-container-high"
								>
									Suivant
								</Link>
							)}
						</nav>
					)}
				</>
			)}
		</div>
	);
}

/** Fiche d'un modèle : le 3D assemblé, puis ses textures. */
async function Fiche({ famille, code }: { famille: ModelFamily; code: string }) {
	const def = modelFamily(famille);
	const texDir = modelTextureDir(famille, code);
	// Dossier des pièces du modèle. `null` pour un personnage, dont le corps, le visage et
	// l'uniforme vivent dans trois arbres distincts que `/model-full` réunit.
	const srcDir = def?.vfsDir ? `${def.vfsDir}/${code}` : null;
	const [tous, listeTex, listeSrc] = await Promise.all([
		modelesDe(famille),
		texDir ? lsOrNull(texDir, 200) : Promise.resolve(null),
		srcDir ? lsOrNull(srcDir, 100) : Promise.resolve(null),
	]);
	const entree = tous.find((m) => m.code === code);
	const g4tx = (listeTex?.files ?? []).filter((f) => f.ext === "g4tx");
	const pieces = listeSrc?.files ?? [];
	const octets = pieces.reduce((s, f) => s + (f.size ?? 0), 0);

	return (
		<div className="w-full space-y-5">
			<EnTete
				titre={entree?.name ?? code}
				description={`${def?.label ?? famille} · code ${code} — modèle assemblé à la volée depuis les fichiers du jeu.`}
			/>

			<MediaBack href={`/modeles/${famille}`} label={`Retour — ${def?.label ?? famille}`} />

			<nav className="flex flex-wrap items-center gap-1 text-sm text-on-surface-variant">
				<Link href="/modeles" className="hover:text-primary">
					Modèles
				</Link>
				<span aria-hidden>›</span>
				<Link href={`/modeles/${famille}`} className="hover:text-primary">
					{def?.label ?? famille}
				</Link>
				<span aria-hidden>›</span>
				<span className="font-mono text-on-surface">{code}</span>
				{entree?.slug && (
					<>
						<span aria-hidden className="mx-1">
							·
						</span>
						<Link href={`/chara/${entree.slug}`} className="text-primary hover:underline">
							Fiche du personnage
						</Link>
					</>
				)}
			</nav>

			{/* `relative` + une hauteur : le viewer se pose en `absolute inset-0`, un conteneur
			    sans position ni hauteur le réduisait à zéro pixel — cadre visible, 3D invisible. */}
			<div className="relative h-[min(70vh,560px)] overflow-hidden rounded-2xl border border-outline-variant bg-surface-container-low">
				<InlineModelViewer glbUrl={modelGlbUrl(famille, code)} name={entree?.name ?? code} autoRotate={false} />
			</div>

			<section className="rounded-2xl border border-outline-variant bg-surface-container-low p-4">
				<h2 className="mb-2 text-sm font-medium text-on-surface">Télécharger</h2>
				<a
					href={modelGlbUrl(famille, code)}
					download={`${code}.glb`}
					className="inline-flex items-center gap-1.5 rounded-full border border-outline-variant bg-surface-container px-3 py-1.5 text-xs text-on-surface transition hover:border-primary/50 hover:bg-surface-container-high"
				>
					<span className="font-medium uppercase">glb</span>
				</a>
				<p className="mt-2 text-xs text-on-surface-variant">
					glTF binaire — maillage, squelette et textures dans un seul fichier.
					{srcDir && (
						<>
							{" "}
							<Link href={cpkExplorerHref(srcDir)} className="text-primary hover:underline">
								Ouvrir le dossier du modèle dans l'explorateur CPK
							</Link>
						</>
					)}
					{texDir && (
						<>
							{" · "}
							<Link href={cpkExplorerHref(texDir)} className="text-primary hover:underline">
								Ouvrir le dossier des textures
							</Link>
						</>
					)}
				</p>
			</section>

			{/* Ce dont le modèle est FAIT — lu sur les fichiers eux-mêmes, jamais supposé. Un code
			    seul ne dit rien ; le dossier, lui, dit s'il y a un squelette, des animations, des
			    textures, et ce que tout cela pèse. */}
			{pieces.length > 0 && (
				<section className="rounded-2xl border border-outline-variant bg-surface-container-low p-4">
					<h2 className="mb-1 text-sm font-medium text-on-surface">Fichiers d'origine</h2>
					<p className="mb-3 text-xs text-on-surface-variant">
						{pieces.length} fichier{pieces.length > 1 ? "s" : ""} · {poids(octets)} ·{" "}
						<span className="font-mono">{srcDir}</span>
						{listeSrc?.role && <> — {listeSrc.role.role}</>}
					</p>
					<ul className="divide-y divide-outline-variant/20">
						{pieces.map((f) => (
							<li key={f.path} className="flex items-center gap-3 py-1.5 text-xs">
								<span className="w-16 shrink-0 rounded-full bg-surface-container px-2 py-0.5 text-center font-mono text-[10px] uppercase text-on-surface-variant">
									{f.ext}
								</span>
								<span className="min-w-0 flex-1 truncate text-on-surface" title={f.name}>
									{f.name}
								</span>
								<span className="hidden shrink-0 text-on-surface-variant sm:block">
									{ROLE_EXT[f.ext] ?? "—"}
								</span>
								<span className="w-20 shrink-0 text-right tabular-nums text-on-surface-variant">
									{poids(f.size)}
								</span>
							</li>
						))}
					</ul>
				</section>
			)}

			{g4tx.length > 0 && (
				<>
					<MediaCount
						left={`${g4tx.length.toLocaleString("fr")} fichier${
							g4tx.length > 1 ? "s" : ""
						} de texture`}
					/>
					<ul className="grid grid-cols-2 gap-3 sm:grid-cols-4 lg:grid-cols-6">
						{g4tx.map((t) => (
							<li key={t.path}>
								<Link
									href={`/textures/${t.path.replace(/^data\/dx11\//, "")}`}
									className="group block overflow-hidden rounded-xl border border-outline-variant/30 bg-surface-container transition hover:border-primary/50"
								>
									<span className="flex aspect-square items-center justify-center bg-surface-container-high">
										{/* eslint-disable-next-line @next/next/no-img-element */}
										<img
											src={cpkThumbUrl(t.path, t.ext, 400) ?? ""}
											alt={t.name}
											loading="lazy"
											decoding="async"
											className="max-h-full max-w-full object-contain"
										/>
									</span>
									<span
										className="block truncate px-2 py-1.5 font-mono text-[11px] text-on-surface-variant group-hover:text-on-surface"
										title={t.name}
									>
										{t.name.replace(/\.g4tx$/i, "")}
									</span>
								</Link>
							</li>
						))}
					</ul>
				</>
			)}
		</div>
	);
}
