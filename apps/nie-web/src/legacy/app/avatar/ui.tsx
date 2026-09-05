"use client";

/**
 * Les briques de l'interface de l'éditeur, chacune sur le sprite du jeu qui la porte.
 *
 * Rien n'est dessiné en CSS qui existe en sprite. Les sprites viennent de `menu/161_avatar/avatar01`
 * (l'atlas de l'éditeur) et de `menu/200_icon/15_icon_common` ; les textes sont composés par
 * `/ui/text.png` dans la police bitmap du jeu.
 *
 * | brique | sprite |
 * |---|---|
 * | titre de section | `edit_line01` (filet) + `edit_win_iconNN` (pictogramme) |
 * | vignette de grille | `preset_list01`, sélection `preset_list01_ol`, coche `edit_check01` |
 * | curseur | `edit_bar_base02_on`, `edit_bar_gauge_base01`, `edit_bar_gauge01_on`, bouts `edit_bar_iconNN_on` |
 * | dégradé de couleur | `edit_bar_color01` |
 * | ligne de sélecteur | `type_icon_list02_l` (repos) / `type_icon_list01_l_on` (choisie) |
 * | case vide | `icon_none02` |
 * | pagination | `arrow01_l` / `arrow01_r` |
 *
 * Les **bouts de curseur** ont été identifiés en planche : `01` −, `02` +, `03` rétrécir,
 * `04` agrandir, `05` ↓, `06` ↑, `07` rapprocher, `08` écarter, `09` ↺, `10` ↻, `11` comprimer,
 * `12` étirer, `13` ◄, `14` ►. Les paires employées ci-dessous sont celles que les captures du jeu
 * montrent pour chaque curseur.
 */

import type { Libelle } from "./libelles";

/** Racine des sprites de l'éditeur dans le dump servi par le CDN. */
export const A01 = "/dx11/menu/161_avatar/avatar01";
/** Icônes communes (builds, éléments, morphologies 01 à 07). */
export const ICN = "/dx11/menu/200_icon/15_icon_common";
/**
 * Second atlas d'icônes communes, **localisé** : le jeu y range ce qui porte du texte dessiné —
 * les pastilles de position (`icon_cmd_positionNN`, où « DÉF » et « MIL » sont peints dans
 * l'image) et la morphologie `00`. Une copie par langue existe ; celle-ci est la française.
 */
export const ICN2 = "/dx11/menu/200_icon/15_icon_common2/fr";

/** Gaiji : les glyphes images de la police du jeu. */
export const FONT = "/dx11/font";

/** Conteneur des vignettes de l'éditeur dans les fichiers du jeu. */
export const ATLAS_AVATAR = "/g4tx/dx11/menu/200_icon/21_icon_avatar";
/** Largeur servie : la case de grille fait ~134 px, 200 px la couvre en écran dense. */
export const VIGNETTE_W = 200;

/**
 * URL d'une vignette de l'éditeur, en WebP redimensionné et mis en cache sur disque.
 *
 * Une vignette n'est pas un fichier : c'est une TEXTURE NOMMÉE d'un conteneur `.g4tx` —
 * `icon_ava_face04_001` vit dans `icon_ava_face04.g4tx`, et les ~493 vignettes du catalogue ne
 * viennent que d'une douzaine de conteneurs. La route `/avatar/icon/<nom>.png` les décode au
 * serveur d'assets ; la surface `/g4tx/…?w=` passe par `cdn-variants`, qui redimensionne une
 * fois, écrit le WebP sur disque et le ressert sous `immutable` — le même chemin que la
 * galerie. Une page complète y gagne ~1,5 Mo.
 */
export function urlVignette(cdn: string, icone: string | null): string | null {
	if (!icone) return null;
	const coupe = icone.lastIndexOf("_");
	if (coupe <= 0) return null;
	const atlas = icone.slice(0, coupe);
	return `${cdn}${ATLAS_AVATAR}/${atlas}.g4tx/${icone}.png?w=${VIGNETTE_W}&format=webp`;
}

/** Couleur du texte de l'éditeur, échantillonnée sur les captures. */
export const BLEU = "047AFF";
/** Couleur des messages d'avertissement du jeu (texte rouge de l'écran Habits). */
export const ROUGE = "E8384A";
/** Gris des numéros de vignette. */
export const GRIS = "9AA6B2";

/** Un texte composé dans la police bitmap du jeu. */
export function Txt({
	t,
	cdn,
	couleur = BLEU,
	h,
	className,
	style,
}: {
	t: string;
	cdn: string;
	couleur?: string;
	h: string;
	className?: string;
	style?: React.CSSProperties;
}) {
	if (!t) return null;
	return (
		// eslint-disable-next-line @next/next/no-img-element
		<img
			src={`${cdn}/ui/text.png?t=${encodeURIComponent(t)}&fg=${couleur}`}
			alt={t}
			loading="lazy"
			decoding="async"
			style={{ height: h, ...style }}
			className={`
     w-auto max-w-full object-contain
     ${className ?? ""}
   `}
		/>
	);
}

/** Un sprite du jeu, posé tel quel. */
export function Sprite({
	cdn,
	src,
	className,
	style,
}: {
	cdn: string;
	src: string;
	className?: string;
	style?: React.CSSProperties;
}) {
	return (
		// eslint-disable-next-line @next/next/no-img-element
		<img src={`${cdn}${src}`} alt="" aria-hidden className={className} style={style} />
	);
}

/**
 * Un libellé du jeu avec ses gaiji : les glyphes images précèdent le texte, comme dans le jeu
 * (`[$gaiji_icon_build04] Tension`). Les 508 gaiji sont des régions des atlas de `font/`, servis
 * sous leur nom complet — `gaiji_icon_build04`, et non `icon_build04` des atlas d'icônes, qui est
 * un autre dessin.
 */
export function TxtGaiji({
	l,
	cdn,
	couleur = BLEU,
	h,
	className,
}: {
	l: Libelle | undefined;
	cdn: string;
	couleur?: string;
	h: string;
	className?: string;
}) {
	if (!l) return null;
	return (
		<span className={`
    inline-flex items-center gap-[0.4em]
    ${className ?? ""}
  `}>
			{l.gaiji.map((g) => (
				<Sprite
					key={g}
					cdn={cdn}
					src={`${FONT}/${g}.png`}
					style={{ height: h }}
					className="w-auto"
				/>
			))}
			<Txt t={l.libelle} cdn={cdn} couleur={couleur} h={h} />
		</span>
	);
}

/** Titre de section du panneau : le filet bleu vertical du jeu, puis le libellé. */
export function Titre({ cdn, t, gaiji }: { cdn: string; t: string; gaiji?: string[] }) {
	if (!t) return null;
	return (
		<span className="mb-[0.5%] mt-[1.2%] flex shrink-0 items-center gap-[0.6vw]">
			<span
				className="inline-block shrink-0"
				style={{ width: "0.22vw", height: "1.2vw", background: `#${BLEU}` }}
			/>
			<TxtGaiji l={{ libelle: t, gaiji: gaiji ?? [] }} cdn={cdn} h="1.15vw" />
		</span>
	);
}

/** Une vignette de grille : cadre, numéro, image, coche et curseur de sélection. */
export function Vignette({
	cdn,
	numero,
	image,
	choisie,
	verrouillee = false,
	onClick,
	className,
}: {
	cdn: string;
	numero?: string;
	image: string | null;
	choisie: boolean;
	verrouillee?: boolean;
	onClick?: () => void;
	className?: string;
}) {
	return (
		<button
			type="button"
			onClick={onClick}
			disabled={verrouillee}
			className={`
     relative flex items-center justify-center
     ${className ?? ""}
   `}
		>
			<Sprite
				cdn={cdn}
				src={choisie ? `${A01}/avatar01_17/preset_list01_ol.png` : `${A01}/avatar01_17/preset_list01.png`}
				className="absolute inset-0 size-full"
			/>
			{numero && (
				<Txt
					t={numero}
					cdn={cdn}
					couleur={GRIS}
					h="26%"
					className="absolute left-[8%] top-[-16%]"
				/>
			)}
			{image ? (
				// eslint-disable-next-line @next/next/no-img-element
				<img
					src={image}
					alt=""
					loading="lazy"
					decoding="async"
					width={VIGNETTE_W}
					height={VIGNETTE_W}
					className="relative size-[86%] object-contain"
				/>
			) : (
				<Sprite cdn={cdn} src={`${A01}/avatar01_17/icon_none02.png`} className="relative w-[52%]" />
			)}
			{choisie && (
				<>
					<Sprite
						cdn={cdn}
						src={`${A01}/avatar01_17/edit_check01.png`}
						className="absolute right-[2%] top-[-14%] w-[30%]"
					/>
					<Curseur cdn={cdn} className="absolute left-[-22%] top-[26%] h-[46%]" />
				</>
			)}
		</button>
	);
}

/** Le curseur vert du jeu, qui marque l'élément sous la main du joueur. */
export function Curseur({ cdn, className }: { cdn: string; className?: string }) {
	return <Sprite cdn={cdn} src="/dx11/menu/20_cmn/cmn05/cmn05_01/cur01.png" className={`
   w-auto
   ${className ?? ""}
 `} />;
}

/** Bouts de curseur, par rôle — les paires que les captures du jeu montrent. */
export const BOUTS = {
	moinsPlus: ["01", "02"],
	largeur: ["07", "08"],
	longueur: ["11", "12"],
	verticale: ["05", "06"],
	echelle: ["03", "04"],
	rotation: ["09", "10"],
	couleur: ["13", "14"],
} as const;

/**
 * Un curseur du jeu : deux bouts, la gouttière, la jauge remplie et la valeur.
 *
 * `degrade` remplace la jauge par `edit_bar_color01`, la bande de dégradé que le jeu emploie pour
 * la teinte ; `pisteFond` la remplace par une bande CSS quand le jeu peint une rampe calculée
 * (saturation, luminosité) — le sprite du jeu n'en contient qu'une, celle de la teinte.
 */
export function Barre({
	cdn,
	bouts,
	valeur,
	max,
	onChange,
	degrade = false,
	pisteFond,
}: {
	cdn: string;
	bouts: readonly [string, string] | readonly string[];
	valeur: number;
	max: number;
	onChange: (v: number) => void;
	degrade?: boolean;
	pisteFond?: string;
}) {
	const part = max > 0 ? Math.max(0, Math.min(1, valeur / max)) : 0;
	return (
		<div className="flex w-full items-center gap-[3%]">
			<div className="relative flex h-[2.1vw] grow items-center">
				<Sprite
					cdn={cdn}
					src={`${A01}/avatar01_13/edit_bar_base02_on.png`}
					className="absolute inset-0 size-full"
				/>
				<button
					type="button"
					aria-label="Diminuer"
					onClick={() => onChange(Math.max(0, valeur - 1))}
					className="relative ml-[3%] h-[62%]"
				>
					<Sprite
						cdn={cdn}
						src={`${A01}/avatar01_13/edit_bar_icon${bouts[0]}_on.png`}
						className="h-full w-auto"
					/>
				</button>
				<div className="relative mx-[3%] h-[26%] grow">
					{degrade ? (
						<Sprite
							cdn={cdn}
							src={`${A01}/avatar01_13/edit_bar_color01.png`}
							className="absolute inset-0 size-full"
						/>
					) : (
						<>
							<Sprite
								cdn={cdn}
								src={`${A01}/avatar01_13/edit_bar_gauge_base01.png`}
								className="absolute inset-0 size-full"
								style={pisteFond ? { opacity: 0 } : undefined}
							/>
							{pisteFond && (
								<span
									className="absolute inset-0 rounded-full"
									style={{ background: pisteFond }}
								/>
							)}
							<Sprite
								cdn={cdn}
								src={`${A01}/avatar01_13/edit_bar_gauge01_on.png`}
								className="absolute inset-y-0 left-0"
								style={{ width: `${part * 100}%` }}
							/>
						</>
					)}
					{/* Poignée : le jeu en pose une ronde blanche sur la jauge. */}
					<span
						className="
        absolute top-1/2 size-[0.95vw] -translate-x-1/2 -translate-y-1/2 rounded-full border-[0.1vw] border-[#9AC7F5]
        bg-white
      "
						style={{ left: `${part * 100}%` }}
					/>
					<input
						type="range"
						min={0}
						max={max}
						value={valeur}
						onChange={(e) => onChange(Number(e.target.value))}
						className="absolute inset-0 size-full cursor-pointer opacity-0"
						aria-label="Valeur"
					/>
				</div>
				<button
					type="button"
					aria-label="Augmenter"
					onClick={() => onChange(Math.min(max, valeur + 1))}
					className="relative mr-[3%] h-[62%]"
				>
					<Sprite
						cdn={cdn}
						src={`${A01}/avatar01_13/edit_bar_icon${bouts[1]}_on.png`}
						className="h-full w-auto"
					/>
				</button>
			</div>
			<Txt t={String(valeur)} cdn={cdn} h="1.5vw" className="w-[8%] shrink-0" />
		</div>
	);
}

/**
 * Une ligne de sélecteur : icône de section à gauche, vignette de la valeur à droite.
 *
 * C'est la forme des rubriques « Yeux », « Extras » et des couleurs (« Couleur des lèvres ») :
 * le jeu n'y montre pas de grille mais une ligne par sous-choix, qui ouvre la grille au clic.
 */
export function Ligne({
	cdn,
	icone,
	numero,
	image,
	choisie,
	onClick,
	teinte,
}: {
	cdn: string;
	icone: string | null;
	numero?: string;
	image?: string | null;
	choisie: boolean;
	onClick?: () => void;
	/** Pastille de couleur, quand la ligne porte une couleur et non une part. */
	teinte?: string | null;
}) {
	return (
		<button
			type="button"
			onClick={onClick}
			className="relative mb-[1.5%] flex h-[3.1vw] w-full shrink-0 items-center"
		>
			<Sprite
				cdn={cdn}
				src={
					choisie
						? `${A01}/avatar01_26/type_icon_list01_l_on.png`
						: `${A01}/avatar01_21/type_icon_list02_l.png`
				}
				className="absolute inset-0 size-full"
			/>
			{icone && (
				<Sprite cdn={cdn} src={icone} className="relative ml-[6%] h-[56%] w-auto" />
			)}
			<span className="relative ml-auto mr-[8%] flex h-[76%] items-center">
				{numero && (
					<Txt t={numero} cdn={cdn} couleur={GRIS} h="34%" className="mr-[-1.4vw] mt-[-1vw]" />
				)}
				{teinte !== undefined ? (
					<span className="relative flex size-[2.2vw] items-center justify-center">
						<Sprite
							cdn={cdn}
							src={`${A01}/avatar01_21/icon_item_color01.png`}
							className="absolute inset-0 size-full"
						/>
						{teinte ? (
							<span
								className="relative size-[62%] rounded-[0.15vw]"
								style={{ background: teinte }}
							/>
						) : (
							<Sprite
								cdn={cdn}
								src={`${A01}/avatar01_17/icon_none02.png`}
								className="relative w-[62%]"
							/>
						)}
					</span>
				) : (
					<span className="relative flex size-[2.4vw] items-center justify-center">
						<Sprite
							cdn={cdn}
							src={`${A01}/avatar01_17/preset_list01.png`}
							className="absolute inset-0 size-full"
						/>
						{image ? (
							// eslint-disable-next-line @next/next/no-img-element
							<img
								src={image}
								alt=""
								loading="lazy"
								decoding="async"
								width={VIGNETTE_W}
								height={VIGNETTE_W}
								className="relative size-[84%] object-contain"
							/>
						) : (
							<Sprite
								cdn={cdn}
								src={`${A01}/avatar01_17/icon_none02.png`}
								className="relative w-[56%]"
							/>
						)}
					</span>
				)}
			</span>
		</button>
	);
}

/**
 * Une note d'aide du panneau, sur le sprite `memo_base01` du jeu (l'objet `avatar01_45_memo_text`),
 * précédée de son pictogramme : `gaiji_system01` pour l'information, `gaiji_system02` (triangle
 * rouge) pour l'avertissement.
 */
export function Note({
	cdn,
	texte,
	alerte = false,
}: {
	cdn: string;
	texte: string;
	alerte?: boolean;
}) {
	if (!texte) return null;
	return (
		<div className="relative mt-auto flex shrink-0 items-center gap-[3%] px-[4%] py-[2.5%]">
			<Sprite
				cdn={cdn}
				src={`${A01}/avatar01_45/memo_base01.png`}
				className="absolute inset-0 size-full"
			/>
			<Sprite
				cdn={cdn}
				src={`${FONT}/${alerte ? "gaiji_system02" : "gaiji_system01"}.png`}
				className="relative h-[1.6vw] w-auto shrink-0"
			/>
			<span className="relative flex grow flex-col items-center gap-[0.15vw]">
				{texte.split("\n").map((ligne, i) => (
					<Txt key={i} t={ligne} cdn={cdn} couleur={alerte ? ROUGE : BLEU} h="1.05vw" />
				))}
			</span>
		</div>
	);
}

/** Les chevrons de pagination du jeu, avec les badges de touche qu'il affiche dessous. */
export function Pagination({
	cdn,
	page,
	pages,
	setPage,
}: {
	cdn: string;
	page: number;
	pages: number;
	setPage: (n: number) => void;
}) {
	if (pages <= 1) return null;
	return (
		<>
			<button
				type="button"
				aria-label="Page précédente"
				onClick={() => setPage((page - 1 + pages) % pages)}
				className="absolute left-[-1%] top-1/2 -translate-y-1/2"
			>
				<Sprite cdn={cdn} src={`${A01}/avatar01_11/arrow01_l.png`} className="h-[1.7vw] w-auto" />
			</button>
			<button
				type="button"
				aria-label="Page suivante"
				onClick={() => setPage((page + 1) % pages)}
				className="absolute right-[-1%] top-1/2 -translate-y-1/2"
			>
				<Sprite cdn={cdn} src={`${A01}/avatar01_11/arrow01_r.png`} className="h-[1.7vw] w-auto" />
			</button>
		</>
	);
}
