"use client";

/**
 * Les six panneaux de l'éditeur — un par onglet, dans le rôle et l'ordre que le jeu leur donne.
 *
 * Chaque panneau lit sa structure dans `structure.ts` (grilles issues des `objbin`, association
 * rubrique → catégorie recoupée sur les captures) et ses libellés dans le catalogue par hachage
 * (`libelles.ts`). Aucun texte, aucune géométrie de grille n'est écrit ici.
 *
 * Ce que le jeu fait et qui n'est pas reproduit : les teintes des palettes de préréglages. Le jeu
 * les applique au moment du rendu (`CMenuChangeMaterialColor`) et aucun fichier lu ne porte leur
 * valeur RVB — les cases restent donc vides plutôt que colorées au hasard. Les trois curseurs de
 * couleur, eux, ont une teinte calculée : elle vient de leurs propres valeurs.
 */

import { useMemo } from "react";

import {
	BUILDS,
	ELEMENTS,
	H,
	MORPHOLOGIES,
	PERSONNALITES_VOIX,
	TYPES_VOIX,
	type Libelle,
} from "./libelles";
import {
	COULEUR_MAX,
	CURSEUR_MAX,
	RUBRIQUES_VISAGE,
	SECTIONS_HABITS,
	type Section,
} from "./structure";
import { A01, Barre, BOUTS, Curseur, FONT, GRIS, ICN, ICN2, Ligne, Note, Pagination, Sprite, Titre, Txt, TxtGaiji, urlVignette, VIGNETTE_W, Vignette } from "./ui";
import type { Catalogue, Categorie } from "./types";

/** Ce que tous les panneaux reçoivent. */
export type Contexte = {
	cdn: string;
	catalogue: Catalogue;
	lib: (hash: string) => Libelle | undefined;
	/** Texte d'un libellé, vide si le jeu ne le porte pas. */
	txt: (hash: string) => string;
	choix: Record<number, string>;
	setChoix: (categorie: number, id: string) => void;
	valeurs: Record<string, number>;
	setValeur: (cle: string, v: number) => void;
	page: Record<string, number>;
	setPage: (cle: string, n: number) => void;
	genre: number;
	setGenre: (g: number) => void;
	morphologie: number;
	setMorphologie: (m: number) => void;
	/** Rubrique ouverte dans la colonne (onglet Visage ou Stats). */
	rubrique: number;
	/** Sous-choix ouvert d'une rubrique en lignes (Yeux, Extras). */
	ligneOuverte: string | null;
	setLigneOuverte: (cle: string | null) => void;
};

/** Retrouve une catégorie du catalogue par son `faceSettingType`. */
function categorie(catalogue: Catalogue, type: number | undefined): Categorie | undefined {
	if (type === undefined) return undefined;
	return catalogue.categories.find((c) => c.faceSettingType === type);
}

/** Une grille de vignettes paginée, avec ses chevrons et ses badges de touche. */
function Grille({
	ctx,
	cle,
	cat,
	colonnes,
	lignes,
}: {
	ctx: Contexte;
	cle: string;
	cat: Categorie;
	colonnes: number;
	lignes: number;
}) {
	const parPage = colonnes * lignes;
	const pages = Math.max(1, Math.ceil(cat.parts.length / parPage));
	const page = Math.min(ctx.page[cle] ?? 0, pages - 1);
	const visibles = cat.parts.slice(page * parPage, page * parPage + parPage);
	return (
		<div className="relative shrink-0">
			<div
				className="grid gap-x-[3%] gap-y-[12%]"
				style={{ gridTemplateColumns: `repeat(${colonnes}, minmax(0, 1fr))` }}
			>
				{visibles.map((p, i) => (
					<Vignette
						key={`${p.id}-${p.itemNo}`}
						cdn={ctx.cdn}
						numero={String(page * parPage + i + 1).padStart(2, "0")}
						image={urlVignette(ctx.cdn, p.icone)}
						choisie={ctx.choix[cat.faceSettingType] === p.id}
						onClick={() => ctx.setChoix(cat.faceSettingType, p.id)}
						className="aspect-square"
					/>
				))}
			</div>
			<Pagination
				cdn={ctx.cdn}
				page={page}
				pages={pages}
				setPage={(n) => ctx.setPage(cle, n)}
			/>
			{pages > 1 && (
				<>
					<Sprite
						cdn={ctx.cdn}
						src={`${FONT}/gaiji_w.png`}
						className="absolute bottom-[-8%] left-[-1.6%] h-[1vw] w-auto"
					/>
					<Sprite
						cdn={ctx.cdn}
						src={`${FONT}/gaiji_c.png`}
						className="absolute bottom-[-8%] right-[-1.6%] h-[1vw] w-auto"
					/>
				</>
			)}
		</div>
	);
}

/**
 * Une palette de préréglages puis les trois composantes de la couleur.
 *
 * La grille de la palette vient du catalogue (`grillePalette`, recoupée par le nombre de couleurs
 * et le nom des écrans `color_preset_list_10x4` / `_12x5` / `_13x5`). Les pastilles n'ont pas de
 * teinte : leur valeur RVB n'est dans aucun fichier lu à ce jour.
 */
function Couleur({ ctx, cat, cle }: { ctx: Contexte; cat: Categorie; cle: string }) {
	const g = cat.grillePalette;
	const h = ctx.valeurs[`${cle}.h`] ?? 0;
	const s = ctx.valeurs[`${cle}.s`] ?? 0;
	const v = ctx.valeurs[`${cle}.v`] ?? COULEUR_MAX;
	const teinte = `hsl(${(h / (COULEUR_MAX + 1)) * 360} ${(s / COULEUR_MAX) * 100}% ${(v / COULEUR_MAX) * 50 + 25}%)`;
	return (
		<>
			<span className="mb-[1%] mt-[1%] flex shrink-0 items-center justify-center gap-[6%]">
				<Sprite
					cdn={ctx.cdn}
					src={`${A01}/avatar01_10/edit_win_icon04.png`}
					className="h-[2.4vw] w-auto"
				/>
				<span
					className="size-[2.4vw] rounded-[0.2vw] border border-[#C9D6E2]"
					style={{ background: teinte }}
				/>
			</span>
			<Titre cdn={ctx.cdn} t={ctx.txt(H.prereglageCouleur)} />
			{g && (
				<div className="relative mb-[2%] shrink-0">
					<Sprite
						cdn={ctx.cdn}
						src={`${A01}/avatar01_3${g.colonnes === 10 ? "5" : "4"}/color_preset_base0${g.colonnes === 10 ? "1" : "2"}.png`}
						className="absolute inset-0 size-full"
					/>
					<div
						className="relative grid gap-[1.5%] p-[1.5%]"
						style={{ gridTemplateColumns: `repeat(${g.colonnes}, minmax(0, 1fr))` }}
					>
					{cat.couleurs.map((c, i) => (
							<button
								key={`${c}-${i}`}
								type="button"
								onClick={() => {
									ctx.setValeur(`${cle}.preset`, i);
									// Clé canonique, indépendante du chemin de panneau : c'est elle que
									// l'assemblage du modèle lit pour teinter. Sans elle il faudrait
									// reconstituer `cle` depuis l'éditeur, qui ne connaît pas la
									// hiérarchie des panneaux.
									ctx.setValeur(`couleur.${cat.faceSettingType}`, i);
								}}
								className="relative aspect-square"
							>
								{ctx.valeurs[`${cle}.preset`] === i && (
									<Sprite
										cdn={ctx.cdn}
										src={`${A01}/avatar01_36/color_preset01_ol.png`}
										className="absolute inset-[-14%] size-[128%]"
									/>
								)}
							</button>
						))}
					</div>
				</div>
			)}
			<Titre cdn={ctx.cdn} t={ctx.txt(H.ajusterCouleur)} />
			<div className="flex shrink-0 flex-col gap-[2%]">
				<Barre
					cdn={ctx.cdn}
					bouts={BOUTS.couleur}
					valeur={h}
					max={COULEUR_MAX}
					onChange={(n) => ctx.setValeur(`${cle}.h`, n)}
					degrade
				/>
				<Barre
					cdn={ctx.cdn}
					bouts={BOUTS.couleur}
					valeur={s}
					max={COULEUR_MAX}
					onChange={(n) => ctx.setValeur(`${cle}.s`, n)}
					pisteFond={`linear-gradient(90deg, #9AA6B2, hsl(${(h / (COULEUR_MAX + 1)) * 360} 100% 50%))`}
				/>
				<Barre
					cdn={ctx.cdn}
					bouts={BOUTS.couleur}
					valeur={v}
					max={COULEUR_MAX}
					onChange={(n) => ctx.setValeur(`${cle}.v`, n)}
					pisteFond="linear-gradient(90deg, #000, #fff)"
				/>
			</div>
		</>
	);
}

/** Une section de rubrique : grille, curseurs, palette ou lignes de sélecteur. */
function SectionRendue({ ctx, s, cle }: { ctx: Contexte; s: Section; cle: string }) {
	const cat = categorie(ctx.catalogue, s.categorie ?? s.couleurDe);

	if (s.lignes) {
		return (
			<>
				{s.lignes.map((l) => {
					const c = categorie(ctx.catalogue, l.categorie);
					const choisi = c?.parts.find((p) => p.id === ctx.choix[l.categorie]);
					const ouverte = ctx.ligneOuverte === l.hash;
					return (
						<div key={l.hash} className="shrink-0">
							<Titre cdn={ctx.cdn} t={ctx.txt(l.hash)} />
							<Ligne
								cdn={ctx.cdn}
								icone={`${A01}/avatar01_10/edit_win_icon${l.icone}.png`}
								numero={choisi ? String(choisi.itemNo).padStart(2, "0") : "00"}
								image={urlVignette(ctx.cdn, choisi?.icone ?? null)}
								choisie={ouverte}
								onClick={() => ctx.setLigneOuverte(ouverte ? null : l.hash)}
							/>
							{l.couleurDe !== undefined && (
								<Ligne
									cdn={ctx.cdn}
									icone={`${A01}/avatar01_21/icon_item_color01.png`}
									choisie={false}
									teinte={null}
								/>
							)}
							{ouverte && c && (
								<Grille ctx={ctx} cle={`${cle}.${l.hash}`} cat={c} colonnes={6} lignes={2} />
							)}
						</div>
					);
				})}
			</>
		);
	}

	if (s.couleurDe !== undefined && cat) {
		return (
			<>
				{s.hash && <Titre cdn={ctx.cdn} t={ctx.txt(s.hash)} />}
				<Couleur ctx={ctx} cat={cat} cle={`${cle}.c${s.couleurDe}`} />
			</>
		);
	}

	return (
		<>
			{s.hash && <Titre cdn={ctx.cdn} t={ctx.txt(s.hash)} gaiji={ctx.lib(s.hash)?.gaiji} />}
			{s.grille && cat && (
				<Grille
					ctx={ctx}
					cle={`${cle}.g${s.categorie}`}
					cat={cat}
					colonnes={s.grille.colonnes}
					lignes={s.grille.lignes}
				/>
			)}
			{s.reglages && (
				<div className="flex shrink-0 flex-col gap-[2%]">
					{s.reglages.map((r, i) => (
						<Barre
							key={i}
							cdn={ctx.cdn}
							bouts={BOUTS[r.bouts]}
							valeur={ctx.valeurs[`${cle}.r${i}`] ?? 7}
							max={CURSEUR_MAX}
							onChange={(n) => ctx.setValeur(`${cle}.r${i}`, n)}
						/>
					))}
				</div>
			)}
		</>
	);
}

/** Onglet « Visage et coupe de cheveux » : le panneau de la rubrique ouverte. */
export function PanVisage({ ctx }: { ctx: Contexte }) {
	const r = RUBRIQUES_VISAGE[Math.min(ctx.rubrique, RUBRIQUES_VISAGE.length - 1)];
	if (!r) return null;
	return (
		<>
			{r.sections.map((s, i) => (
				<SectionRendue key={i} ctx={ctx} s={s} cle={`v${ctx.rubrique}.${i}`} />
			))}
			{r.aide && <Note cdn={ctx.cdn} texte={ctx.txt(r.aide)} />}
		</>
	);
}

/**
 * Onglet « Style » : les deux styles, en 2×1.
 *
 * Le jeu affiche le libellé du style choisi au-dessus des deux cartes, et pose ses silhouettes
 * `icon_ava_gender01_001` / `_002` — celles de ses propres atlas d'avatar.
 */
export function PanStyle({ ctx }: { ctx: Contexte }) {
	const hashes = [H.masculin, H.feminin];
	return (
		<>
			<Titre cdn={ctx.cdn} t={ctx.txt(H.ongletStyle)} />
			<span className="mb-[3%] flex shrink-0 justify-center">
				<Txt t={ctx.txt(hashes[ctx.genre] ?? H.masculin)} cdn={ctx.cdn} h="1.35vw" />
			</span>
			<div className="grid shrink-0 grid-cols-2 gap-[8%] px-[8%]">
				{[0, 1].map((g) => (
					<button
						key={g}
						type="button"
						onClick={() => ctx.setGenre(g)}
						className="relative flex aspect-[3/4] items-center justify-center"
					>
						{/* `gender_list01` est la carte de cet écran (objet `avatar01_52_icon_item_gender`) ;
						    `_ol` est son cadre de sélection. */}
						<Sprite
							cdn={ctx.cdn}
							src={`${A01}/avatar01_52/gender_list01${ctx.genre === g ? "_ol" : ""}.png`}
							className="absolute inset-0 size-full"
						/>
						<Txt
							t={String(g + 1).padStart(2, "0")}
							cdn={ctx.cdn}
							couleur={GRIS}
							h="14%"
							className="absolute left-[8%] top-[-8%]"
						/>
						{/* eslint-disable-next-line @next/next/no-img-element */}
						<img
							src={urlVignette(ctx.cdn, `icon_ava_gender01_00${g + 1}`) ?? ""}
							alt=""
							decoding="async"
							width={VIGNETTE_W}
							height={VIGNETTE_W}
							className="relative size-[84%] object-contain"
						/>
						{ctx.genre === g && (
							<Sprite
								cdn={ctx.cdn}
								src={`${A01}/avatar01_17/edit_check01.png`}
								className="absolute right-[2%] top-[-8%] w-[24%]"
							/>
						)}
						{ctx.genre === g && <Curseur cdn={ctx.cdn} className="absolute left-[-14%] top-[40%] h-[22%]" />}
					</button>
				))}
			</div>
		</>
	);
}

/**
 * Onglet « Physionomie » : le carrousel 3×1 des morphologies et le curseur de taille.
 *
 * Les huit morphologies sont celles de `nie_data::chara_edit::BODY_TYPES`, avec les silhouettes
 * `icon_body_type00..07` des atlas communs — huit icônes pour huit morphologies. Le curseur de
 * taille de poitrine n'apparaît que pour le style féminin, comme dans le jeu.
 */
export function PanPhysionomie({ ctx }: { ctx: Contexte }) {
	// Les deux premières entrées, `male` et `female`, sont la MÊME morphologie « Moyen » : c'est le
	// genre qui les départage. Les proposer toutes deux offrait un cran qui ne changeait rien au
	// modèle. On masque donc celle que le genre courant rend redondante.
	const redondante = ctx.genre === 1 ? 0 : 1;
	const visibles = MORPHOLOGIES.map((_, i) => i).filter((i) => i !== redondante);
	const n = visibles.length;
	const rangVisible = Math.max(0, visibles.indexOf(ctx.morphologie));
	const allerA = (d: number) => ctx.setMorphologie(visibles[(rangVisible + d + n) % n] ?? 0);
	return (
		<>
			<Titre cdn={ctx.cdn} t={ctx.txt(H.ongletPhysionomie)} />
			<div className="relative mb-[4%] flex shrink-0 items-center justify-center gap-[4%] px-[8%]">
				<button
					type="button"
					aria-label="Morphologie précédente"
					onClick={() => allerA(-1)}
					className="absolute left-[0%]"
				>
					<Sprite cdn={ctx.cdn} src={`${A01}/avatar01_11/arrow01_l.png`} className="h-[1.7vw] w-auto" />
				</button>
				{[-1, 0, 1].map((d) => {
					const i = visibles[(rangVisible + d + n) % n] ?? 0;
						// La série d'icônes commence à **01** : `icon_body_type00` n'existe dans aucun
						// atlas commun. Le nom se trouve bien dans `15_icon_common2/fr`, mais il y
						// désigne un bouton « TOUT » — d'où le pavé gris à la place de la première
						// silhouette. Vérifié en listant les 29 régions de `icon_common2.g4tx` :
						// aucune ne s'appelle ainsi.
						//
						// Deux séries de 84 × 84 cohabitent, et c'est le GENRE qui les sépare :
						// `01`…`07` sont bleues, `11`…`15` et `17` rouges. Sept silhouettes pour
						// huit entrées, parce que `male` et `female` sont la même morphologie —
						// toutes deux « Moyen » (`81E951EF`) — d'où le rang plafonné en tête.
						// La série féminine n'a pas de `16` : ce rang-là retombe sur la bleue,
						// faute d'icône, plutôt que de casser l'image.
						const rang = Math.max(1, i);
						const feminin = ctx.genre === 1 && rang !== 6;
						return (
						<Vignette
							key={d}
							cdn={ctx.cdn}
							numero={String(i + 1).padStart(2, "0")}
							image={`${ctx.cdn}${ICN}/icon_body_type${feminin ? 1 : 0}${rang}.png`}
							choisie={d === 0}
							onClick={() => ctx.setMorphologie(i)}
							className="aspect-[4/5] w-[26%]"
						/>
					);
				})}
				<button
					type="button"
					aria-label="Morphologie suivante"
					onClick={() => allerA(1)}
					className="absolute right-[0%]"
				>
					<Sprite cdn={ctx.cdn} src={`${A01}/avatar01_11/arrow01_r.png`} className="h-[1.7vw] w-auto" />
				</button>
			</div>
			<Titre cdn={ctx.cdn} t={ctx.txt(H.taille)} />
			<Barre
				cdn={ctx.cdn}
				bouts={BOUTS.moinsPlus}
				valeur={ctx.valeurs["taille"] ?? 7}
				max={CURSEUR_MAX}
				onChange={(v) => ctx.setValeur("taille", v)}
			/>
			{ctx.genre === 1 && (
				<>
					<Titre cdn={ctx.cdn} t={ctx.txt(H.taillePoitrine)} />
					<Barre
						cdn={ctx.cdn}
						bouts={BOUTS.moinsPlus}
						valeur={ctx.valeurs["poitrine"] ?? 7}
						max={CURSEUR_MAX}
						onChange={(v) => ctx.setValeur("poitrine", v)}
					/>
				</>
			)}
		</>
	);
}

/** Onglet « Habits » : col, manches, ourlet, puis l'avertissement du jeu. */
export function PanHabits({ ctx }: { ctx: Contexte }) {
	return (
		<>
			{SECTIONS_HABITS.map((s, i) => (
				<SectionRendue key={i} ctx={ctx} s={s} cle={`h${i}`} />
			))}
			<Note cdn={ctx.cdn} texte={ctx.txt(H.avertissementHabits)} alerte />
		</>
	);
}

/**
 * Onglet « Stats », rubrique « Stats de base » : éléments, positions, build.
 *
 * Les quatre éléments viennent de `chara_edit_parts_menu_status` (4×1, `icon_list_body_ability_4x1`)
 * avec les icônes `icon_cmd_type01..04` ; les six builds du même script, chacun avec son gaiji et
 * sa description. Les positions se choisissent par leur icône `icon_cmd_positionNN`.
 */
export function PanStatsBase({ ctx }: { ctx: Contexte }) {
	const element = ctx.valeurs["element"] ?? 0;
	const build = ctx.valeurs["build"] ?? 3;
	return (
		<>
			<Titre cdn={ctx.cdn} t={ctx.txt(H.elements)} />
			<div className="mb-[2%] grid shrink-0 grid-cols-4 gap-[3%]">
				{ELEMENTS.map((h, i) => (
					<button
						key={h}
						type="button"
						onClick={() => ctx.setValeur("element", i)}
						className="relative flex flex-col items-center"
					>
						<Sprite
							cdn={ctx.cdn}
							src={
								element === i
									? `${A01}/avatar01_41/cmd_type_list01_ol.png`
									: `${A01}/avatar01_41/cmd_type_list01.png`
							}
							className="absolute inset-0 size-full"
						/>
						<Sprite
							cdn={ctx.cdn}
							src={`${ICN}/icon_cmd_${element === i ? "on_" : ""}type0${i + 1}.png`}
							className="relative my-[8%] h-[1.9vw] w-auto"
						/>
						{element === i && (
							<>
								<Txt t={ctx.txt(h)} cdn={ctx.cdn} h="0.95vw" className="relative mb-[6%]" />
								<Sprite
									cdn={ctx.cdn}
									src={`${A01}/avatar01_17/edit_check01.png`}
									className="absolute right-[-6%] top-[-14%] w-[34%]"
								/>
							</>
						)}
					</button>
				))}
			</div>
			<Titre cdn={ctx.cdn} t={ctx.txt(H.positionPrincipale)} />
			<Positions ctx={ctx} cle="pos1" />
			<Titre cdn={ctx.cdn} t={ctx.txt(H.positionSecondaire)} />
			<Positions ctx={ctx} cle="pos2" />
			<Titre cdn={ctx.cdn} t={ctx.txt(H.typeBuild)} />
			<div className="mb-[2%] grid shrink-0 grid-cols-3 gap-[3%]">
				{BUILDS.map((b, i) => (
					<button
						key={b.hash}
						type="button"
						onClick={() => ctx.setValeur("build", i)}
						className="relative flex items-center justify-center py-[4%]"
					>
						<Sprite
							cdn={ctx.cdn}
							src={
								build === i
									? `${A01}/avatar01_26/category_btn_on_l.png`
									: `${A01}/avatar01_26/category_btn_off_l.png`
							}
							className="absolute inset-0 size-full"
						/>
						<TxtGaiji
							l={ctx.lib(b.hash)}
							cdn={ctx.cdn}
							couleur={build === i ? "FFFFFF" : undefined}
							h="0.95vw"
							className="relative"
						/>
					</button>
				))}
			</div>
			<Note cdn={ctx.cdn} texte={ctx.txt(BUILDS[build]?.aide ?? H.aideBuild)} />
		</>
	);
}

/**
 * Le panneau central de « Stats de base » : l'élément, le radar des sept statistiques et les
 * techniques à apprendre.
 *
 * Ce que le jeu montre ici et que rien ne permet de reproduire : les **valeurs** des sept
 * statistiques et la liste des techniques. Aucune des seize listes de `chara_edit` n'en porte —
 * elles dépendent du personnage créé, calculé ailleurs. Le jeu a lui-même un sprite pour ce cas,
 * `status_blank01` (des « ? »), et c'est lui qui est posé : la structure est juste, les valeurs
 * sont annoncées inconnues au lieu d'être inventées.
 */
export function CentreStats({ ctx }: { ctx: Contexte }) {
	const axes = ctx.catalogue.statsRadar ?? [];
	const element = ctx.valeurs["element"] ?? 0;
	return (
		<div className="flex size-full flex-col gap-[3%]">
			{/* Bandeau d'élément */}
			<div className="relative flex h-[9%] shrink-0 items-center gap-[4%] px-[4%]">
				<Sprite
					cdn={ctx.cdn}
					src={`${A01}/avatar01_05/param_plate01.png`}
					className="absolute inset-0 size-full"
				/>
				<Sprite
					cdn={ctx.cdn}
					src={`${ICN}/icon_cmd_on_type0${element + 1}.png`}
					className="relative h-[70%] w-auto"
				/>
				<Txt t={ctx.txt(ELEMENTS[element] ?? "")} cdn={ctx.cdn} h="46%" className="relative" />
			</div>

			{/* Radar : les sept axes nommés, valeurs inconnues */}
			<div className="relative grow">
				<Sprite
					cdn={ctx.cdn}
					src={`${A01}/avatar01_10/status_frame01.png`}
					className="absolute inset-0 size-full"
				/>
				<div className="relative size-full">
					{axes.map((a, i) => {
						const angle = (i / axes.length) * 2 * Math.PI - Math.PI / 2;
						const x = 50 + 38 * Math.cos(angle);
						const y = 50 + 38 * Math.sin(angle);
						return (
							<span
								key={a.hash}
								className="absolute flex -translate-x-1/2 -translate-y-1/2 flex-col items-center"
								style={{ left: `${x}%`, top: `${y}%` }}
							>
								<Sprite
									cdn={ctx.cdn}
									src={`${A01}/avatar01_10/status_blank01.png`}
									className="h-[1.1vw] w-auto"
								/>
								<Txt t={a.libelle} cdn={ctx.cdn} couleur={GRIS} h="0.85vw" />
							</span>
						);
					})}
					{/* Le treillis du radar : les axes seuls, sans surface — une surface affirmerait des
					    valeurs que rien ne fournit. */}
					<svg viewBox="0 0 100 100" className="absolute inset-0 size-full" aria-hidden>
						{axes.map((a, i) => {
							const angle = (i / axes.length) * 2 * Math.PI - Math.PI / 2;
							return (
								<line
									key={a.hash}
									x1="50"
									y1="50"
									x2={50 + 26 * Math.cos(angle)}
									y2={50 + 26 * Math.sin(angle)}
									stroke="#8FC7F5"
									strokeWidth="0.4"
								/>
							);
						})}
						<polygon
							points={axes
								.map((_, i) => {
									const angle = (i / axes.length) * 2 * Math.PI - Math.PI / 2;
									return `${50 + 26 * Math.cos(angle)},${50 + 26 * Math.sin(angle)}`;
								})
								.join(" ")}
							fill="none"
							stroke="#8FC7F5"
							strokeWidth="0.5"
						/>
					</svg>
				</div>
			</div>

			{/* Techniques à apprendre : les emplacements du jeu, valeurs inconnues */}
			<div className="relative h-[30%] shrink-0 px-[4%] py-[2%]">
				<Sprite
					cdn={ctx.cdn}
					src={`${A01}/avatar01_05/waza_plate01.png`}
					className="absolute inset-0 size-full"
				/>
				<div className="relative flex size-full flex-col justify-start gap-[4%]">
					<Txt t={ctx.txt(H.techniquesAApprendre)} cdn={ctx.cdn} couleur={GRIS} h="14%" />
					{[0, 1, 2].map((i) => (
						<Sprite
							key={i}
							cdn={ctx.cdn}
							src={`${A01}/avatar01_10/status_blank02.png`}
							className="h-[20%] w-full"
						/>
					))}
				</div>
			</div>
		</div>
	);
}

/** Les quatre positions du jeu, choisies par leur icône. */
function Positions({ ctx, cle }: { ctx: Contexte; cle: string }) {
	const choisie = ctx.valeurs[cle] ?? 0;
	return (
		<div className="mb-[2%] grid shrink-0 grid-cols-4 gap-[3%]">
			{[1, 2, 3, 4].map((n, i) => (
				<button
					key={n}
					type="button"
					onClick={() => ctx.setValeur(cle, i)}
					className="relative flex items-center justify-center py-[3%]"
				>
					<Sprite
						cdn={ctx.cdn}
						src={
							choisie === i
								? `${A01}/avatar01_30/category_btn_on_m.png`
								: `${A01}/avatar01_30/category_btn_off_m.png`
						}
						className="absolute inset-0 size-full"
					/>
					<Sprite
						cdn={ctx.cdn}
						src={`${ICN2}/icon_cmd_position0${n}.png`}
						className="relative h-[1.5vw] w-auto"
					/>
				</button>
			))}
		</div>
	);
}

/** Onglet « Stats », rubrique « Personnalité » : deux personnalités et la note du jeu. */
export function PanPersonnalite({ ctx }: { ctx: Contexte }) {
	const liste = ctx.catalogue.personnalites ?? [];
	return (
		<>
			{[H.personnalite1, H.personnalite2].map((h, k) => {
				const i = ctx.valeurs[`perso${k}`] ?? k;
				const p = liste[i % Math.max(1, liste.length)];
				return (
					<div key={h} className="shrink-0">
						<Titre cdn={ctx.cdn} t={ctx.txt(h)} />
						<button
							type="button"
							onClick={() => ctx.setValeur(`perso${k}`, (i + 1) % Math.max(1, liste.length))}
							className="relative flex h-[2.6vw] w-full items-center justify-center"
						>
							<Sprite
								cdn={ctx.cdn}
								src={`${A01}/avatar01_50/type_base01_off.png`}
								className="absolute inset-0 size-full"
							/>
							<Txt
								t={String((i % Math.max(1, liste.length)) + 1).padStart(2, "0")}
								cdn={ctx.cdn}
								couleur={GRIS}
								h="1vw"
								className="relative mr-[2%]"
							/>
							<Txt t={p?.libelle ?? ""} cdn={ctx.cdn} h="1.15vw" className="relative" />
						</button>
					</div>
				);
			})}
			<Note cdn={ctx.cdn} texte={ctx.txt(H.aidePersonnalite)} />
		</>
	);
}

/**
 * Onglet « Stats », rubrique « Voix » : style, personnalité de voix, type.
 *
 * Les trois champs sont ceux du jeu, et la combinaison désigne une banque du catalogue
 * (`voix[]` : genre, personnalité, ton) — c'est elle que le bouton d'écoute joue.
 */
export function PanVoix({
	ctx,
	jouer,
	enCours,
}: {
	ctx: Contexte;
	jouer: (banque: string) => void;
	enCours: string | null;
}) {
	const perso = ctx.valeurs["voixPerso"] ?? 0;
	const ton = ctx.valeurs["voixTon"] ?? 0;
	const banque = useMemo(() => {
		const v = ctx.catalogue.voix ?? [];
		return (
			v.find((b) => b.genre === ctx.genre && b.personnalite === perso && b.ton === ton) ??
			v.find((b) => b.genre === ctx.genre && b.personnalite === perso) ??
			v[0]
		);
	}, [ctx.catalogue.voix, ctx.genre, perso, ton]);

	const champs = [
		{ hash: H.ongletStyle, texte: ctx.txt([H.masculin, H.feminin][ctx.genre] ?? H.masculin) },
		{
			hash: H.personnalite,
			texte: ctx.txt(PERSONNALITES_VOIX[perso % PERSONNALITES_VOIX.length] ?? ""),
			suivant: () => ctx.setValeur("voixPerso", (perso + 1) % PERSONNALITES_VOIX.length),
		},
		{
			hash: H.type,
			texte: ctx.txt(TYPES_VOIX[ton % TYPES_VOIX.length] ?? ""),
			suivant: () => ctx.setValeur("voixTon", (ton + 1) % TYPES_VOIX.length),
		},
	];

	return (
		<>
			{champs.map((c) => (
				<div key={c.hash} className="shrink-0">
					<Titre cdn={ctx.cdn} t={ctx.txt(c.hash)} />
					<button
						type="button"
						onClick={() => {
							c.suivant?.();
							if (banque) jouer(banque.banque);
						}}
						className="relative flex h-[2.6vw] w-full items-center justify-center"
					>
						<Sprite
							cdn={ctx.cdn}
							src={
								enCours && banque && enCours === banque.banque
									? `${A01}/avatar01_50/type_base01_on.png`
									: `${A01}/avatar01_50/type_base01_off.png`
							}
							className="absolute inset-0 size-full"
						/>
						<Txt t={c.texte} cdn={ctx.cdn} h="1.15vw" className="relative" />
						{enCours && banque && enCours === banque.banque && (
							<Sprite
								cdn={ctx.cdn}
								src={`${A01}/avatar01_48/icon_sound01.png`}
								className="absolute right-[4%] h-[54%] w-auto"
							/>
						)}
					</button>
				</div>
			))}
		</>
	);
}

/**
 * Onglet « Nom » : les quatre champs du jeu, sur `edit_name_base01`.
 *
 * Le numéro de maillot est un champ numérique dans le jeu ; les trois autres sont textuels. Les
 * messages d'aide correspondants existent dans `chara_edit_parts_menu_name` et s'affichent en bas
 * de l'écran, hors du panneau.
 */
export function PanNom({
	ctx,
	champs,
	setChamp,
}: {
	ctx: Contexte;
	champs: Record<string, string>;
	setChamp: (cle: string, v: string) => void;
}) {
	const liste = [
		{ hash: H.nom, cle: "nom", numerique: false },
		{ hash: H.surnom, cle: "surnom", numerique: false },
		{ hash: H.nomUniforme, cle: "uniforme", numerique: false },
		{ hash: H.numeroMaillot, cle: "numero", numerique: true },
	];
	return (
		<>
			{liste.map((c) => (
				<div key={c.cle} className="shrink-0">
					<Titre cdn={ctx.cdn} t={ctx.txt(c.hash)} />
					<div className="relative flex h-[2.9vw] w-full items-center">
						<Sprite
							cdn={ctx.cdn}
							src={`${A01}/avatar01_44/name_base01_off.png`}
							className="absolute inset-0 size-full"
						/>
						<input
							value={champs[c.cle] ?? ""}
							onChange={(e) => setChamp(c.cle, e.target.value)}
							inputMode={c.numerique ? "numeric" : "text"}
							maxLength={c.numerique ? 2 : 16}
							className="relative size-full bg-transparent text-center text-[1.15vw] text-[#047AFF] outline-none"
						/>
					</div>
				</div>
			))}
		</>
	);
}

/** Le curseur vert du jeu, posé à gauche d'un élément — réexporté pour le cadre. */
export { Curseur };
