/**
 * Galerie de textures — les 53 126 fichiers `.g4tx` du jeu, décodés en PNG à la volée.
 *
 * La galerie `/gallery` sert 360 illustrations curées (table `inagle_gallery`) : c'est une
 * sélection éditoriale, pas le corpus. Cette page-ci est le corpus, parcouru par dossier sur le
 * VFS **live** (`/vfs/ls`) : `dx11/menu` en porte 40 158 à lui seul, dont 19 245 d'icônes.
 *
 * Chaque vignette est un WebP redimensionné par le CDN (`?w=…&format=webp`, ~10 ko) et non le
 * PNG plein cadre (~60 ko et plus) : à 120 images par page, la différence est celle entre une
 * page qui s'affiche et une page qui ne s'affiche pas.
 *
 * Server Component : aucune donnée n'est bundlée, tout vient du pont VFS au rendu.
 */
import type { Metadata } from "next";
import Link from "next/link";
import { lsOrNull, texContainer, texUrl } from "@rosegriffon/azalee/cpk/live";
import { MediaBack, MediaCount, MediaEmpty, MediaHeader } from "@/components/wiki/MediaShell";
import { itemNames } from "@/lib/cpk/media-names";
import { MediaDownload } from "@/components/wiki/MediaDownload";
import { cpkThumbUrl, segLabel } from "@rosegriffon/azalee/cpk/shared";

export const dynamic = "force-dynamic";
export const runtime = "nodejs";

/** Racine du sous-arbre exploré : les `.g4tx` vivent tous sous `data/dx11/`. */
const RACINE = "data/dx11";
/** Textures par page — borne le nombre de requêtes de vignettes qu'un rendu déclenche. */
const PAR_PAGE = 120;

export async function generateMetadata({
	params,
}: {
	params: Promise<{ path?: string[] }>;
}): Promise<Metadata> {
	const { path } = await params;
	const segments = path ?? [];
	const titre = segments.length > 0 ? segments.map(segLabel).join(" › ") : "Toutes les textures";
	return {
		alternates: { canonical: `/textures${segments.length ? `/${segments.join("/")}` : ""}` },
		description: `Textures d'Inazuma Eleven: Victory Road décodées depuis les fichiers du jeu — ${titre}.`,
		title: `Textures — ${titre} | Azalée`,
	};
}

export default async function TexturesPage({
	params,
	searchParams,
}: {
	params: Promise<{ path?: string[] }>;
	searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
	const [{ path }, sp] = await Promise.all([params, searchParams]);
	const segments = (path ?? []).filter((s) => s !== ".." && s !== ".");
	const dir = segments.length > 0 ? `${RACINE}/${segments.join("/")}` : RACINE;

	// Dernier segment en `.g4tx` : ce n'est pas un dossier mais un CONTENEUR, et il faut
	// l'ouvrir. Un `.g4tx` porte jusqu'à 203 textures nommées ; la vue dossier n'en montrerait
	// qu'une seule vignette, laissant tout le reste inatteignable.
	const dernier = segments.at(-1) ?? "";
	if (dernier.toLowerCase().endsWith(".g4tx")) {
		const rel = `dx11/${segments.join("/")}`.replace(/\.g4tx$/i, "");
		return <VueConteneur rel={rel} segments={segments} />;
	}

	const pageParam = typeof sp.page === "string" ? Number.parseInt(sp.page, 10) : 1;
	const page = Number.isFinite(pageParam) && pageParam > 0 ? pageParam : 1;
	const offset = (page - 1) * PAR_PAGE;

	const listing = await lsOrNull(dir, PAR_PAGE, offset);

	if (!listing) {
		return (
			<div className="w-full space-y-5">
				<MediaHeader
					title="Textures"
					active="/textures"
					description="Le corpus complet des textures du jeu, décodées en PNG à la volée et parcourues par dossier."
				/>
				<MediaEmpty>
					Le VFS du jeu est momentanément injoignable — les textures ne peuvent pas être listées.
				</MediaEmpty>
			</div>
		);
	}

	const textures = listing.files.filter((f) => f.ext === "g4tx");
	const pages = Math.max(1, Math.ceil(listing.fileTotal / PAR_PAGE));
	const base = `/textures${segments.length ? `/${segments.join("/")}` : ""}`;

	return (
		<div className="w-full space-y-5">
			<MediaHeader
				title={segments.length > 0 ? segLabel(segments.at(-1) ?? "Textures") : "Textures"}
				active="/textures"
				description="Le corpus complet des textures du jeu, décodées en PNG à la volée et parcourues par dossier."
			>
				<nav
					aria-label="Fil d'Ariane"
					className="flex flex-wrap items-center gap-1 pt-1 text-sm text-on-surface-variant"
				>
					<Link href="/textures" className="hover:text-primary">
						dx11
					</Link>
					{segments.map((seg, i) => (
						<span key={`${seg}-${i}`} className="flex items-center gap-1">
							<span aria-hidden>›</span>
							<Link
								href={`/textures/${segments.slice(0, i + 1).join("/")}`}
								className="hover:text-primary"
							>
								{segLabel(seg)}
							</Link>
						</span>
					))}
				</nav>
				{listing.role && (
					<p className="text-xs text-on-surface-variant">
						{listing.role.role} — {listing.role.status}
					</p>
				)}
			</MediaHeader>

			{segments.length > 0 && (
				<MediaBack
					href={`/textures${segments.length > 1 ? `/${segments.slice(0, -1).join("/")}` : ""}`}
					label={
						segments.length > 1
							? `Retour à ${segLabel(segments.at(-2) ?? "")}`
							: "Retour aux textures"
					}
				/>
			)}

			{listing.dirs.length > 0 && (
				<ul className="flex flex-wrap gap-2">
					{listing.dirs.map((d) => (
						<li key={d.name}>
							<Link
								href={`${base}/${d.name}`}
								className="flex items-center gap-2 rounded-full border border-outline-variant bg-surface-container-low px-3 py-1.5 text-sm text-on-surface transition hover:border-primary/50 hover:bg-surface-container"
							>
								<span>{segLabel(d.name)}</span>
								<span className="text-xs text-on-surface-variant">
									{d.count.toLocaleString("fr")}
								</span>
							</Link>
						</li>
					))}
				</ul>
			)}

			{listing.fileTotal > 0 && (
				<MediaCount
					left={`${textures.length.toLocaleString("fr")} texture${
						textures.length > 1 ? "s" : ""
					} sur ${listing.fileTotal.toLocaleString("fr")} fichier${listing.fileTotal > 1 ? "s" : ""}`}
					right={`Page ${page} sur ${pages}`}
				/>
			)}

			{textures.length > 0 ? (
				<ul className="grid grid-cols-2 gap-3 sm:grid-cols-4 lg:grid-cols-6">
					{textures.map((t) => (
						<li key={t.path}>
							{/* Le lien mène au CONTENEUR, pas au PNG : le fichier porte souvent des
							    dizaines de textures nommées, que seule la vue conteneur expose. */}
							<Link
								href={`${base}/${t.name}`}
								className="group block overflow-hidden rounded-xl border border-outline-variant/30 bg-surface-container transition hover:border-primary/50"
							>
								<span className="flex aspect-square items-center justify-center bg-surface-container-high bg-[linear-gradient(45deg,rgba(128,128,128,.12)_25%,transparent_25%,transparent_75%,rgba(128,128,128,.12)_75%),linear-gradient(45deg,rgba(128,128,128,.12)_25%,transparent_25%,transparent_75%,rgba(128,128,128,.12)_75%)] bg-[length:16px_16px] bg-[position:0_0,8px_8px]">
									{/* Vignette CDN : `next/image` est inutile ici, le CDN redimensionne déjà. */}
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
			) : (
				listing.dirs.length === 0 && (
					<p className="py-16 text-center text-sm italic text-on-surface-variant">
						Aucune texture dans ce dossier.
					</p>
				)
			)}

			{pages > 1 && (
				<nav className="flex items-center justify-center gap-3 pt-2 text-sm">
					{page > 1 && (
						<Link
							href={`${base}?page=${page - 1}`}
							className="rounded-full bg-surface-container px-4 py-2 text-on-surface transition hover:bg-surface-container-high"
						>
							Précédent
						</Link>
					)}
					<span className="text-on-surface-variant">
						{page} / {pages}
					</span>
					{page < pages && (
						<Link
							href={`${base}?page=${page + 1}`}
							className="rounded-full bg-surface-container px-4 py-2 text-on-surface transition hover:bg-surface-container-high"
						>
							Suivant
						</Link>
					)}
				</nav>
			)}
		</div>
	);
}

/**
 * Vue d'un conteneur G4TX : toutes ses textures nommées, chacune adressable.
 *
 * C'est ce que la vue dossier ne pouvait pas montrer. Un `.g4tx` porte en moyenne 82,8 textures
 * dans `200_icon/02_icon_item` (mesuré sur 10 conteneurs, jusqu'à 203 dans un seul fichier) :
 * s'arrêter au fichier, c'était n'en exposer qu'une sur quatre-vingts.
 */
async function VueConteneur({ rel, segments }: { rel: string; segments: string[] }) {
	const [conteneur, objets] = await Promise.all([texContainer(rel), itemNames()]);
	const nom = segments.at(-1) ?? "";
	const parent = `/textures/${segments.slice(0, -1).join("/")}`;

	// Le nom de texture EST le code interne de l'objet dans les conteneurs d'icônes : la
	// jointure rend une grille d'objets là où il n'y avait qu'une grille de codes.
	const libelle = (n: string) => objets.get(n.toLowerCase()) ?? null;
	const nommees = conteneur?.textures.filter((t) => libelle(t.name)).length ?? 0;

	return (
		<div className="w-full space-y-5">
			<MediaHeader
				title={nom.replace(/\.g4tx$/i, "")}
				active="/textures"
				description="Conteneur de textures : chaque image ci-dessous est un payload nommé du même fichier."
			>
				<nav
					aria-label="Fil d'Ariane"
					className="flex flex-wrap items-center gap-1 pt-1 text-sm text-on-surface-variant"
				>
					<Link href="/textures" className="hover:text-primary">
						dx11
					</Link>
					{segments.slice(0, -1).map((seg, i) => (
						<span key={`${seg}-${i}`} className="flex items-center gap-1">
							<span aria-hidden>›</span>
							<Link
								href={`/textures/${segments.slice(0, i + 1).join("/")}`}
								className="hover:text-primary"
							>
								{segLabel(seg)}
							</Link>
						</span>
					))}
					<span aria-hidden>›</span>
					<span className="font-mono text-on-surface">{nom}</span>
				</nav>
			</MediaHeader>

			<MediaBack href={parent} label={`Retour à ${segLabel(segments.at(-2) ?? "")}`} />

			<MediaDownload path={`data/dx11/${segments.join("/")}`} />

			{!conteneur || conteneur.count === 0 ? (
				<MediaEmpty>Ce conteneur n'expose aucune texture décodable.</MediaEmpty>
			) : (
				<>
					<MediaCount
						left={`${conteneur.count.toLocaleString("fr")} texture${
							conteneur.count > 1 ? "s" : ""
						} dans ce fichier`}
						right={
							nommees > 0
								? `${nommees.toLocaleString("fr")} rattachée${nommees > 1 ? "s" : ""} à un objet`
								: undefined
						}
					/>
					<ul className="grid grid-cols-2 gap-3 sm:grid-cols-4 lg:grid-cols-6">
						{conteneur.textures.map((t) => (
							<li key={`${t.id}-${t.name}`}>
								<a
									href={texUrl(t.path)}
									target="_blank"
									rel="noreferrer"
									className="group block overflow-hidden rounded-xl border border-outline-variant/30 bg-surface-container transition hover:border-primary/50"
								>
									<span className="flex aspect-square items-center justify-center bg-surface-container-high bg-[linear-gradient(45deg,rgba(128,128,128,.12)_25%,transparent_25%,transparent_75%,rgba(128,128,128,.12)_75%),linear-gradient(45deg,rgba(128,128,128,.12)_25%,transparent_25%,transparent_75%,rgba(128,128,128,.12)_75%)] bg-[length:16px_16px] bg-[position:0_0,8px_8px]">
										{/* eslint-disable-next-line @next/next/no-img-element */}
										<img
											src={texUrl(t.path, 400)}
											alt={t.name}
											loading="lazy"
											decoding="async"
											className="max-h-full max-w-full object-contain"
										/>
									</span>
									<span className="block px-2 py-1.5">
										{/* Le nom de l'objet prime ; le code technique passe dessous. */}
										<span
											className={`block truncate text-[11px] group-hover:text-on-surface ${
												libelle(t.name)
													? "font-medium text-on-surface"
													: "font-mono text-on-surface-variant"
											}`}
											title={libelle(t.name) ?? t.name}
										>
											{libelle(t.name) ?? t.name}
										</span>
										<span className="block truncate text-[10px] text-on-surface-variant/70">
											{libelle(t.name) && <span className="font-mono">{t.name} · </span>}
											{t.width}×{t.height}
											{t.regions > 0 && ` · ${t.regions} région${t.regions > 1 ? "s" : ""}`}
										</span>
									</span>
								</a>
							</li>
						))}
					</ul>
				</>
			)}
		</div>
	);
}
