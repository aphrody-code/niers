/**
 * L'accueil d'Aphrody : le menu principal, et non une liste de fichiers.
 *
 * ## Le recadrage
 *
 * Aphrody n'est ni le wiki (c'est Azalée) ni l'explorateur de fichiers (c'est Inacord). C'est le
 * site d'outils, et son écran d'accueil est un MENU — les catalogues y sont des entrées, pas la
 * page d'arrivée. Ce fichier est ce changement : la racine rend le menu, `/textures` et ses
 * voisines rendent un catalogue.
 *
 * ## Deux couches, deux natures — et il ne faut pas les confondre
 *
 * 1. **Le calque exporté** (`LayoutRender`) dessine `mainmenu01.layout.json`, produit par
 *    `nie-game --runtime --menu mainmenu01 --export-layout`. C'est de la DONNÉE : un réexport
 *    le met à jour sans qu'une ligne d'ici change.
 * 2. **La reconstruction** (les `CanvasItem` ci-dessous) pose les panneaux, la rangée de tuiles
 *    et les bandeaux. Ces positions sont LUES SUR LA RÉFÉRENCE archivée
 *    (`data/design/aphrody-ui-ref-mainmenu-7.1.2.png`), pas mesurées dans le binaire.
 *
 * La distinction n'est pas cosmétique : l'export ne donne pas la place des widgets du menu — 24
 * de ses 34 objets restent sur le centre par défaut et 5 sortent du canevas. Présenter la
 * seconde couche comme une mesure serait faire passer une reconstruction pour une preuve. Le
 * panneau de diagnostic (touche « Calque ») affiche ces comptes plutôt que de les taire.
 */
import type { SanteApi, VueCatalogue } from "@niers/asset-source";
import {
	Badge,
	bilanLayout,
	CanvasItem,
	CenterPlate,
	CornerChip,
	GameCanvas,
	GLYPHES,
	HeroPanel,
	IconTile,
	KeyCap,
	LayoutRender,
	lireLayout,
	type NomGlyphe,
	NoticeCard,
	RibbonBand,
	TileStrip,
	VersionChip,
} from "@niers/inacord-ui";
import { useCallback, useMemo, useState } from "react";
import brut from "../donnees/mainmenu01.layout.json";

/**
 * Le layout, validé au chargement du module.
 *
 * `lireLayout` plutôt qu'un `as` : un réexport qui perdrait `canvas` ou renommerait `objects`
 * rendrait sinon une page vide, sans message, avec un typecheck vert.
 */
const LAYOUT = lireLayout(brut);

/** Ce que le layout contient réellement — compté, jamais affirmé. */
const BILAN = bilanLayout(LAYOUT);

/** Une entrée du menu. `vue` désigne la route ; `glyphe` n'est qu'un appui visuel. */
interface EntreeMenu {
	vue: string;
	libelle: string;
	glyphe: NomGlyphe;
	/** Le filtre du catalogue dont on affiche le total, quand il y en a un. */
	compte?: VueCatalogue;
}

/** Les entrées de la rangée principale, dans l'ordre où le serveur publie ses filtres. */
const PRINCIPALES: readonly EntreeMenu[] = [
	{ vue: "textures", libelle: "Textures", glyphe: "image", compte: "textures" },
	{ vue: "modeles", libelle: "Modèles", glyphe: "cube", compte: "modeles" },
	{ vue: "sons", libelle: "Sons", glyphe: "onde", compte: "sons" },
	{ vue: "videos", libelle: "Vidéos", glyphe: "film", compte: "videos" },
	{ vue: "explorateur", libelle: "Explorateur", glyphe: "arbre" },
];

/** L'état du calque exporté. Trois valeurs, parce qu'il y a trois questions distinctes. */
type ModeCalque = "masque" | "calque" | "diagnostic";

const SUIVANT: Record<ModeCalque, ModeCalque> = {
	masque: "calque",
	calque: "diagnostic",
	diagnostic: "masque",
};

const LIBELLE_CALQUE: Record<ModeCalque, string> = {
	masque: "Calque masqué",
	calque: "Calque du jeu",
	diagnostic: "Diagnostic",
};

/** Formate un compte, ou rend `null` quand le serveur ne le connaît pas encore. */
function compte(n: number | null | undefined): string | null {
	return typeof n === "number" ? n.toLocaleString("fr") : null;
}

export function MenuPrincipal({
	vue,
	onChoisir,
	etat,
	vfsPret,
}: {
	/** L'entrée courante, pour marquer la tuile correspondante. */
	vue: string;
	onChoisir: (vue: string) => void;
	etat: SanteApi | null;
	/** L'index du VFS est-il monté ? Une tuile ne promet pas ce qu'elle ne peut pas montrer. */
	vfsPret: boolean;
}) {
	const [calque, setCalque] = useState<ModeCalque>("calque");
	// Le résultat RÉEL du chargement de chaque texture, mesuré dans la page plutôt que déduit
	// d'un test passé ailleurs sur un autre état du serveur.
	const [textures, setTextures] = useState<Record<string, boolean>>({});
	const surTexture = useCallback((nom: string, chargee: boolean) => {
		setTextures((connu) => (connu[nom] === chargee ? connu : { ...connu, [nom]: chargee }));
	}, []);

	const totaux = useMemo(
		() => new Map(etat?.vues.map((v) => [v.nom, v.total]) ?? []),
		[etat],
	);
	const totalGeneral = useMemo(
		() => etat?.vues.reduce((s, v) => s + (v.total ?? 0), 0) ?? 0,
		[etat],
	);

	const chargees = Object.values(textures).filter(Boolean).length;
	const echouees = Object.values(textures).filter((ok) => !ok).length;

	return (
		<GameCanvas
			canvas={LAYOUT.canvas}
			fond="linear-gradient(180deg, var(--jeu-ciel-clair) 0%, var(--jeu-ciel-clair) 45%, var(--jeu-ciel-brume) 100%)"
		>
			{/* Le calque du jeu passe SOUS la reconstruction : c'est la donnée qui porte, pas
			    l'habillage. `baseZ` négatif garde les priorités de dessin exportées entre elles. */}
			{calque !== "masque" ? (
				<LayoutRender
					layout={LAYOUT}
					diagnostic={calque === "diagnostic"}
					// En calque, l'export s'efface derriere l'interface : 24 de ses objets sont
					// empiles sur le centre du canevas faute de position, et les rendre a pleine
					// opacite mettrait un tas de fragments au milieu de la page d'accueil. En
					// diagnostic, c'est l'inverse : on VIENT les lire.
					opacite={calque === "diagnostic" ? 1 : 0.4}
					baseZ={-1000}
					onTexture={surTexture}
				/>
			) : null}

			{/* --- Coin haut-gauche : l'état du service, là où le jeu met ses informations --- */}
			<CanvasItem x={8} y={8} largeur={322} z={10}>
				<NoticeCard
					titre="Aphrody — outils et ressources"
					lignes={[
						vfsPret ? "Le VFS du jeu est monté." : "Montage de l'index du VFS…",
						`${totalGeneral.toLocaleString("fr")} ressources cataloguées`,
					]}
					bouton={LIBELLE_CALQUE[calque]}
					onClick={() => setCalque(SUIVANT[calque])}
				/>
			</CanvasItem>

			{/* --- Coin haut-droit : la version du service, comme le jeu affiche la sienne --- */}
			<CanvasItem x={1268} y={8} ancreX={1} z={10}>
				<VersionChip version={etat ? `${etat.service} ${etat.version || "—"}` : "hors ligne"} />
			</CanvasItem>

			{/* --- Le titre, au centre haut --- */}
			<CanvasItem x={640} y={24} ancreX={0.5} z={10}>
				<div style={{ textAlign: "center", lineHeight: 1 }}>
					<div
						style={{
							fontSize: 64,
							fontWeight: 900,
							letterSpacing: "0.06em",
							color: "var(--jeu-nuit-profonde)",
							textShadow:
								"0 3px 0 var(--jeu-texte-vif), 0 0 18px rgb(165 225 246 / 90%)",
						}}
					>
						APHRODY
					</div>
					<div
						style={{
							marginTop: 6,
							fontSize: 19,
							fontWeight: 800,
							letterSpacing: "0.34em",
							textTransform: "uppercase",
							color: "var(--jeu-tuile-bas)",
						}}
					>
						Victory Road
					</div>
				</div>
			</CanvasItem>

			{/* --- Les deux panneaux : ce que le site expose, de part et d'autre --- */}
			<CanvasItem x={0} y={150} largeur={604} z={5}>
				<HeroPanel titre="Ressources" cote="gauche" onClick={() => onChoisir("textures")}>
					<span style={ANGLE_PANNEAU_GAUCHE}>
						{compte(totaux.get("textures")) ?? "—"} textures ·{" "}
						{compte(totaux.get("modeles")) ?? "—"} modèles
					</span>
				</HeroPanel>
			</CanvasItem>
			<CanvasItem x={676} y={150} largeur={604} z={5}>
				<HeroPanel titre="Explorer" cote="droite" onClick={() => onChoisir("explorateur")}>
					<span style={ANGLE_PANNEAU_DROIT}>
						{etat?.capacites.vfs_entrees
							? `${etat.capacites.vfs_entrees.toLocaleString("fr")} entrées indexées`
							: "index en cours"}
					</span>
				</HeroPanel>
			</CanvasItem>

			{/* --- La plaque centrale --- */}
			<CanvasItem x={640} y={266} ancreX={0.5} z={20}>
				<CenterPlate libelle="Ressources" valeur={totalGeneral.toLocaleString("fr")} />
			</CanvasItem>

			{/* --- La rangée principale : les entrées du menu --- */}
			<CanvasItem x={640} y={378} ancreX={0.5} z={20}>
				<TileStrip>
					{PRINCIPALES.map((entree) => {
						const total = entree.compte ? totaux.get(entree.compte) : undefined;
						return (
							<IconTile
								key={entree.vue}
								icone={GLYPHES[entree.glyphe]}
								libelle={entree.libelle}
								appoint={compte(total) ?? undefined}
								actif={entree.vue === vue}
								// Tant que l'index n'est pas prêt, la tuile est en sourdine : elle ne
								// promet pas un contenu qu'elle ne peut pas encore montrer.
								sourdine={!vfsPret}
								onClick={() => onChoisir(entree.vue)}
							/>
						);
					})}
				</TileStrip>
			</CanvasItem>

			{/* --- Le bandeau, sous la rangée --- */}
			<CanvasItem x={640} y={486} ancreX={0.5} largeur={430} hauteur={38} z={20}>
				<RibbonBand>
					<span>Inazuma Eleven : Victory Road</span>
				</RibbonBand>
			</CanvasItem>

			{/* --- Le compte du calque exporté : la mesure, à l'écran --- */}
			<CanvasItem x={640} y={540} ancreX={0.5} z={20}>
				<div
					style={{
						display: "flex",
						gap: 8,
						alignItems: "center",
						justifyContent: "center",
						color: "var(--jeu-nuit-profonde)",
						fontSize: 12,
						fontWeight: 700,
					}}
				>
					<Badge>{BILAN.total}</Badge>
					<span>objets exportés</span>
					<Badge teinte="ambre">{BILAN.avecSprite}</Badge>
					<span>avec texture</span>
					<Badge teinte={echouees > 0 ? "brique" : "cyan"}>
						{chargees}/{BILAN.avecSprite}
					</Badge>
					<span>chargées</span>
					{BILAN.muets > 0 ? (
						<>
							<Badge teinte="brique">{BILAN.muets}</Badge>
							<span>muets</span>
						</>
					) : null}
				</div>
			</CanvasItem>

			{/* --- Le détail, seulement en diagnostic : un panneau permanent serait du bruit --- */}
			{calque === "diagnostic" ? (
				<CanvasItem x={640} y={566} ancreX={0.5} largeur={620} z={30}>
					<div
						style={{
							padding: "8px 14px",
							background: "rgb(10 47 102 / 92%)",
							color: "var(--jeu-surface-glace)",
							fontSize: 12,
							lineHeight: 1.5,
							borderLeft: "4px solid var(--jeu-accent-ambre)",
						}}
					>
						<div>
							<strong>{LAYOUT.screen}</strong> — {LAYOUT.canvas.w}×{LAYOUT.canvas.h},{" "}
							{BILAN.total} objets, {BILAN.visibles} visibles, {BILAN.avecTexte} avec texte,{" "}
							{BILAN.textures.length} textures distinctes.
						</div>
						<div>
							<span style={{ color: "var(--jeu-accent-azur)" }}>
								{BILAN.auCentre} restés au centre par défaut
							</span>{" "}
							·{" "}
							<span style={{ color: "var(--jeu-accent-brique)" }}>
								{BILAN.horsCanvas} hors canevas
							</span>{" "}
							·{" "}
							<span style={{ color: "var(--jeu-accent-ambre)" }}>{BILAN.muets} muets</span>
							{echouees > 0 ? ` · ${echouees} textures en échec` : ""}
						</div>
						<div style={{ opacity: 0.8 }}>
							L'export ne donne pas la position des widgets du menu : ces objets-là sont
							rendus là où la donnée les met, jamais déplacés pour faire joli.
						</div>
					</div>
				</CanvasItem>
			) : null}

			{/* --- Les angles bas : mention d'édition, aide, mention légale --- */}
			<CanvasItem x={30} y={614} z={20}>
				<CornerChip>Aphrody · aphrody.com</CornerChip>
			</CanvasItem>
			<CanvasItem x={640} y={678} ancreX={0.5} z={20}>
				<button
					type="button"
					onClick={() => setCalque(SUIVANT[calque])}
					style={{
						display: "flex",
						alignItems: "center",
						gap: 8,
						border: 0,
						background: "transparent",
						color: "var(--jeu-nuit-profonde)",
						font: "inherit",
						fontWeight: 800,
						fontSize: 13,
						cursor: "pointer",
					}}
				>
					<KeyCap>V</KeyCap>
					<span>{LIBELLE_CALQUE[calque]}</span>
				</button>
			</CanvasItem>
			<CanvasItem x={1268} y={700} ancreX={1} z={20}>
				<span style={{ fontSize: 12, fontWeight: 700, color: "var(--jeu-tuile-bas)" }}>
					Ressources © LEVEL-5 Inc.
				</span>
			</CanvasItem>
		</GameCanvas>
	);
}

/** Le texte d'appoint d'un panneau, posé au-dessus de son titre. */
// Rentre de 84 px comme le titre, et pour la même raison : le `clip-path` du panneau coupe en
// biais, donc tout ce qui est posé à moins de 64 px du bord bas se fait rogner — et un texte
// rogné reste lisible, ce qui est exactement ce qui rend le défaut difficile à voir (« 54 203
// textures » devient « 203 textures », un chiffre plausible).
const ANGLE_PANNEAU_GAUCHE = {
	position: "absolute",
	bottom: 62,
	left: 84,
	fontSize: 13,
	fontWeight: 800,
	whiteSpace: "nowrap",
	color: "var(--jeu-nuit-profonde)",
} as const;

const ANGLE_PANNEAU_DROIT = {
	position: "absolute",
	bottom: 62,
	right: 84,
	fontSize: 13,
	fontWeight: 800,
	whiteSpace: "nowrap",
	color: "var(--jeu-nuit-profonde)",
} as const;
