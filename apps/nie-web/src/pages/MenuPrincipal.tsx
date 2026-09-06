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
 *    et les bandeaux. Ses positions viennent de `BOITES`, MESURÉES sur une capture du jeu par
 *    `scripts/validation/mesurer-mainmenu.py` — pas du binaire, et pas de l'œil.
 *
 * La distinction n'est pas cosmétique : l'export ne donne pas la place des widgets du menu — 24
 * de ses 34 objets restent sur le centre par défaut et 5 sortent du canevas. Présenter la
 * seconde couche comme une mesure du jeu serait faire passer une reconstruction pour une
 * preuve. Le panneau de diagnostic (touche « Calque ») affiche ces comptes plutôt que de les
 * taire.
 *
 * ## Ce qui a changé après comparaison avec une vraie capture
 *
 * La première version posait ses positions à l'œil. Mise à côté de la capture, elle avait huit
 * écarts, dont quatre structurels : un fond en dégradé bleu là où l'écran du jeu est presque
 * blanc, deux panneaux qui se rejoignaient au centre au lieu de laisser 330 px au logo, un
 * biseau penché dans le MAUVAIS SENS (le jeu décale le haut vers la droite), et deux blocs
 * entiers absents — la rangée basse et la pile de bannières. Chaque position est désormais un
 * nombre mesuré, et le script qui les produit est versionné à côté.
 */
import type { SanteApi, VueCatalogue } from "@niers/asset-source";
import {
	Badge,
	Banniere,
	bilanLayout,
	BOITES,
	CanvasItem,
	CenterPlate,
	CornerChip,
	ECART_TUILE,
	FOND_MENU,
	GameCanvas,
	GLYPHES,
	HeroPanel,
	IconTile,
	KeyCap,
	LARGEUR_TUILE,
	LayoutRender,
	lireLayout,
	type NomGlyphe,
	NoticeCard,
	PENTE_PANNEAU,
	RibbonBand,
	TileStrip,
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
	/** Le total publié par le serveur pour cette vue, ou `null` s'il ne le connaît pas encore. */
	total: number | null;
}

/**
 * L'habillage d'une vue : son libellé et son pictogramme.
 *
 * Ce sont les deux SEULES choses figées ici, et elles ne sont pas de la donnée — un nom de vue
 * qui n'y figure pas s'affiche tel que le serveur l'a nommé, avec un pictogramme neutre. La
 * liste des entrées et leurs comptes, eux, viennent entièrement de `/api/v1/health`.
 */
const HABILLAGE: Record<string, { libelle: string; glyphe: NomGlyphe }> = {
	textures: { libelle: "Textures", glyphe: "image" },
	modeles: { libelle: "Modèles", glyphe: "cube" },
	sons: { libelle: "Sons", glyphe: "onde" },
	videos: { libelle: "Vidéos", glyphe: "film" },
};

/**
 * La rangée principale, construite depuis ce que le SERVEUR publie.
 *
 * Rien n'est figé : le jour où `nie-site` expose une vue de plus, elle apparaît dans le menu
 * sans qu'une ligne d'ici bouge — avec son nom et son compte à lui. Une liste écrite en dur
 * aurait au contraire continué d'afficher cinq tuiles et quatre totaux périmés, sans erreur
 * nulle part. C'est la même raison qui interdit de reprendre les chiffres de la capture du jeu
 * (« VICTOIRES 221 », « NIVEAU DE L'ÉQUIPE 99 ») : ils décrivent UNE sauvegarde, pas l'écran.
 *
 * L'explorateur est la seule entrée qui ne vienne pas de là : il ne parcourt pas un catalogue
 * mais le VFS, et le serveur ne le publie pas comme une vue. Il n'a donc pas de total.
 */
function entreesPrincipales(etat: SanteApi | null): EntreeMenu[] {
	const vues = (etat?.vues ?? []).map((v) => ({
		vue: v.nom,
		libelle: HABILLAGE[v.nom]?.libelle ?? v.nom,
		glyphe: HABILLAGE[v.nom]?.glyphe ?? ("arbre" as NomGlyphe),
		total: v.total,
	}));
	return [
		...vues,
		{ vue: "explorateur", libelle: "Explorateur", glyphe: "arbre", total: null },
	];
}

/** Une entrée de la rangée basse : trois liens de service, comme les trois tuiles du jeu. */
interface EntreeService {
	libelle: string;
	glyphe: NomGlyphe;
	href?: string;
	action?: "calque";
}

/**
 * La rangée basse.
 *
 * Le jeu y met glossaire, réglages et informations. Ces trois-là sont des routes RÉELLES de
 * `nie-site` — vérifiables d'un `curl` — et non des tuiles décoratives.
 */
const SERVICES: readonly EntreeService[] = [
	{ libelle: "Calque", glyphe: "livre", action: "calque" },
	{ libelle: "API", glyphe: "engrenage", href: "/api/v1/health" },
	{ libelle: "Flux", glyphe: "info", href: "/feed.atom" },
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

/** Le décalage du bord intérieur d'un panneau, entre son haut et son bas. */
const BISEAU_PANNEAU = Math.round(PENTE_PANNEAU * BOITES.panneaux.h);

/**
 * La largeur des deux panneaux, déduite de leurs bords mesurés.
 *
 * Le panneau gauche va du bord de l'écran jusqu'à son bord intérieur le plus large (en bas) ;
 * le droit part de son bord le plus large (en bas également, côté gauche) jusqu'au bord droit.
 */
const PANNEAU_GAUCHE_L = BOITES.panneauGaucheBord.bas;
const PANNEAU_DROIT_X = BOITES.panneauDroitBord.bas;
const PANNEAU_DROIT_L = LAYOUT.canvas.w - PANNEAU_DROIT_X;

/** Le centre mesuré de la rangée principale — la rangée du jeu n'est pas centrée sur l'écran. */
const CENTRE_RANGEE = BOITES.rangee.x + BOITES.rangee.l / 2;

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

	const principales = useMemo(() => entreesPrincipales(etat), [etat]);
	const totalGeneral = useMemo(
		() => etat?.vues.reduce((s, v) => s + (v.total ?? 0), 0) ?? 0,
		[etat],
	);

	const chargees = Object.values(textures).filter(Boolean).length;
	const echouees = Object.values(textures).filter((ok) => !ok).length;

	return (
		<GameCanvas canvas={LAYOUT.canvas} fond={FOND_MENU}>
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
			<CanvasItem x={BOITES.notice.x} y={BOITES.notice.y} largeur={BOITES.notice.l} z={10}>
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

			{/* --- Coin haut-droit : la version, en clair comme le jeu affiche la sienne ------
			    Pas une pastille grise : le jeu écrit « ver.7.1.2 0.90 301 » en bleu, sans fond. */}
			<CanvasItem
				x={BOITES.version.x + BOITES.version.l}
				y={BOITES.version.y}
				ancreX={1}
				z={10}
			>
				<span
					style={{
						fontSize: 17,
						fontWeight: 800,
						letterSpacing: "0.06em",
						color: "var(--jeu-accent-azur)",
						whiteSpace: "nowrap",
					}}
				>
					{etat ? `${etat.service} ${etat.version || "—"}` : "hors ligne"}
				</span>
			</CanvasItem>

			{/* --- Le titre, au centre haut, dans la boîte du logo du jeu --- */}
			<CanvasItem
				x={BOITES.titre.x + BOITES.titre.l / 2}
				y={BOITES.titre.y + 40}
				ancreX={0.5}
				z={10}
			>
				<div style={{ textAlign: "center", lineHeight: 1 }}>
					<div
						style={{
							fontSize: 82,
							fontWeight: 900,
							letterSpacing: "0.04em",
							color: "var(--jeu-nuit-profonde)",
							textShadow:
								"0 3px 0 var(--jeu-texte-vif), 0 0 18px rgb(165 225 246 / 90%)",
						}}
					>
						APHRODY
					</div>
					<div
						style={{
							marginTop: 10,
							fontSize: 24,
							fontWeight: 800,
							letterSpacing: "0.30em",
							textTransform: "uppercase",
							color: "var(--jeu-tuile-bas)",
						}}
					>
						Victory Road
					</div>
				</div>
			</CanvasItem>

			{/* --- L'encart du haut-droit : là où le jeu place « Inazuma Post » --- */}
			<CanvasItem
				x={BOITES.encartHautDroit.x}
				y={BOITES.encartHautDroit.y}
				largeur={BOITES.encartHautDroit.l}
				z={10}
			>
				<a
					href="/feed.atom"
					style={{
						display: "flex",
						alignItems: "center",
						justifyContent: "flex-end",
						gap: 10,
						color: "var(--jeu-accent-azur)",
						fontWeight: 800,
						fontSize: 19,
						textDecoration: "none",
					}}
				>
					<KeyCap>F</KeyCap>
					<span>Nouveautés</span>
					<span style={{ color: "var(--jeu-tuile-bas)" }}>{GLYPHES.info}</span>
				</a>
			</CanvasItem>

			{/* --- Les deux panneaux : ce que le site expose, de part et d'autre du titre ------
			    Ils ne se touchent pas : la capture laisse 228 px entre leurs bords les plus
			    larges, et c'est cet écart qui donne sa place au logo. */}
			<CanvasItem x={0} y={BOITES.panneaux.y} largeur={PANNEAU_GAUCHE_L} z={5}>
				<HeroPanel
					titre="Ressources"
					cote="gauche"
					penche={BISEAU_PANNEAU}
					onClick={() => onChoisir("textures")}
				>
					<span style={APPOINT_GAUCHE}>
						{compte(totaux.get("textures")) ?? "—"} textures
						<br />
						{compte(totaux.get("modeles")) ?? "—"} modèles
					</span>
				</HeroPanel>
			</CanvasItem>
			<CanvasItem
				x={PANNEAU_DROIT_X}
				y={BOITES.panneaux.y}
				largeur={PANNEAU_DROIT_L}
				z={5}
			>
				<HeroPanel
					titre="Explorer"
					cote="droite"
					penche={BISEAU_PANNEAU}
					onClick={() => onChoisir("explorateur")}
				>
					<span style={APPOINT_DROIT}>
						ENTRÉES INDEXÉES
						<br />
						<strong style={{ fontSize: 30 }}>
							{etat?.capacites.vfs_entrees
								? etat.capacites.vfs_entrees.toLocaleString("fr")
								: "…"}
						</strong>
					</span>
				</HeroPanel>
			</CanvasItem>

			{/* --- La plaque centrale --- */}
			<CanvasItem
				x={BOITES.plaque.x + BOITES.plaque.l / 2}
				y={BOITES.plaque.y}
				ancreX={0.5}
				z={20}
			>
				<CenterPlate libelle="Ressources" valeur={totalGeneral.toLocaleString("fr")} />
			</CanvasItem>

			{/* --- La rangée principale : les entrées du menu --- */}
			<CanvasItem x={CENTRE_RANGEE} y={BOITES.rangee.y} ancreX={0.5} z={20}>
				<TileStrip ecart={ECART_TUILE}>
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

			{/* --- Le bandeau, sous la rangée. Il n'est PAS centré sur l'écran : son centre
			    mesuré est à x = 813, décalé vers la droite comme dans le jeu. --- */}
			<CanvasItem
				x={BOITES.bandeau.x}
				y={BOITES.bandeau.y}
				largeur={BOITES.bandeau.l}
				hauteur={BOITES.bandeau.h}
				z={20}
			>
				<RibbonBand>
					<span>Inazuma Eleven : Victory Road</span>
				</RibbonBand>
			</CanvasItem>

			{/* --- La rangée basse : trois entrées de service --- */}
			<CanvasItem
				x={BOITES.rangeeBasse.x + BOITES.rangeeBasse.l / 2}
				y={BOITES.rangeeBasse.y}
				ancreX={0.5}
				z={20}
			>
				<TileStrip ecart={ECART_TUILE}>
					{SERVICES.map((service) =>
						service.href ? (
							<a
								key={service.libelle}
								href={service.href}
								style={{ textDecoration: "none" }}
							>
								<IconTile
									icone={GLYPHES[service.glyphe]}
									libelle={service.libelle}
									hauteur={BOITES.rangeeBasse.h}
								/>
							</a>
						) : (
							<IconTile
								key={service.libelle}
								icone={GLYPHES[service.glyphe]}
								libelle={service.libelle}
								appoint={LIBELLE_CALQUE[calque]}
								hauteur={BOITES.rangeeBasse.h}
								onClick={() => setCalque(SUIVANT[calque])}
							/>
						),
					)}
				</TileStrip>
			</CanvasItem>

			{/* --- Le détail, seulement en diagnostic : un panneau permanent serait du bruit ---
			    Les comptes du calque vivent ICI et non sur l'écran par défaut : le jeu n'affiche
			    rien à cet endroit, et une rangée de badges au milieu du menu est le genre de
			    détail qui trahit une maquette. */}
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
						<div
							style={{
								display: "flex",
								gap: 8,
								alignItems: "center",
								marginBottom: 4,
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

			{/* --- Les angles bas : mention d'édition, bannières, aide, mention légale --- */}
			<CanvasItem x={BOITES.coinBasGauche.x} y={BOITES.coinBasGauche.y} z={20}>
				<CornerChip>Aphrody · aphrody.com</CornerChip>
			</CanvasItem>

			<CanvasItem
				x={BOITES.bannieres.x}
				y={BOITES.bannieres.y}
				largeur={BOITES.bannieres.l}
				z={20}
			>
				<div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
					<Banniere href="/sitemap.xml" teinte="nuit">
						Plan du site
					</Banniere>
					<Banniere href="/llms.txt" teinte="azur">
						llms.txt
					</Banniere>
					<Banniere href="https://github.com/aphrody-dev" teinte="ambre">
						aphrody-dev
					</Banniere>
				</div>
			</CanvasItem>

			<CanvasItem
				x={BOITES.aide.x + BOITES.aide.l / 2}
				y={BOITES.aide.y}
				ancreX={0.5}
				z={20}
			>
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
						whiteSpace: "nowrap",
					}}
				>
					<KeyCap>V</KeyCap>
					<span>{LIBELLE_CALQUE[calque]}</span>
				</button>
			</CanvasItem>

			<CanvasItem
				x={BOITES.mention.x + BOITES.mention.l}
				y={BOITES.mention.y}
				ancreX={1}
				z={20}
			>
				<span
					style={{
						fontSize: 13,
						fontWeight: 700,
						color: "var(--jeu-tuile-bas)",
						whiteSpace: "nowrap",
					}}
				>
					Ressources © LEVEL-5 Inc.
				</span>
			</CanvasItem>
		</GameCanvas>
	);
}

/**
 * Le texte d'appoint du panneau gauche : en haut, du côté extérieur.
 *
 * Le bord extérieur est vertical — c'est le bord de l'écran — donc rien ne rogne ici. Poser ce
 * texte du côté intérieur le ferait entrer dans le biseau de 98 px : il resterait lisible en
 * étant coupé, ce qui est exactement le défaut qu'on ne voit pas (« 54 203 textures » devient
 * « 203 textures », un chiffre plausible).
 */
const APPOINT_GAUCHE = {
	position: "absolute",
	top: 16,
	left: 28,
	fontSize: 15,
	fontWeight: 800,
	lineHeight: 1.35,
	color: "var(--jeu-nuit-profonde)",
} as const;

/**
 * L'appoint du panneau droit : en haut, du côté extérieur.
 *
 * Le jeu, lui, pose son « NIVEAU DE L'ÉQUIPE 99 » du côté INTÉRIEUR, sur le biseau. Le faire
 * demanderait de rentrer le texte de 98 px et de le décaler ligne à ligne pour suivre la pente
 * — ce que la capture ne permet pas de mesurer. Écart assumé, et dit ici plutôt que caché.
 */
const APPOINT_DROIT = {
	position: "absolute",
	top: 16,
	right: 28,
	fontSize: 13,
	fontWeight: 800,
	lineHeight: 1.25,
	textAlign: "right",
	letterSpacing: "0.06em",
	color: "var(--jeu-nuit-profonde)",
} as const;
