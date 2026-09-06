/**
 * La vue « Modèles » — la couche 3D du dépôt, vue depuis le navigateur.
 *
 * ## Pourquoi cette vue ne ressemble pas aux trois autres catalogues
 *
 * `Catalogue.tsx` liste un filtre d'extensions sur le VFS : `modeles` y retenait `.g4md`,
 * `.g4mg`, `.g4sk`, `.g4mt`, `.g4pk`. Ce sont des **pièces**, pas des modèles — un `.g4mg` seul
 * n'est qu'un tampon de géométrie, il n'a ni texture, ni squelette, ni recette d'assemblage, et
 * la grille n'en montrait donc qu'un nom de fichier et une taille. On listait 143 000 fichiers
 * dont aucun ne pouvait s'afficher.
 *
 * Ici, l'unité est le **code de modèle** — ce que le jeu assemble et ce qu'on peut regarder.
 * Le serveur en publie 6 191, répartis en six familles (`/api/v1/3d`), et il sait rendre
 * chacun sous deux formes : un GLB assemblé et une image.
 *
 * ## Deux chemins de rendu, et pourquoi les deux
 *
 * | | Vignette de la grille | Viewport |
 * |---|---|---|
 * | qui rend | `nie-render3d`, **côté serveur** (rastériseur CPU à z-buffer) | WebGPU, côté navigateur |
 * | ce que reçoit le navigateur | un PNG de 12 ko | le GLB (jusqu'à quelques Mo) |
 * | coût pour 24 cartes | 24 `<img>` | 24 périphériques WebGPU — inacceptable |
 *
 * Une grille ne doit pas monter un périphérique GPU par carte : les navigateurs plafonnent le
 * nombre de contextes de rendu et détruisent silencieusement les plus anciens, ce qui donne
 * une grille dont la moitié des cases redeviennent noires en défilant. Le serveur, lui, rend la
 * même image une fois puis la sert depuis son cache en 0,6 ms (mesuré : 182 ms au premier
 * rendu, 0,6 ms ensuite). Le viewport interactif n'est monté que pour le modèle qu'on ouvre —
 * **un seul contexte à la fois**.
 *
 * ## Le viewport reproduit les conventions du rastériseur, il ne les réinvente pas
 *
 * Focale, distance de caméra et inclinaison sont **lues sur `/api/v1/3d`**, qui les publie
 * depuis `nie_render3d::render::{FOCALE, DISTANCE_CAMERA, TILT}`. Le nuanceur de sommets refait
 * exactement le calcul de `render.rs` : normalisation par la sphère englobante, rotation autour
 * de Y, inclinaison autour de X, projection `focale · r / (distance − r.z)`. C'est ce qui fait
 * que le viewport cadre la vignette qu'il remplace, au lieu d'en proposer une autre.
 *
 * Une seule chose change en passant à WebGPU, et elle est invisible tant qu'on ne la traite pas :
 * la profondeur en coordonnées normalisées y vaut `[0, 1]`, pas `[-1, 1]` comme en OpenGL. La
 * matrice de projection porte donc `a = LOIN / (LOIN − PROCHE)` au lieu de
 * `(LOIN + PROCHE) / (LOIN − PROCHE)` ; se tromper ici n'écrit aucune valeur fausse à l'écran,
 * cela écrête simplement la moitié du modèle.
 *
 * Trois écarts sont **assumés**, et les deux premiers sont ceux que `nie-render3d` documente déjà
 * entre son chemin CPU et son chemin GPU : l'ombrage est lissé (normale de sommet interpolée) là
 * où le CPU l'applique par face, et les faces arrière sont conservées — les maillages du jeu ont
 * une orientation incohérente, et les écarter fait disparaître des mèches de cheveux. Le
 * troisième tient à WebGPU : il n'a pas de `generateMipmap`, les textures sont donc chargées à un
 * seul niveau et filtrées en linéaire, ce qui crénelle un peu plus qu'en WebGL de loin.
 *
 * ## Ce qui n'est pas fait ici
 *
 * Les transformations de nœuds et le *skinning* sont ignorés, comme dans `nie_render3d::glb` :
 * les positions du GLB sont déjà en espace monde (`nie_formats::assemble::to_glb_embedded`).
 * Un modèle s'affiche donc dans sa pose de liaison, jamais animé.
 */
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { accorde, Note, TitreVue } from "./Ecran";

/** Taille de page. 24 cartes : une grille pleine sans imposer 60 rendus à froid au serveur. */
const PAR_PAGE = 24;

/** Poids maximal d'un GLB chargé dans le viewport. Au-delà, on garde l'aperçu serveur. */
const GLB_OCTETS_MAX = 32 * 1024 * 1024;

/** Une famille, telle que `/api/v1/3d` la décrit. */
interface Famille {
	segment: string;
	libelle: string;
	source: string;
	dossier: string | null;
	total: number | null;
	verifie: boolean;
}

/** Les conventions de caméra du rastériseur, publiées par le serveur. */
interface Moteur {
	focale: number;
	distance: number;
	tilt: number;
	taille_defaut: number;
	taille_max: number;
	simultanes: number;
}

/** Corps de `/api/v1/3d`. */
interface Capacites3d {
	amont: string;
	vfs_pret: boolean;
	miroir_present: boolean;
	moteur: Moteur;
	familles: Famille[];
}

/** Un modèle du catalogue. */
interface Modele {
	code: string;
	famille: string;
	nom: string | null;
	fichiers: number | null;
	glb: string;
	apercu: string;
}

/** Une page du catalogue. */
interface PageModeles {
	elements: Modele[];
	page: number;
	per_page: number;
	total: number;
	pages: number;
}

/** Corps de `/api/v1/3d/modeles/{famille}/{code}/analyse`. */
interface Analyse {
	glb_octets: number;
	primitives: number;
	primitives_sans_texture: number;
	sommets: number;
	triangles: number;
	textures: { largeur: number; hauteur: number }[];
	texels: number;
	boite: { min: number[]; max: number[]; centre: number[]; rayon: number };
}

/** Lit une réponse JSON, en distinguant l'abandon volontaire d'un vrai échec. */
async function json<T>(url: string, signal: AbortSignal): Promise<T> {
	const r = await fetch(url, { signal });
	if (!r.ok) throw new Error(`${url} → ${r.status}`);
	return (await r.json()) as T;
}

/** Formate un nombre à la française, sans dépendance. */
function n(x: number): string {
	return x.toLocaleString("fr");
}

/** Formate une taille en octets. */
function octets(x: number): string {
	if (x < 1024) return `${x} o`;
	if (x < 1024 * 1024) return `${(x / 1024).toFixed(1)} ko`;
	return `${(x / (1024 * 1024)).toFixed(1)} Mo`;
}

export function Modeles3D() {
	const [capacites, setCapacites] = useState<Capacites3d | null>(null);
	const [capacitesKo, setCapacitesKo] = useState(false);
	const [famille, setFamille] = useState("perso");
	const [page, setPage] = useState(1);
	const [saisie, setSaisie] = useState("");
	const [filtre, setFiltre] = useState("");
	const [liste, setListe] = useState<PageModeles | null>(null);
	const [listeKo, setListeKo] = useState(false);
	const [ouvert, setOuvert] = useState<Modele | null>(null);

	// Les capacités décrivent ce que la machine sait faire : familles, totaux, conventions de
	// caméra. Sans elles on n'affiche pas une liste vide, on dit que le service ne répond pas.
	useEffect(() => {
		const ac = new AbortController();
		json<Capacites3d>("/api/v1/3d", ac.signal)
			.then(setCapacites)
			.catch(() => {
				if (!ac.signal.aborted) setCapacitesKo(true);
			});
		return () => ac.abort();
	}, []);

	// Changer de famille ou de recherche ramène à la page 1 : garder la page 200 en passant de
	// 5 490 personnages à 2 animaux afficherait un vide que rien n'expliquerait.
	useEffect(() => {
		setPage(1);
	}, []);

	useEffect(() => {
		const ac = new AbortController();
		setListe(null);
		setListeKo(false);
		const url = `/api/v1/3d/modeles?famille=${encodeURIComponent(famille)}&page=${page}&per_page=${PAR_PAGE}${
			filtre ? `&q=${encodeURIComponent(filtre)}` : ""
		}`;
		json<PageModeles>(url, ac.signal)
			.then(setListe)
			.catch(() => {
				if (!ac.signal.aborted) setListeKo(true);
			});
		return () => ac.abort();
	}, [famille, page, filtre]);

	const familles = capacites?.familles ?? [];
	const familleCourante = familles.find((f) => f.segment === famille);

	if (capacitesKo) {
		return (
			<Note ton="alerte">
				La couche 3D ne répond pas. Le service de décodage est peut-être arrêté ; réessayez dans un
				instant.
			</Note>
		);
	}
	if (!capacites) return <Note>Chargement…</Note>;

	return (
		<section>
			<TitreVue appoint={liste ? accorde(liste.total, "modèle") : undefined}>Modèles</TitreVue>

			<p style={{ margin: "var(--jeu-espace-m) 0", maxWidth: "70ch", lineHeight: 1.5 }}>
				Chaque vignette est un <strong>rendu réel</strong> du modèle du jeu, produit par le moteur
				du dépôt&nbsp;: les pièces sont assemblées en glTF puis rastérisées côté serveur. Ouvrez une
				carte pour manipuler le modèle en 3D.
			</p>

			{/* Les familles ne sont pas des dossiers : ce sont les six manières dont le jeu range
			    ses modèles, et chacune a sa propre recette d'assemblage. */}
			<nav aria-label="Familles de modèles" style={{ display: "flex", flexWrap: "wrap", gap: "var(--jeu-espace-s)" }}>
				{familles.map((f) => (
					<button
						key={f.segment}
						type="button"
						aria-pressed={f.segment === famille}
						onClick={() => {
							setFamille(f.segment);
							setPage(1);
							setOuvert(null);
						}}
						style={f.segment === famille ? ONGLET_ACTIF : ONGLET}
					>
						{f.libelle}
						{f.total !== null ? (
							<span style={{ opacity: 0.75, fontWeight: 500 }}> · {n(f.total)}</span>
						) : null}
					</button>
				))}
			</nav>

			<form
				onSubmit={(e) => {
					e.preventDefault();
					setPage(1);
					setFiltre(saisie);
				}}
				style={{ display: "flex", gap: "var(--jeu-espace-s)", margin: "var(--jeu-espace-m) 0" }}
			>
				<input
					type="search"
					value={saisie}
					onChange={(e) => setSaisie(e.target.value)}
					placeholder={
						familleCourante?.source === "miroir"
							? "Chercher un personnage par nom ou par code…"
							: "Chercher un code…"
					}
					aria-label="Chercher un modèle"
					style={CHAMP}
				/>
				<button type="submit" style={BOUTON}>
					Chercher
				</button>
				{filtre ? (
					<button
						type="button"
						onClick={() => {
							setSaisie("");
							setFiltre("");
							setPage(1);
						}}
						style={BOUTON}
					>
						Effacer
					</button>
				) : null}
			</form>

			{ouvert ? <Viewport modele={ouvert} moteur={capacites.moteur} onFermer={() => setOuvert(null)} /> : null}

			{listeKo ? (
				<Note ton="alerte">Ce catalogue n'a pas pu être chargé. Réessayez dans un instant.</Note>
			) : !liste ? (
				<Note>Chargement…</Note>
			) : liste.elements.length === 0 ? (
				<Note>Aucun modèle ne correspond à cette recherche.</Note>
			) : (
				<ul style={GRILLE}>
					{liste.elements.map((m) => (
						<li key={`${m.famille}/${m.code}`}>
							<Carte modele={m} ouvert={ouvert?.code === m.code && ouvert.famille === m.famille} onOuvrir={() => setOuvert(m)} />
						</li>
					))}
				</ul>
			)}

			{liste && liste.pages > 1 ? (
				<nav aria-label="Pagination" style={{ display: "flex", alignItems: "center", gap: "var(--jeu-espace-m)" }}>
					<button type="button" disabled={page <= 1} onClick={() => setPage((p) => p - 1)} style={BOUTON}>
						Précédent
					</button>
					<span aria-live="polite" style={{ fontWeight: 700 }}>
						Page {n(page)} sur {n(liste.pages)}
					</span>
					<button type="button" disabled={page >= liste.pages} onClick={() => setPage((p) => p + 1)} style={BOUTON}>
						Suivant
					</button>
				</nav>
			) : null}
		</section>
	);
}

/**
 * Une carte de la grille : l'aperçu rendu par le serveur, le nom, le code.
 *
 * `loading="lazy"` n'est pas cosmétique ici : chaque `<img>` déclenche, la première fois, une
 * rastérisation côté serveur. Charger les 24 d'un coup pendant qu'on lit le haut de la page
 * remplirait la file de rendu pour rien.
 */
function Carte({ modele, ouvert, onOuvrir }: { modele: Modele; ouvert: boolean; onOuvrir: () => void }) {
	const [echec, setEchec] = useState(false);
	return (
		<button
			type="button"
			onClick={onOuvrir}
			aria-label={`Voir ${modele.nom ?? modele.code} en 3D`}
			style={{
				...CARTE,
				borderColor: ouvert ? "var(--jeu-texte-vif, #2b6cb0)" : "var(--jeu-tuile-bord)",
			}}
		>
			<div style={{ position: "relative", width: "100%", aspectRatio: "1", background: FOND_RENDU }}>
				{echec ? (
					// Un aperçu qui échoue ne laisse pas une case vide : il DIT que le rendu de ce
					// modèle n'aboutit pas. Le GLB, lui, reste souvent servable — d'où le bouton,
					// qui reste actif.
					<span style={MESSAGE_APERCU}>Aperçu indisponible</span>
				) : (
					<img
						src={modele.apercu}
						alt=""
						loading="lazy"
						decoding="async"
						onError={() => setEchec(true)}
						style={{ width: "100%", height: "100%", objectFit: "contain", display: "block" }}
					/>
				)}
			</div>
			<div style={{ padding: "var(--jeu-espace-s)", textAlign: "left" }}>
				<div style={{ fontSize: "0.82rem", fontWeight: 700, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
					{modele.nom ?? modele.code}
				</div>
				<div style={{ fontSize: "0.7rem", color: "var(--jeu-tuile-bas)" }}>
					{/* Le code est l'identité : il est affiché même quand un nom existe, parce que
					    c'est lui qui est dans l'URL et dans les fichiers du jeu. */}
					{modele.code}
					{modele.fichiers !== null ? ` · ${accorde(modele.fichiers, "fichier")}` : ""}
				</div>
			</div>
		</button>
	);
}

/* ------------------------------------------------------------------------------------------ */
/* Le viewport WebGPU                                                                           */
/* ------------------------------------------------------------------------------------------ */

/** Une primitive prête à téléverser. */
interface PrimitiveGl {
	positions: Float32Array;
	normales: Float32Array;
	uv: Float32Array;
	indices: Uint32Array;
	image: number | null;
}

/** Un GLB décodé, réduit à ce que le nuanceur consomme. */
interface ModeleGl {
	primitives: PrimitiveGl[];
	images: Blob[];
	/** Indices écartés parce qu'ils sortaient de leur accesseur `POSITION`. */
	indicesRejetes: number;
}

/** Nombre de composantes par type d'accesseur glTF. */
const COMPOSANTES: Record<string, number> = { SCALAR: 1, VEC2: 2, VEC3: 3, VEC4: 4, MAT4: 16 };

/** Taille en octets et constructeur par `componentType` glTF. */
const COMPOSANT: Record<number, { taille: number; lire: (dv: DataView, o: number) => number }> = {
	5120: { taille: 1, lire: (dv, o) => dv.getInt8(o) },
	5121: { taille: 1, lire: (dv, o) => dv.getUint8(o) },
	5122: { taille: 2, lire: (dv, o) => dv.getInt16(o, true) },
	5123: { taille: 2, lire: (dv, o) => dv.getUint16(o, true) },
	5125: { taille: 4, lire: (dv, o) => dv.getUint32(o, true) },
	5126: { taille: 4, lire: (dv, o) => dv.getFloat32(o, true) },
};

/**
 * Décode un GLB en primitives prêtes pour le GPU.
 *
 * Volontairement partiel, et pour la même raison que `nie_render3d::glb` : les GLB de ce dépôt
 * sont produits par `nie_formats::assemble::to_glb_embedded`, qui écrit les positions **en
 * espace monde**. Les transformations de nœuds et le *skinning* sont donc ignorés — les
 * appliquer déplacerait un modèle déjà placé.
 *
 * L'`byteStride` est respecté : les attributs d'une même primitive peuvent partager une vue
 * entrelacée, et lire à pas serré donnerait une géométrie repliée sur elle-même.
 */
function decoderGlb(buffer: ArrayBuffer): ModeleGl {
	const dv = new DataView(buffer);
	if (dv.getUint32(0, true) !== 0x46546c67) throw new Error("magic glTF absent");
	let json: Record<string, unknown> | null = null;
	let bin: Uint8Array | null = null;
	let o = 12;
	while (o + 8 <= buffer.byteLength) {
		const taille = dv.getUint32(o, true);
		const type = dv.getUint32(o + 4, true);
		const debut = o + 8;
		if (debut + taille > buffer.byteLength) break;
		if (type === 0x4e4f534a) json = JSON.parse(new TextDecoder().decode(new Uint8Array(buffer, debut, taille)));
		else if (type === 0x004e4942) bin = new Uint8Array(buffer, debut, taille);
		o = debut + taille + ((4 - (taille % 4)) % 4);
	}
	if (!json || !bin) throw new Error("chunk JSON ou BIN manquant");

	const racine = json as {
		accessors?: Record<string, unknown>[];
		bufferViews?: Record<string, unknown>[];
		meshes?: { primitives: Record<string, unknown>[] }[];
		materials?: Record<string, unknown>[];
		textures?: { source?: number }[];
		images?: { bufferView?: number; mimeType?: string }[];
	};
	const accessors = racine.accessors ?? [];
	const vues = racine.bufferViews ?? [];

	/** Lit un accesseur en `Float32Array` aplati (composantes × count). */
	function lire(index: number | undefined): { data: Float32Array; count: number; comp: number } | null {
		if (index === undefined) return null;
		const a = accessors[index] as
			| { bufferView?: number; byteOffset?: number; componentType: number; count: number; type: string; normalized?: boolean }
			| undefined;
		if (!a) return null;
		const comp = COMPOSANTES[a.type] ?? 0;
		const spec = COMPOSANT[a.componentType];
		if (!comp || !spec || a.bufferView === undefined) return null;
		const v = vues[a.bufferView] as { byteOffset?: number; byteStride?: number } | undefined;
		if (!v) return null;
		const base = (bin as Uint8Array).byteOffset + (v.byteOffset ?? 0) + (a.byteOffset ?? 0);
		const pas = v.byteStride ?? comp * spec.taille;
		const vue = new DataView(buffer);
		const out = new Float32Array(a.count * comp);
		// Un entier `normalized` se ramène à [0,1] (ou [-1,1] signé) : les UV du jeu sont parfois
		// stockées en u16 normalisé, et les lire brutes donnerait des coordonnées à 65 535.
		const echelle = a.normalized
			? { 5120: 127, 5121: 255, 5122: 32767, 5123: 65535 }[a.componentType as 5120 | 5121 | 5122 | 5123]
			: undefined;
		for (let i = 0; i < a.count; i++) {
			for (let c = 0; c < comp; c++) {
				const brut = spec.lire(vue, base + i * pas + c * spec.taille);
				out[i * comp + c] = echelle ? Math.max(brut / echelle, -1) : brut;
			}
		}
		return { data: out, count: a.count, comp };
	}

	/** `primitive.material → baseColorTexture → textures[].source`. */
	function imageDe(materiau: number | undefined): number | null {
		if (materiau === undefined) return null;
		const m = racine.materials?.[materiau] as
			| { pbrMetallicRoughness?: { baseColorTexture?: { index?: number } } }
			| undefined;
		const t = m?.pbrMetallicRoughness?.baseColorTexture?.index;
		if (t === undefined) return null;
		const src = racine.textures?.[t]?.source;
		return src === undefined ? null : src;
	}

	const primitives: PrimitiveGl[] = [];
	let indicesRejetes = 0;
	for (const mesh of racine.meshes ?? []) {
		for (const p of mesh.primitives) {
			const attributs = p.attributes as Record<string, number> | undefined;
			if (!attributs) continue;
			// `mode` 4 = TRIANGLES. Les autres modes (bandes, éventails) n'apparaissent pas dans
			// les GLB du dépôt ; les rendre comme des triangles produirait une bouillie.
			if (p.mode !== undefined && p.mode !== 4) continue;
			const pos = lire(attributs.POSITION);
			const idx = lire(p.indices as number | undefined);
			if (!pos || !idx || pos.comp !== 3) continue;
			const nor = lire(attributs.NORMAL);
			const uv = lire(attributs.TEXCOORD_0);

			// Un indice hors de l'accesseur POSITION est écarté triangle par triangle, comme le
			// fait `nie_render3d::render`. Le cas est réel : `/model-chr/keshin/k000010.glb`
			// porte des indices GLOBAUX (jusqu'à 11 493) pour des primitives locales de 818 à
			// 2 394 sommets. Le parseur Rust refuse le fichier entier ; ici on rend ce qui est
			// cohérent plutôt que rien.
			const garde = new Uint32Array(idx.data.length);
			let k = 0;
			for (let t = 0; t + 2 < idx.data.length; t += 3) {
				const a = idx.data[t] ?? 0;
				const b = idx.data[t + 1] ?? 0;
				const c = idx.data[t + 2] ?? 0;
				if (a < pos.count && b < pos.count && c < pos.count) {
					garde[k++] = a;
					garde[k++] = b;
					garde[k++] = c;
				} else {
					indicesRejetes += 3;
				}
			}
			if (k === 0) continue;

			primitives.push({
				positions: pos.data,
				normales: nor && nor.comp === 3 ? nor.data : new Float32Array(pos.count * 3),
				uv: uv && uv.comp === 2 ? uv.data : new Float32Array(pos.count * 2),
				indices: garde.subarray(0, k),
				image: imageDe(p.material as number | undefined),
			});
		}
	}

	const images: Blob[] = [];
	for (const img of racine.images ?? []) {
		if (img.bufferView === undefined) {
			images.push(new Blob([]));
			continue;
		}
		const v = vues[img.bufferView] as { byteOffset?: number; byteLength?: number } | undefined;
		if (!v) {
			images.push(new Blob([]));
			continue;
		}
		const debut = bin.byteOffset + (v.byteOffset ?? 0);
		images.push(
			new Blob([new Uint8Array(buffer, debut, v.byteLength ?? 0)], { type: img.mimeType ?? "image/png" }),
		);
	}

	return { primitives, images, indicesRejetes };
}

/** Boîte englobante → centre et rayon, exactement comme `nie_render3d::render::bounds`. */
function bornes(m: ModeleGl): { centre: [number, number, number]; rayon: number } {
	const lo = [Infinity, Infinity, Infinity];
	const hi = [-Infinity, -Infinity, -Infinity];
	for (const p of m.primitives) {
		for (let i = 0; i + 2 < p.positions.length; i += 3) {
			for (let k = 0; k < 3; k++) {
				const v = p.positions[i + k] ?? 0;
				if (v < (lo[k] ?? Infinity)) lo[k] = v;
				if (v > (hi[k] ?? -Infinity)) hi[k] = v;
			}
		}
	}
	if (!Number.isFinite(lo[0] ?? Infinity)) return { centre: [0, 0, 0], rayon: 1 };
	const centre: [number, number, number] = [
		(((lo[0] ?? 0) + (hi[0] ?? 0)) * 0.5),
		(((lo[1] ?? 0) + (hi[1] ?? 0)) * 0.5),
		(((lo[2] ?? 0) + (hi[2] ?? 0)) * 0.5),
	];
	let rayon = 1e-3;
	for (let k = 0; k < 3; k++) rayon = Math.max(rayon, ((hi[k] ?? 0) - (lo[k] ?? 0)) * 0.5);
	return { centre, rayon };
}

/**
 * Le module WGSL — les deux nuanceurs. Celui de sommets est la transcription littérale de
 * `nie_render3d::render::render`.
 *
 * Le modèle est ramené dans sa sphère unité (`(p − centre) / rayon`), tourné autour de Y, puis
 * incliné autour de X ; la profondeur est `distance − z` et la projection `focale · r / z`.
 * `aspect` reproduit un détail du rastériseur qui se lit mal dans son code : les deux axes y
 * sont mis à l'échelle par la LARGEUR (`scale = w * 0.5`), donc la composante verticale porte le
 * rapport largeur/hauteur.
 *
 * Celui de fragments module la texture par un Lambert et découpe l'alpha au seuil du rastériseur
 * (8/255). L'argile `rgb(206, 198, 188)` des primitives sans texture n'y est plus une branche : on
 * leur lie une texture 1×1 de cette couleur exacte, ce qui donne la même valeur sans condition.
 */
const MODULE_WGSL = `
struct Uniformes {
  centre : vec3f,
  invRayon : f32,
  rotY : vec2f,
  rotX : vec2f,
  distance : f32,
  focale : f32,
  aspect : f32,
  remplissage : f32,
}

@group(0) @binding(0) var<uniform> u : Uniformes;
@group(1) @binding(0) var echantillonneur : sampler;
@group(1) @binding(1) var tex : texture_2d<f32>;

struct Sortie {
  @builtin(position) position : vec4f,
  @location(0) uv : vec2f,
  @location(1) ombre : f32,
}

fn orienter(v : vec3f) -> vec3f {
  let x = v.x * u.rotY.x + v.z * u.rotY.y;
  let z = -v.x * u.rotY.y + v.z * u.rotY.x;
  return vec3f(x, v.y * u.rotX.x - z * u.rotX.y, v.y * u.rotX.y + z * u.rotX.x);
}

const PROCHE : f32 = 0.05;
const LOIN : f32 = 24.0;

@vertex
fn sommet(
  @location(0) position : vec3f,
  @location(1) normale : vec3f,
  @location(2) uv : vec2f,
) -> Sortie {
  let r = orienter((position - u.centre) * u.invRayon);
  let d = u.distance - r.z;
  // WebGPU : z normalise dans [0, 1]. a*PROCHE + b = 0 et a*LOIN + b = LOIN, donc
  // z/w vaut 0 au plan proche et 1 au plan lointain. En OpenGL le meme calcul demanderait
  // (LOIN + PROCHE) / (LOIN - PROCHE) ; l'ecart ne se voit qu'a l'ecretage.
  let a = LOIN / (LOIN - PROCHE);
  let b = -LOIN * PROCHE / (LOIN - PROCHE);
  var s : Sortie;
  s.position = vec4f(u.focale * r.x, u.focale * r.y * u.aspect, a * d + b, d);
  s.uv = uv;
  let nn = orienter(normale);
  let l = length(nn);
  var lambert = 1.0;
  if (l > 0.0) {
    lambert = abs(dot(nn / l, normalize(vec3f(0.35, 0.75, 0.55))));
  }
  s.ombre = 0.35 + 0.65 * lambert;
  return s;
}

@fragment
fn fragment(e : Sortie) -> @location(0) vec4f {
  let c = textureSample(tex, echantillonneur, e.uv);
  // Meme seuil de decoupe que le rastériseur : 8/255.
  if (c.a < 0.0314) {
    discard;
  }
  return vec4f(c.rgb * e.ombre, 1.0);
}
`;

/**
 * Ce qui vit sur le GPU pour un modèle. Détruit en bloc à la fermeture — WebGPU expose
 * `destroy()` sur les tampons, les textures et le périphérique lui-même, et c'est ce `destroy()`
 * qui remplace les `deleteBuffer`/`deleteTexture`/`WEBGL_lose_context` de l'ancien chemin.
 */
interface RessourcesGpu {
	lots: { positions: GPUBuffer; normales: GPUBuffer; uv: GPUBuffer; indices: GPUBuffer; nb: number; groupe: GPUBindGroup }[];
	tampons: GPUBuffer[];
	textures: GPUTexture[];
}

/**
 * La texture 1×1 de l'argile du rastériseur — `rgb(206, 198, 188)`.
 *
 * Elle remplace la branche `uTexture` du nuanceur WebGL : le format `rgba8unorm` ne convertit
 * rien, donc l'échantillon vaut exactement `(0.808, 0.776, 0.737, 1.0)`, la constante que
 * l'ancien fragment écrivait en dur.
 */
function argile(device: GPUDevice): GPUTexture {
	const t = device.createTexture({
		size: [1, 1],
		format: "rgba8unorm",
		usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST,
	});
	device.queue.writeTexture({ texture: t }, new Uint8Array([206, 198, 188, 255]), { bytesPerRow: 4 }, [1, 1]);
	return t;
}

/** Téléverse un `Float32Array`/`Uint32Array` dans un tampon GPU frais. */
function tampon(device: GPUDevice, data: Float32Array | Uint32Array, usage: number): GPUBuffer {
	// La taille d'un tampon `mappedAtCreation` doit être un multiple de 4 ; nos vues le sont
	// toujours (4 octets par composante), mais l'arrondi rend la fonction sûre pour tout appelant.
	const taille = Math.max(4, (data.byteLength + 3) & ~3);
	const b = device.createBuffer({ size: taille, usage, mappedAtCreation: true });
	if (data instanceof Float32Array) new Float32Array(b.getMappedRange()).set(data);
	else new Uint32Array(b.getMappedRange()).set(data);
	b.unmap();
	return b;
}

/**
 * Le viewport : un seul périphérique WebGPU, demandé à l'ouverture d'une carte et détruit à sa
 * fermeture.
 *
 * Tout est libéré explicitement — `destroy()` sur chaque tampon, chaque texture, puis sur le
 * `GPUDevice` : un canevas simplement démonté laisse ses ressources vivantes jusqu'au prochain
 * ramassage, et ouvrir puis fermer une dizaine de modèles suffit à immobiliser plusieurs centaines
 * de mégaoctets de mémoire GPU, sans le moindre message.
 */
function Viewport({ modele, moteur, onFermer }: { modele: Modele; moteur: Moteur; onFermer: () => void }) {
	const canevas = useRef<HTMLCanvasElement | null>(null);
	const [etat, setEtat] = useState<"chargement" | "pret" | "echec">("chargement");
	const [raison, setRaison] = useState("");
	const [analyse, setAnalyse] = useState<Analyse | null>(null);
	const [rejetes, setRejetes] = useState(0);
	// L'orientation vit dans une ref plutôt que dans un état : elle change à chaque mouvement de
	// souris, et un `setState` par image ferait re-rendre React soixante fois par seconde pour
	// une valeur que seul le nuanceur lit.
	const vue = useRef({ angle: 0.6, tilt: moteur.tilt, distance: moteur.distance });

	useEffect(() => {
		const ac = new AbortController();
		setAnalyse(null);
		json<Analyse>(`/api/v1/3d/modeles/${modele.famille}/${modele.code}/analyse`, ac.signal)
			.then(setAnalyse)
			// L'analyse est un complément : son échec (GLB refusé par le parseur Rust) ne doit
			// pas masquer un viewport qui, lui, sait rendre ce qui est cohérent.
			.catch(() => undefined);
		return () => ac.abort();
	}, [modele.famille, modele.code]);

	useEffect(() => {
		const toile = canevas.current;
		if (!toile) return;
		// `navigator.gpu` peut être absent (navigateur sans WebGPU) OU présent sans adaptateur
		// utilisable (pilote sur liste noire, machine sans GPU exposé). Les deux cas mènent au même
		// endroit : le repli PNG, jamais une case vide.
		if (!navigator.gpu) {
			setEtat("echec");
			setRaison("Ce navigateur n'expose pas WebGPU. L'aperçu rendu par le serveur reste disponible.");
			return;
		}
		const gpu = navigator.gpu;

		const ac = new AbortController();
		let vivant = true;
		let animation = 0;
		let ressources: RessourcesGpu | null = null;
		let device: GPUDevice | null = null;
		let profondeur: GPUTexture | null = null;
		let uniformes: GPUBuffer | null = null;
		vue.current = { angle: 0.6, tilt: moteur.tilt, distance: moteur.distance };
		setEtat("chargement");
		setRejetes(0);

		(async () => {
			const adaptateur = await gpu.requestAdapter();
			if (!adaptateur) {
				throw new Error(
					"Ce navigateur expose WebGPU mais aucun adaptateur graphique n'est utilisable. L'aperçu rendu par le serveur reste disponible.",
				);
			}
			const dev = await adaptateur.requestDevice();
			device = dev;
			if (!vivant) return;
			const contexte = toile.getContext("webgpu");
			if (!contexte) throw new Error("le canevas n'accepte pas de contexte WebGPU");
			const format = gpu.getPreferredCanvasFormat();
			contexte.configure({ device: dev, format, alphaMode: "opaque" });

			const r = await fetch(modele.glb, { signal: ac.signal });
			if (!r.ok) throw new Error(`le modèle n'a pas pu être assemblé (${r.status})`);
			const longueur = Number(r.headers.get("content-length") ?? 0);
			if (longueur > GLB_OCTETS_MAX) throw new Error(`modèle trop lourd pour le navigateur (${octets(longueur)})`);
			const buffer = await r.arrayBuffer();
			if (!vivant) return;
			const decode = decoderGlb(buffer);
			if (decode.primitives.length === 0) throw new Error("aucune géométrie exploitable dans ce modèle");
			setRejetes(decode.indicesRejetes);

			// Les images sont décodées AVANT le premier dessin : monter le modèle en gris puis
			// voir ses textures apparaître une par une donne l'impression d'un rendu qui rate.
			const bitmaps = await Promise.all(
				decode.images.map((b) => (b.size > 0 ? createImageBitmap(b).catch(() => null) : Promise.resolve(null))),
			);
			if (!vivant) return;

			// Une erreur de validation WGSL n'est pas levée par `createRenderPipeline` : elle part
			// dans une portée d'erreur asynchrone. Sans cette portée, un nuanceur fautif ne
			// produirait qu'un canevas noir muet.
			dev.pushErrorScope("validation");
			const module = dev.createShaderModule({ code: MODULE_WGSL });
			const pipeline = dev.createRenderPipeline({
				layout: "auto",
				vertex: {
					module,
					entryPoint: "sommet",
					buffers: [
						{ arrayStride: 12, attributes: [{ shaderLocation: 0, offset: 0, format: "float32x3" }] },
						{ arrayStride: 12, attributes: [{ shaderLocation: 1, offset: 0, format: "float32x3" }] },
						{ arrayStride: 8, attributes: [{ shaderLocation: 2, offset: 0, format: "float32x2" }] },
					],
				},
				fragment: { module, entryPoint: "fragment", targets: [{ format }] },
				// Faces arrière CONSERVÉES (`cullMode: "none"`) : c'est le choix documenté du
				// chemin GPU de `nie-render3d` — les maillages du jeu ont une orientation
				// incohérente, et les écarter fait disparaître des mèches de cheveux et
				// l'intérieur des bouches.
				primitive: { topology: "triangle-list", cullMode: "none" },
				depthStencil: { format: "depth24plus", depthCompare: "less", depthWriteEnabled: true },
			});
			const faute = await dev.popErrorScope();
			if (faute) throw new Error(`pipeline de rendu : ${faute.message}`);
			if (!vivant) return;

			const textures: GPUTexture[] = [];
			for (const bmp of bitmaps) {
				if (!bmp) {
					// Une place doit rester dans le tableau : `p.image` indexe `racine.images`,
					// et décaler l'index donnerait à un modèle la texture d'un autre.
					textures.push(argile(dev));
					continue;
				}
				const t = dev.createTexture({
					size: [bmp.width, bmp.height],
					format: "rgba8unorm",
					usage:
						GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST | GPUTextureUsage.RENDER_ATTACHMENT,
				});
				// Pas de retournement vertical : `nie_render3d::render::sample` indexe la ligne
				// `v * hauteur` depuis le HAUT de l'image, et `copyExternalImageToTexture` écrit
				// la première ligne de l'`ImageBitmap` en `v = 0`. Les deux coïncident — poser
				// `flipY: true` ici retournerait toutes les textures par rapport à la vignette.
				dev.queue.copyExternalImageToTexture({ source: bmp }, { texture: t }, [bmp.width, bmp.height]);
				bmp.close();
				textures.push(t);
			}
			// La primitive sans texture n'est pas une branche du nuanceur : elle reçoit une
			// texture 1×1 de l'argile du rastériseur, qui donne exactement la même couleur.
			const defaut = argile(dev);

			// `clamp-to-edge`, comme le rastériseur qui borne ses UV à [0,1]. Pas de
			// `mipmapFilter` : WebGPU n'a pas de `generateMipmap`, et fabriquer la pyramide
			// demanderait une passe de rendu par niveau pour un gain invisible à cette taille.
			const echantillonneur = dev.createSampler({
				magFilter: "linear",
				minFilter: "linear",
				addressModeU: "clamp-to-edge",
				addressModeV: "clamp-to-edge",
			});

			const { centre, rayon } = bornes(decode);
			// 12 flottants : centre(3) + invRayon(1) + rotY(2) + rotX(2) + distance + focale +
			// aspect + remplissage. La disposition suit celle de `struct Uniformes` en WGSL.
			const donneesU = new Float32Array(12);
			donneesU[0] = centre[0];
			donneesU[1] = centre[1];
			donneesU[2] = centre[2];
			donneesU[3] = 1 / rayon;
			donneesU[9] = moteur.focale;
			uniformes = dev.createBuffer({
				size: donneesU.byteLength,
				usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
			});
			const groupeU = dev.createBindGroup({
				layout: pipeline.getBindGroupLayout(0),
				entries: [{ binding: 0, resource: { buffer: uniformes } }],
			});

			const tampons: GPUBuffer[] = [];
			const lots: RessourcesGpu["lots"] = [];
			for (const p of decode.primitives) {
				const positions = tampon(dev, p.positions, GPUBufferUsage.VERTEX);
				const normales = tampon(dev, p.normales, GPUBufferUsage.VERTEX);
				const uv = tampon(dev, p.uv, GPUBufferUsage.VERTEX);
				const indices = tampon(dev, p.indices, GPUBufferUsage.INDEX);
				tampons.push(positions, normales, uv, indices);
				const tex = p.image !== null ? (textures[p.image] ?? defaut) : defaut;
				lots.push({
					positions,
					normales,
					uv,
					indices,
					nb: p.indices.length,
					groupe: dev.createBindGroup({
						layout: pipeline.getBindGroupLayout(1),
						entries: [
							{ binding: 0, resource: echantillonneur },
							{ binding: 1, resource: tex.createView() },
						],
					}),
				});
			}
			ressources = { lots, tampons, textures: [...textures, defaut] };

			setEtat("pret");

			const dessiner = () => {
				if (!vivant || !ressources) return;
				const dpr = Math.min(window.devicePixelRatio || 1, 2);
				const l = Math.max(1, Math.round(toile.clientWidth * dpr));
				const h = Math.max(1, Math.round(toile.clientHeight * dpr));
				if (toile.width !== l || toile.height !== h) {
					toile.width = l;
					toile.height = h;
				}
				// La texture de profondeur doit avoir la taille EXACTE de la cible de couleur :
				// WebGPU refuse la passe sinon. Elle est donc recréée à chaque redimensionnement,
				// et l'ancienne détruite — sans quoi un simple étirement de fenêtre fuit.
				let cible = profondeur;
				if (!cible || cible.width !== l || cible.height !== h) {
					cible?.destroy();
					cible = dev.createTexture({
						size: [l, h],
						format: "depth24plus",
						usage: GPUTextureUsage.RENDER_ATTACHMENT,
					});
					profondeur = cible;
				}

				const { angle, tilt, distance } = vue.current;
				donneesU[4] = Math.cos(angle);
				donneesU[5] = Math.sin(angle);
				donneesU[6] = Math.cos(tilt);
				donneesU[7] = Math.sin(tilt);
				donneesU[8] = distance;
				donneesU[10] = l / h;
				dev.queue.writeBuffer(uniformes as GPUBuffer, 0, donneesU);

				const encodeur = dev.createCommandEncoder();
				const passe = encodeur.beginRenderPass({
					colorAttachments: [
						{
							view: contexte.getCurrentTexture().createView(),
							// Le même dégradé que `nie_render3d::render::couleur_fond` en son
							// milieu : la vignette et le viewport doivent se poser sur le même
							// fond, sinon passer de l'une à l'autre ressemble à un changement de
							// rendu.
							clearValue: { r: 37 / 255, g: 43 / 255, b: 57 / 255, a: 1 },
							loadOp: "clear",
							storeOp: "store",
						},
					],
					depthStencilAttachment: {
						view: cible.createView(),
						depthClearValue: 1,
						depthLoadOp: "clear",
						depthStoreOp: "store",
					},
				});
				passe.setPipeline(pipeline);
				passe.setBindGroup(0, groupeU);
				for (const v of ressources.lots) {
					passe.setBindGroup(1, v.groupe);
					passe.setVertexBuffer(0, v.positions);
					passe.setVertexBuffer(1, v.normales);
					passe.setVertexBuffer(2, v.uv);
					passe.setIndexBuffer(v.indices, "uint32");
					passe.drawIndexed(v.nb);
				}
				passe.end();
				dev.queue.submit([encodeur.finish()]);
				animation = requestAnimationFrame(dessiner);
			};
			animation = requestAnimationFrame(dessiner);
		})().catch((e: unknown) => {
			if (!vivant || ac.signal.aborted) return;
			setEtat("echec");
			setRaison(e instanceof Error ? e.message : "modèle illisible");
		});

		return () => {
			vivant = false;
			ac.abort();
			cancelAnimationFrame(animation);
			if (ressources) {
				for (const b of ressources.tampons) b.destroy();
				for (const t of ressources.textures) t.destroy();
			}
			uniformes?.destroy();
			profondeur?.destroy();
			// `destroy()` sur le périphérique invalide tout ce qui en dépend d'un coup — c'est le
			// remplaçant direct de `WEBGL_lose_context` de l'ancien chemin WebGL 2.
			device?.destroy();
		};
	}, [modele.glb, moteur.focale, moteur.distance, moteur.tilt]);

	// Manipulation : glisser tourne et incline, la molette rapproche. Les bornes du tilt
	// évitent de passer sous les pieds du modèle, où la vue n'a plus de sens.
	const enCours = useRef<{ x: number; y: number } | null>(null);
	const onDown = useCallback((e: React.PointerEvent<HTMLCanvasElement>) => {
		enCours.current = { x: e.clientX, y: e.clientY };
		e.currentTarget.setPointerCapture(e.pointerId);
	}, []);
	const onMove = useCallback((e: React.PointerEvent<HTMLCanvasElement>) => {
		const p = enCours.current;
		if (!p) return;
		vue.current.angle += (e.clientX - p.x) * 0.01;
		vue.current.tilt = Math.min(1.2, Math.max(-1.2, vue.current.tilt + (e.clientY - p.y) * 0.006));
		enCours.current = { x: e.clientX, y: e.clientY };
	}, []);
	const onUp = useCallback((e: React.PointerEvent<HTMLCanvasElement>) => {
		enCours.current = null;
		e.currentTarget.releasePointerCapture(e.pointerId);
	}, []);
	const onWheel = useCallback((e: React.WheelEvent<HTMLCanvasElement>) => {
		vue.current.distance = Math.min(9, Math.max(1.4, vue.current.distance + Math.sign(e.deltaY) * 0.2));
	}, []);

	const titre = modele.nom ?? modele.code;

	return (
		<section aria-label={`Vue 3D de ${titre}`} style={PANNEAU}>
			<header style={{ display: "flex", alignItems: "baseline", gap: "var(--jeu-espace-m)", flexWrap: "wrap" }}>
				<h3 style={{ margin: 0, fontSize: "1.1rem" }}>{titre}</h3>
				<code style={{ fontSize: "0.8rem", opacity: 0.75 }}>
					{modele.famille}/{modele.code}
				</code>
				<span style={{ flex: 1 }} />
				<a href={modele.glb} download style={LIEN}>
					Télécharger le glTF
				</a>
				<button type="button" onClick={onFermer} style={BOUTON}>
					Fermer
				</button>
			</header>

			<div style={{ position: "relative", marginTop: "var(--jeu-espace-m)" }}>
				<canvas
					ref={canevas}
					onPointerDown={onDown}
					onPointerMove={onMove}
					onPointerUp={onUp}
					onPointerCancel={onUp}
					onWheel={onWheel}
					style={{
						width: "100%",
						aspectRatio: "16/10",
						display: "block",
						borderRadius: "var(--jeu-rayon)",
						background: FOND_RENDU,
						cursor: etat === "pret" ? "grab" : "default",
						touchAction: "none",
					}}
				/>
				{etat !== "pret" ? (
					<div style={SURCOUCHE}>
						{etat === "chargement" ? (
							"Assemblage du modèle…"
						) : (
							<>
								<div style={{ marginBottom: "var(--jeu-espace-s)" }}>{raison}</div>
								{/* Le viewport a échoué : on montre au moins le rendu du serveur, qui
								    est produit par un autre chemin et réussit souvent quand celui-ci
								    échoue (et réciproquement). */}
								<img src={modele.apercu} alt="" style={{ maxWidth: 260, borderRadius: "var(--jeu-rayon)" }} />
							</>
						)}
					</div>
				) : null}
			</div>

			<p style={{ fontSize: "0.78rem", opacity: 0.8, margin: "var(--jeu-espace-s) 0 0" }}>
				Glisser pour tourner, molette pour approcher.
				{analyse
					? ` ${n(analyse.triangles)} triangles, ${n(analyse.sommets)} sommets, ${accorde(
							analyse.textures.length,
							"texture",
						)}, ${octets(analyse.glb_octets)} de glTF.`
					: ""}
				{rejetes > 0
					? ` ${n(rejetes)} indices de sommet hors bornes ont été écartés : le glTF assemblé pour ce modèle est incohérent.`
					: ""}
			</p>
		</section>
	);
}

/* ------------------------------------------------------------------------------------------ */
/* Habillage — les mêmes formes que le reste du site                                            */
/* ------------------------------------------------------------------------------------------ */

/** Le fond des rendus, aligné sur `nie_render3d::render::couleur_fond`. */
const FOND_RENDU = "linear-gradient(180deg, rgb(24, 28, 40), rgb(50, 58, 74))";

const GRILLE: React.CSSProperties = {
	display: "grid",
	gridTemplateColumns: "repeat(auto-fill, minmax(180px, 1fr))",
	gap: "var(--jeu-espace-m)",
	listStyle: "none",
	margin: "var(--jeu-espace-l) 0",
	padding: 0,
};

const CARTE: React.CSSProperties = {
	display: "block",
	width: "100%",
	padding: 0,
	background: "#fff",
	border: "2px solid var(--jeu-tuile-bord)",
	borderRadius: "var(--jeu-rayon)",
	color: "var(--jeu-nuit-profonde)",
	overflow: "hidden",
	boxShadow: "var(--jeu-ombre-tuile)",
	cursor: "pointer",
	font: "inherit",
	textAlign: "left",
};

const MESSAGE_APERCU: React.CSSProperties = {
	position: "absolute",
	inset: 0,
	display: "grid",
	placeItems: "center",
	color: "#cbd5e1",
	fontSize: "0.78rem",
	padding: "var(--jeu-espace-s)",
	textAlign: "center",
};

const PANNEAU: React.CSSProperties = {
	background: "#fff",
	border: "2px solid var(--jeu-tuile-bord)",
	borderRadius: "var(--jeu-rayon)",
	padding: "var(--jeu-espace-m)",
	margin: "var(--jeu-espace-m) 0",
	boxShadow: "var(--jeu-ombre-tuile)",
	color: "var(--jeu-nuit-profonde)",
};

const SURCOUCHE: React.CSSProperties = {
	position: "absolute",
	inset: 0,
	display: "grid",
	placeItems: "center",
	alignContent: "center",
	textAlign: "center",
	color: "#e2e8f0",
	padding: "var(--jeu-espace-m)",
	fontSize: "0.85rem",
};

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

const ONGLET: React.CSSProperties = {
	padding: "var(--jeu-espace-s) var(--jeu-espace-m)",
	border: "2px solid var(--jeu-tuile-bord)",
	borderRadius: "var(--jeu-rayon)",
	background: "#fff",
	color: "var(--jeu-nuit-profonde)",
	font: "inherit",
	fontWeight: 700,
	cursor: "pointer",
};

const ONGLET_ACTIF: React.CSSProperties = {
	...ONGLET,
	border: 0,
	background: "linear-gradient(180deg, var(--jeu-tuile-haut), var(--jeu-tuile-bas))",
	color: "var(--jeu-texte-vif)",
};

const CHAMP: React.CSSProperties = {
	flex: 1,
	padding: "var(--jeu-espace-s) var(--jeu-espace-m)",
	background: "#fff",
	border: "2px solid var(--jeu-tuile-bord)",
	borderRadius: "var(--jeu-rayon)",
	color: "var(--jeu-nuit-profonde)",
	font: "inherit",
};

const LIEN: React.CSSProperties = {
	color: "var(--jeu-nuit-profonde)",
	fontWeight: 700,
	fontSize: "0.85rem",
};
