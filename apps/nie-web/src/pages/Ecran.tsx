/**
 * La coquille des écrans secondaires — la MÊME direction artistique que l'accueil.
 *
 * ## Le défaut qu'elle corrige
 *
 * Le site avait deux interfaces. L'accueil rendait le menu principal du jeu : fond presque
 * blanc, panneaux biseautés, tuiles bleues. Cliquer sur une tuile basculait sur une tout autre
 * application — bandeau sombre, panneau latéral noir, tuiles d'un second modèle (`SkewTile`),
 * pastille de version — sans aucune parenté visuelle avec l'écran qu'on venait de quitter. Deux
 * chartes, deux jeux de composants, un seul site.
 *
 * Ici, il n'en reste qu'une. Les entrées restent les tuiles du menu, simplement réduites à la
 * hauteur d'une barre ; le fond, la typographie et les biseaux sont ceux de l'accueil. Passer
 * d'un écran à l'autre ne change plus que le contenu.
 *
 * ## Pourquoi la barre n'est pas un canevas
 *
 * L'accueil est un `GameCanvas` : 1280×720 mis à l'échelle, positions absolues mesurées sur une
 * capture du jeu. Un catalogue de 54 203 éléments ne tient pas dans un canevas à hauteur fixe —
 * il défile. La coquille est donc en flux, et n'emprunte au canevas que ses formes et ses
 * couleurs. C'est la limite assumée de la reconstruction : la DA se transpose, la géométrie du
 * menu non.
 */
import type { SanteApi } from "@niers/asset-source";
import { biseau, GLYPHES, IconTile, TileStrip } from "@niers/inacord-ui";
import type { ReactNode } from "react";
import { useMemo } from "react";
import { entreesMenu } from "../entrees";
import { ACCUEIL } from "../routage";

/** La hauteur des tuiles de la barre. Assez pour l'icône et son libellé, pas plus. */
const HAUTEUR_TUILE = 58;

/** Leur largeur. Plus étroites que celles du menu (137 px), qui tiennent un canevas entier. */
const LARGEUR_TUILE = 116;

/**
 * La coquille : la barre d'entrées, puis le contenu.
 *
 * Elle ne connaît pas les écrans qu'elle héberge — elle reçoit `children`. C'est ce qui permet
 * au catalogue et à l'explorateur de n'avoir aucun code de navigation.
 */
export function EcranSecondaire({
	vue,
	onChoisir,
	etat,
	children,
}: {
	vue: string;
	onChoisir: (vue: string) => void;
	etat: SanteApi | null;
	children: ReactNode;
}) {
	const entrees = useMemo(() => entreesMenu(etat), [etat]);
	return (
		<div
			style={{
				minHeight: "100vh",
				display: "flex",
				flexDirection: "column",
				background: "var(--jeu-ciel-clair)",
				color: "var(--jeu-nuit-profonde)",
			}}
		>
			<header
				style={{
					display: "flex",
					flexWrap: "wrap",
					alignItems: "center",
					gap: "var(--jeu-espace-l)",
					padding: "10px var(--jeu-espace-xl)",
					// Le dégradé des panneaux du menu, à plat : c'est ce qui rattache la barre à
					// l'écran d'accueil sans le recopier.
					background:
						"linear-gradient(180deg, var(--jeu-surface-glace), var(--jeu-ciel-brume))",
					borderBottom: "3px solid var(--jeu-tuile-bas)",
				}}
			>
				{/* Le titre ramène au menu. Sans ce chemin de retour, on n'atteint l'accueil qu'en
				    réécrivant l'URL à la main. */}
				<button
					type="button"
					onClick={() => onChoisir(ACCUEIL)}
					style={{
						border: 0,
						background: "transparent",
						padding: 0,
						color: "var(--jeu-nuit-profonde)",
						font: "inherit",
						fontSize: 26,
						fontWeight: 900,
						letterSpacing: "0.04em",
						cursor: "pointer",
					}}
				>
					APHRODY
				</button>

				<nav
					aria-label="Entrées du site"
					style={{ marginLeft: "auto", minWidth: 0, maxWidth: "100%", overflowX: "auto" }}
				>
					<TileStrip ecart={6}>
						{entrees.map((entree) => (
							<IconTile
								key={entree.vue}
								icone={GLYPHES[entree.glyphe]}
								libelle={entree.libelle}
								actif={entree.vue === vue}
								largeur={LARGEUR_TUILE}
								hauteur={HAUTEUR_TUILE}
								penche={biseau(HAUTEUR_TUILE)}
								onClick={() => onChoisir(entree.vue)}
							/>
						))}
					</TileStrip>
				</nav>
			</header>

			<main
				style={{
					flex: 1,
					width: "100%",
					maxWidth: 1400,
					margin: "0 auto",
					padding: "var(--jeu-espace-xl)",
				}}
			>
				{children}
			</main>
		</div>
	);
}

/**
 * Le titre d'un écran : le bandeau bleu biseauté du menu, à sa taille de section.
 *
 * `RibbonBand` n'est pas réutilisé tel quel : il occupe toute la hauteur de son parent et
 * centre son texte, ce qui convient à un bandeau posé dans un canevas, pas à un titre de page.
 */
export function TitreVue({ children, appoint }: { children: ReactNode; appoint?: ReactNode }) {
	return (
		<h2
			style={{
				...biseauStyle(14),
				// Le bandeau s'ajuste à son texte, comme celui du jeu (438 px pour « Victory
				// Road »). Étiré sur toute la largeur, son biseau devient invisible et il ne
				// ressemble plus à rien d'autre qu'à une barre de couleur.
				width: "fit-content",
				minWidth: 280,
				maxWidth: "100%",
				display: "flex",
				alignItems: "center",
				gap: "var(--jeu-espace-m)",
				margin: "0 0 var(--jeu-espace-l)",
				padding: "8px 34px",
				background:
					"linear-gradient(180deg, var(--jeu-tuile-active-haut), var(--jeu-tuile-active-bas))",
				color: "var(--jeu-texte-vif)",
				fontSize: 21,
				fontWeight: 800,
				letterSpacing: "0.06em",
				textShadow: "0 1px 3px rgb(10 47 102 / 85%)",
				boxShadow: "var(--jeu-ombre-tuile)",
			}}
		>
			<span>{children}</span>
			{appoint ? (
				<span style={{ fontSize: 14, fontWeight: 700, opacity: 0.9 }}>{appoint}</span>
			) : null}
		</h2>
	);
}

/**
 * Un message de l'écran — attente, vide, panne.
 *
 * Il parle à qui consulte le site, pas à qui l'exploite : ni nom de service, ni code d'erreur,
 * ni terme d'implémentation. Un message technique en façade ne répare rien et n'apprend rien à
 * son lecteur.
 */
export function Note({ children, ton = "info" }: { children: ReactNode; ton?: "info" | "alerte" }) {
	return (
		<p
			role={ton === "alerte" ? "alert" : undefined}
			style={{
				margin: 0,
				padding: "var(--jeu-espace-m) var(--jeu-espace-l)",
				background: "rgb(255 255 255 / 75%)",
				borderLeft: `4px solid ${
					ton === "alerte" ? "var(--jeu-accent-brique)" : "var(--jeu-accent-azur)"
				}`,
				color: "var(--jeu-nuit-profonde)",
				fontWeight: 700,
			}}
		>
			{children}
		</p>
	);
}

/**
 * Un compte suivi de son nom, accordé.
 *
 * « 1 entrées » sur l'explorateur d'un dossier à une seule ligne : la faute est petite, elle
 * est en tête de page, et c'est le genre de détail qui fait douter du reste. Zéro prend le
 * singulier, comme le veut l'usage français.
 */
/**
 * Formate une taille en octets, **jusqu'aux gigaoctets**.
 *
 * L'échelle s'arrêtait aux mégaoctets, dans trois copies de cette fonction : `bgm_chronicle.awb`
 * s'affichait « 1291.9 Mo », et le plus gros fichier du jeu — un `.usm` de 2 099 267 008 octets —
 * « 2002.0 Mo ». Ce n'est pas faux, c'est illisible : passé mille, l'unité a changé et le
 * lecteur doit diviser de tête. Une fonction, un endroit.
 */
export function tailleLisible(octets: number): string {
	if (octets < 1024) return `${octets} o`;
	if (octets < 1024 * 1024) return `${(octets / 1024).toFixed(1)} ko`;
	if (octets < 1024 * 1024 * 1024) return `${(octets / (1024 * 1024)).toFixed(1)} Mo`;
	return `${(octets / (1024 * 1024 * 1024)).toFixed(2)} Go`;
}

export function accorde(n: number, singulier: string, pluriel = `${singulier}s`): string {
	return `${n.toLocaleString("fr")} ${n > 1 ? pluriel : singulier}`;
}

/** Le biseau du menu, en `clip-path` : haut décalé vers la droite, comme les tuiles du jeu. */
function biseauStyle(penche: number) {
	return {
		clipPath: `polygon(${penche}px 0, 100% 0, calc(100% - ${penche}px) 100%, 0 100%)`,
	} as const;
}
