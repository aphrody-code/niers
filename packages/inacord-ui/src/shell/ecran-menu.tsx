/**
 * Les formes de l'ecran de menu principal : panneaux, tuiles biseautees, plaque, bandeau.
 *
 * ## Pourquoi ces composants sont poses en coordonnees du canevas
 *
 * L'ecran du jeu se decrit en pixels d'un canevas 1280x720. Tout ce fichier travaille dans ce
 * repere, et [`GameCanvas`] se charge de la mise a l'echelle. Une position exportee se pose
 * donc telle quelle, sans conversion — et une conversion repetee dans dix composants finit
 * toujours par diverger dans l'un d'eux.
 *
 * ## Ce qu'ils savent, et ce qu'ils ne savent pas
 *
 * Ils ne dessinent que des FORMES : aucune donnee, aucune source, aucun hote. Les couleurs
 * viennent de `game-tokens.css`, mesurees sur la reference archivee ; les pictogrammes sont des
 * traces geometriques de ce depot, pas des images du jeu.
 *
 * **Les positions, elles, ne sont pas mesurees dans le jeu** : l'export de layout ne donne pas
 * la place des widgets du menu (24 objets sur 34 restent au centre par defaut). L'appelant les
 * pose donc lui-meme, et c'est une RECONSTRUCTION lue sur la reference, jamais une mesure du
 * binaire. Le dire ici evite qu'un lecteur prenne la seconde pour la premiere.
 */
import type { CSSProperties, ReactNode } from "react";
import { biseau, BOITES, LARGEUR_TUILE, PENTE_PANNEAU } from "./geometrie-mainmenu";

/** Fusionne des classes en ignorant les vides. */
function cx(...parties: (string | false | null | undefined)[]): string {
	return parties.filter(Boolean).join(" ");
}

/**
 * Pose un enfant dans le repere du canevas.
 *
 * `ancreX`/`ancreY` suivent la MEME convention que `transform.anchorX` de l'export : une
 * fraction de la taille de l'element, `0` en haut a gauche, `0.5` au centre, `1` en bas a
 * droite. Une seule convention de placement dans tout le fichier, celle de la donnee.
 */
export function CanvasItem({
	x,
	y,
	largeur,
	hauteur,
	ancreX = 0,
	ancreY = 0,
	z,
	children,
	style,
}: {
	x: number;
	y: number;
	largeur?: number;
	hauteur?: number;
	ancreX?: number;
	ancreY?: number;
	z?: number;
	children: ReactNode;
	style?: CSSProperties;
}) {
	return (
		<div
			style={{
				position: "absolute",
				left: `${x}px`,
				top: `${y}px`,
				// `max-content` et non `auto` : un bloc absolu sans largeur se contente de la place
				// restante jusqu'au bord du canevas. Pose a x=640, une rangee de 782 px n'en obtient
				// que 640 et se tasse ; pose a droite avec `ancreX=1`, une etiquette n'obtient que
				// quelques pixels et passe a la ligne, lettre par lettre. Les deux defauts sont
				// silencieux : rien n'indique que la largeur a ete decidee par le bord.
				width: largeur !== undefined ? `${largeur}px` : "max-content",
				height: hauteur !== undefined ? `${hauteur}px` : undefined,
				transform:
					ancreX || ancreY ? `translate(${-ancreX * 100}%, ${-ancreY * 100}%)` : undefined,
				zIndex: z,
				...style,
			}}
		>
			{children}
		</div>
	);
}

/**
 * Le parallelogramme du menu, en `clip-path`.
 *
 * Le HAUT est decale vers la droite : la pente mesuree vaut `dx/dy = -0,4` (§
 * [`PENTE_TUILE`]), c'est-a-dire qu'un bord part vers la gauche en descendant. La premiere
 * version de ce fichier penchait dans l'autre sens — un ecart qu'on ne voit pas en lisant le
 * code, seulement en posant le rendu a cote de la capture.
 */
function coupe(penche: number): CSSProperties {
	return {
		clipPath: `polygon(${penche}px 0, 100% 0, calc(100% - ${penche}px) 100%, 0 100%)`,
	};
}

/**
 * Un panneau lateral : UN seul bord est biseaute, l'autre longe le bord de l'ecran.
 *
 * Les deux panneaux du menu ne sont pas des parallelogrammes complets — leur bord exterieur est
 * vertical, colle au bord de l'ecran, et seul le bord interieur penche (vers l'exterieur en
 * descendant, `PENTE_PANNEAU`, l'inverse des tuiles).
 */
function coupePanneau(penche: number, cote: "gauche" | "droite"): CSSProperties {
	return {
		clipPath:
			cote === "gauche"
				? `polygon(0 0, calc(100% - ${penche}px) 0, 100% 100%, 0 100%)`
				: `polygon(${penche}px 0, 100% 0, 100% 100%, 0 100%)`,
	};
}

/**
 * Une tuile du menu : parallelogramme bleu, pictogramme centre, libelle en dessous.
 *
 * L'etat courant se lit SANS la couleur — liseré clair, elevation, et `aria-current`. Une
 * interface qui ne signale la selection que par la teinte exclut qui ne la distingue pas, et
 * c'est exactement le cas d'une rangee de huit tuiles toutes bleues.
 */
export function IconTile({
	icone,
	libelle,
	appoint,
	actif = false,
	sourdine = false,
	onClick,
	largeur = LARGEUR_TUILE,
	hauteur = BOITES.rangee.h,
	penche = biseau(BOITES.rangee.h),
	className,
}: {
	icone: ReactNode;
	libelle: string;
	/** Un compte ou une precision, affiche sous le libelle. */
	appoint?: ReactNode;
	actif?: boolean;
	sourdine?: boolean;
	onClick?: () => void;
	largeur?: number;
	hauteur?: number;
	penche?: number;
	className?: string;
}) {
	const interactif = Boolean(onClick) && !sourdine;
	return (
		<button
			type="button"
			onClick={interactif ? onClick : undefined}
			disabled={sourdine}
			aria-current={actif ? "true" : undefined}
			className={cx("jeu-tuile-icone", actif && "jeu-tuile-icone--actif", className)}
			style={{
				...coupe(penche),
				width: largeur,
				height: hauteur,
				border: 0,
				padding: 0,
				display: "flex",
				flexDirection: "column",
				alignItems: "center",
				justifyContent: "center",
				gap: 4,
				background: actif
					? "linear-gradient(180deg, var(--jeu-tuile-active-haut), var(--jeu-tuile-active-bas))"
					: "linear-gradient(180deg, var(--jeu-tuile-haut), var(--jeu-tuile-bas))",
				color: "var(--jeu-texte-vif)",
				font: "inherit",
				fontWeight: 800,
				fontSize: 13,
				letterSpacing: "var(--jeu-libelle-espacement)",
				textShadow: "0 1px 2px rgb(10 47 102 / 85%)",
				cursor: interactif ? "pointer" : sourdine ? "not-allowed" : "default",
				opacity: sourdine ? 0.45 : 1,
				outline: actif ? "2px solid var(--jeu-surface-glace)" : "none",
				outlineOffset: -2,
				transform: actif ? "translateY(-3px)" : "none",
				boxShadow: actif ? "var(--jeu-lueur-accent)" : "var(--jeu-ombre-tuile)",
				transition:
					"transform var(--jeu-duree-rapide) var(--jeu-courbe), background var(--jeu-duree-rapide) var(--jeu-courbe)",
			}}
		>
			<span aria-hidden="true" style={{ display: "block", lineHeight: 0 }}>
				{icone}
			</span>
			<span>{libelle}</span>
			{appoint ? (
				<span style={{ fontWeight: 700, fontSize: 11, opacity: 0.9 }}>{appoint}</span>
			) : null}
		</button>
	);
}

/** Une rangee de tuiles, centree dans le canevas comme dans le jeu. */
export function TileStrip({ children, ecart = 8 }: { children: ReactNode; ecart?: number }) {
	return (
		<div style={{ display: "flex", justifyContent: "center", alignItems: "flex-start", gap: ecart }}>
			{children}
		</div>
	);
}

/**
 * Un grand panneau lateral, biseaute vers le centre de l'ecran.
 *
 * `cote` ne fait pas que retourner la coupe : il place aussi le titre du cote exterieur, la ou
 * l'oeil arrive.
 */
export function HeroPanel({
	titre,
	cote,
	children,
	onClick,
	hauteur = BOITES.panneaux.h,
	penche = Math.round(PENTE_PANNEAU * BOITES.panneaux.h),
}: {
	titre: string;
	cote: "gauche" | "droite";
	children?: ReactNode;
	onClick?: () => void;
	hauteur?: number;
	/** Le decalage du bord interieur entre le haut et le bas, en pixels du canevas. */
	penche?: number;
}) {
	const gauche = cote === "gauche";
	const Balise = onClick ? "button" : "div";
	return (
		<Balise
			type={onClick ? "button" : undefined}
			onClick={onClick}
			style={{
				...coupePanneau(penche, cote),
				position: "relative",
				display: "block",
				width: "100%",
				height: hauteur,
				border: 0,
				padding: 0,
				textAlign: gauche ? "left" : "right",
				// Le degrade va du clair (bord EXTERIEUR) au bleu (bord INTERIEUR), et il part de
				// `--jeu-surface-glace` et non de `--jeu-ciel-clair` : ce dernier (#f9fcfc) est a
				// une distance de 4 du fond de l'ecran (#f9fdf9), donc INDISCERNABLE. Mesure a
				// l'appui : le panneau gauche, cense commencer a x = 0, n'etait detecte qu'a
				// x = 28 — ses 28 premiers pixels existaient sans se voir, des deux cotes du
				// bord. Un panneau qui s'eteint dans le fond n'a plus de forme a comparer.
				background: gauche
					? "linear-gradient(100deg, var(--jeu-surface-glace) 0%, var(--jeu-ciel-brume) 100%)"
					: "linear-gradient(260deg, var(--jeu-surface-glace) 0%, var(--jeu-ciel-brume) 100%)",
				color: "var(--jeu-nuit-profonde)",
				font: "inherit",
				cursor: onClick ? "pointer" : "default",
				boxShadow: "var(--jeu-ombre-tuile)",
			}}
		>
			{/* Le titre longe le bord EXTERIEUR — celui qui est vertical — donc rien ne le rogne :
			    28 px suffisent, et c'est la marge mesuree sur la capture (« AVATAR » commence a
			    x = 28 du canevas, « VOTRE EQUIPE » finit a 26 du bord droit). Le poser du cote
			    interieur le ferait tomber dans le biseau de 98 px, sans qu'aucun message ne le
			    signale : le texte serait toujours la, simplement coupe. */}
			<span
				style={{
					position: "absolute",
					bottom: 8,
					left: gauche ? 28 : undefined,
					right: gauche ? undefined : 28,
					fontSize: 38,
					fontWeight: 800,
					letterSpacing: "0.12em",
					textTransform: "uppercase",
					color: "var(--jeu-texte-vif)",
					textShadow: "0 2px 6px rgb(10 47 102 / 80%)",
				}}
			>
				{titre}
			</span>
			{children}
		</Balise>
	);
}

/** La plaque centrale : un libelle en petit, une valeur en grand. */
export function CenterPlate({ libelle, valeur }: { libelle: string; valeur: ReactNode }) {
	return (
		<div style={{ textAlign: "center", color: "var(--jeu-texte-vif)" }}>
			<div
				style={{
					fontSize: 17,
					fontWeight: 800,
					letterSpacing: "0.18em",
					textTransform: "uppercase",
					color: "var(--jeu-accent-azur)",
					textShadow: "0 1px 0 var(--jeu-texte-vif)",
				}}
			>
				{libelle}
			</div>
			<div
				style={{
					...coupe(14),
					marginTop: 2,
					padding: "2px 22px",
					background:
						"linear-gradient(180deg, var(--jeu-plaque-bleu), var(--jeu-nuit-profonde))",
					fontSize: 34,
					fontWeight: 800,
					lineHeight: 1.1,
					letterSpacing: "0.06em",
				}}
			>
				{valeur}
			</div>
		</div>
	);
}

/** Le bandeau bleu qui coupe l'ecran sous la rangee de tuiles. */
export function RibbonBand({ children }: { children: ReactNode }) {
	return (
		<div
			style={{
				...coupe(16),
				display: "flex",
				alignItems: "center",
				justifyContent: "center",
				gap: "var(--jeu-espace-s)",
				height: "100%",
				padding: "0 var(--jeu-espace-l)",
				background:
					"linear-gradient(180deg, var(--jeu-tuile-active-haut), var(--jeu-tuile-active-bas))",
				color: "var(--jeu-texte-vif)",
				fontWeight: 800,
				fontSize: 19,
				letterSpacing: "0.04em",
				// Le bandeau est biseaute : un texte qui passe a la ligne deborde de la coupe et se
				// fait rogner par le bas. Il tient sur une ligne, ou l'appelant le raccourcit.
				whiteSpace: "nowrap",
				textShadow: "0 1px 3px rgb(10 47 102 / 85%)",
				boxShadow: "var(--jeu-ombre-tuile)",
			}}
		>
			{children}
		</div>
	);
}

/** L'encart d'information du coin haut-gauche, et son bouton. */
export function NoticeCard({
	titre,
	lignes,
	bouton,
	onClick,
}: {
	titre: string;
	lignes: string[];
	bouton?: string;
	onClick?: () => void;
}) {
	return (
		<div>
			<div
				style={{
					...coupe(12),
					padding: "10px 18px 12px",
					background: "linear-gradient(180deg, var(--jeu-ciel-clair), var(--jeu-ciel-brume))",
					border: "2px solid var(--jeu-nuit-profonde)",
					color: "var(--jeu-nuit-profonde)",
				}}
			>
				<div
					style={{
						fontWeight: 800,
						fontSize: 14,
						color: "var(--jeu-accent-brique)",
						letterSpacing: "0.02em",
					}}
				>
					{titre}
				</div>
				{lignes.map((ligne) => (
					<div key={ligne} style={{ fontSize: 12, fontWeight: 700 }}>
						{ligne}
					</div>
				))}
			</div>
			{bouton ? (
				<button
					type="button"
					onClick={onClick}
					style={{
						...coupe(12),
						marginTop: 4,
						width: "100%",
						height: 32,
						border: 0,
						display: "flex",
						alignItems: "center",
						gap: 10,
						padding: "0 14px",
						background: "linear-gradient(180deg, #1c3f6e, var(--jeu-nuit-profonde))",
						color: "var(--jeu-texte-vif)",
						font: "inherit",
						fontWeight: 800,
						fontSize: 14,
						cursor: onClick ? "pointer" : "default",
					}}
				>
					<KeyCap>X</KeyCap>
					<span>{bouton}</span>
				</button>
			) : null}
		</div>
	);
}

/** Une touche du clavier, comme les guides de boutons du jeu. */
export function KeyCap({ children }: { children: ReactNode }) {
	return (
		<span
			style={{
				display: "inline-flex",
				alignItems: "center",
				justifyContent: "center",
				minWidth: 20,
				height: 18,
				padding: "0 4px",
				borderRadius: 3,
				background: "#3b3f44",
				color: "var(--jeu-texte-vif)",
				fontSize: 11,
				fontWeight: 800,
				lineHeight: 1,
			}}
		>
			{children}
		</span>
	);
}

/**
 * Une banniere penchee, comme la pile du coin bas-droit du menu.
 *
 * Le jeu y annonce ses DLC ; Aphrody y met des liens reels. La forme est la meme : bande
 * penchee, texte clair sur fond sature, empilee vers le haut.
 */
export function Banniere({
	children,
	href,
	teinte = "azur",
}: {
	children: ReactNode;
	href?: string;
	teinte?: "azur" | "nuit" | "ambre";
}) {
	const fonds: Record<string, string> = {
		azur: "linear-gradient(180deg, var(--jeu-tuile-active-haut), var(--jeu-tuile-active-bas))",
		nuit: "linear-gradient(180deg, #1c3f6e, var(--jeu-nuit-profonde))",
		ambre: "linear-gradient(180deg, var(--jeu-accent-ambre), var(--jeu-lisere-or))",
	};
	const Balise = href ? "a" : "div";
	return (
		<Balise
			href={href}
			style={{
				...coupe(10),
				display: "flex",
				alignItems: "center",
				justifyContent: "flex-end",
				gap: 8,
				height: 28,
				padding: "0 16px",
				background: fonds[teinte],
				color: teinte === "ambre" ? "var(--jeu-nuit-profonde)" : "var(--jeu-texte-vif)",
				font: "inherit",
				fontWeight: 800,
				fontSize: 13,
				letterSpacing: "0.02em",
				textDecoration: "none",
				textShadow: teinte === "ambre" ? "none" : "0 1px 2px rgb(10 47 102 / 85%)",
				boxShadow: "var(--jeu-ombre-tuile)",
			}}
		>
			{children}
		</Balise>
	);
}

/** Une pastille d'angle, bordee d'or comme les mentions d'edition du jeu. */
export function CornerChip({ children }: { children: ReactNode }) {
	return (
		<span
			style={{
				...coupe(10),
				display: "inline-flex",
				alignItems: "center",
				gap: 8,
				// 43 px de haut, comme la pastille « Deluxe Edition » mesuree sur la capture ; a
				// 4 px de padding elle n'en faisait que 29 et se perdait dans l'angle.
				padding: "11px 28px",
				background: "linear-gradient(180deg, #10233f, var(--jeu-fond-abysse))",
				border: "2px solid var(--jeu-lisere-or)",
				color: "var(--jeu-accent-ambre)",
				fontWeight: 800,
				fontSize: 14,
				letterSpacing: "0.04em",
			}}
		>
			{children}
		</span>
	);
}

/**
 * Les pictogrammes des tuiles.
 *
 * Traces geometriques de ce depot — aucune image du jeu n'est reproduite ici. Ils ne portent
 * jamais le sens a eux seuls : chaque tuile affiche AUSSI son libelle, et le pictogramme est
 * `aria-hidden`.
 */
function Glyphe({ children }: { children: ReactNode }) {
	return (
		<svg
			width="34"
			height="34"
			viewBox="0 0 24 24"
			fill="none"
			stroke="currentColor"
			strokeWidth="1.8"
			strokeLinecap="round"
			strokeLinejoin="round"
			aria-hidden="true"
			focusable="false"
		>
			{children}
		</svg>
	);
}

/** Les glyphes disponibles, par nom. Un nom absent rend `null` — donc rien, visiblement. */
export const GLYPHES = {
	image: (
		<Glyphe>
			<rect x="3" y="4" width="18" height="16" rx="2" />
			<circle cx="8.5" cy="9.5" r="1.6" />
			<path d="M4 18l5-5 4 4 3-3 4 4" />
		</Glyphe>
	),
	cube: (
		<Glyphe>
			<path d="M12 3l8 4.5v9L12 21l-8-4.5v-9z" />
			<path d="M12 12l8-4.5M12 12v9M12 12L4 7.5" />
		</Glyphe>
	),
	onde: (
		<Glyphe>
			<path d="M3 12h2M7 7v10M11 4v16M15 8v8M19 11h2" />
		</Glyphe>
	),
	film: (
		<Glyphe>
			<rect x="3" y="5" width="18" height="14" rx="2" />
			<path d="M7 5v14M17 5v14M3 12h18" />
		</Glyphe>
	),
	arbre: (
		<Glyphe>
			<path d="M4 5h6l2 2h8v12H4z" />
			<path d="M9 11h8M9 15h5" />
		</Glyphe>
	),
	ballon: (
		<Glyphe>
			<circle cx="12" cy="12" r="9" />
			<path d="M12 7l4 3-1.5 5h-5L8 10z" />
		</Glyphe>
	),
	livre: (
		<Glyphe>
			<path d="M4 5a2 2 0 012-2h12v18H6a2 2 0 01-2-2z" />
			<path d="M8 7h7M8 11h7" />
		</Glyphe>
	),
	engrenage: (
		<Glyphe>
			<circle cx="12" cy="12" r="3.2" />
			<path d="M12 2v3M12 19v3M2 12h3M19 12h3M5 5l2 2M17 17l2 2M19 5l-2 2M7 17l-2 2" />
		</Glyphe>
	),
	info: (
		<Glyphe>
			<circle cx="12" cy="12" r="9" />
			<path d="M12 11v5M12 8h.01" />
		</Glyphe>
	),
} satisfies Record<string, ReactNode>;

/** Le nom d'un glyphe. */
export type NomGlyphe = keyof typeof GLYPHES;
