/**
 * `@niers/inacord-ui` — l'interface partagee d'Inacord et d'Aphrody.
 *
 * Un composant de ce paquet ne connait pas son hote : il demande sa source par
 * `useAssetSource()` et ce qu'elle sait faire par `useCapacites()`. C'est la condition pour que
 * le meme code tourne dans une fenetre Tauri et dans un navigateur.
 */
export {
	AssetSourceProvider,
	type ContexteSource,
	useAssetSource,
	useCapacites,
	useErreurSource,
} from "./source";

// --- Coquilles : la direction artistique du jeu ------------------------------------------
//
// Deux ambiances, montees par deux hotes : le MENU PRINCIPAL pour Aphrody, INACORD pour
// l'application de bureau. Elles ne dessinent que des formes — aucune source, aucun hote.
// Les couleurs vivent dans `shell/game-tokens.css`, mesurees sur la reference archivee.
export {
	Badge,
	Callout,
	HeaderBanner,
	SidePanel,
	SkewTile,
	TileRow,
	TitleBand,
	VersionChip,
} from "./shell/main-menu";
export {
	HexBackdrop,
	type Message,
	MessageThread,
	type Onglet,
	PhoneFrame,
	RoomList,
	type Salon,
	TabBar,
} from "./shell/inacord";

// --- L'ecran de menu principal : ses formes, et le rendu d'un layout exporte ---------------
//
// Les formes (`ecran-menu`) sont posees par l'appelant en coordonnees du canevas ; le layout
// (`layout-jeu` + `layout-render`) vient du jeu et n'est jamais reecrit a la main. Les deux se
// montent dans le MEME `GameCanvas`, donc dans le meme repere.
export {
	Banniere,
	CanvasItem,
	CenterPlate,
	CornerChip,
	GLYPHES,
	HeroPanel,
	IconTile,
	KeyCap,
	type NomGlyphe,
	NoticeCard,
	RibbonBand,
	TileStrip,
} from "./shell/ecran-menu";
// La geometrie de l'ecran, MESUREE sur une capture du jeu (`scripts/validation/
// mesurer-mainmenu.py`). Elle est exportee parce que l'appelant pose les positions : sans elle,
// il les reinventerait, et c'est exactement ce qui a produit un ecran ou tout etait a peu pres
// au bon endroit sans qu'un seul nombre soit rattachable a une mesure.
export {
	ANGLE_TUILE_DEG,
	biseau,
	type Boite,
	BOITES,
	ECART_TUILE,
	FOND_MENU,
	LARGEUR_TUILE,
	largeurTuile,
	PENTE_PANNEAU,
	PENTE_TUILE,
} from "./shell/geometrie-mainmenu";
export {
	auCentreParDefaut,
	type BilanLayout,
	bilanLayout,
	type CanvasLayout,
	cheminVfsSprite,
	dansCanvas,
	echellePourZone,
	estMuet,
	type LayoutJeu,
	lireLayout,
	type ObjetLayout,
	objetsTries,
	type SegmentTexte,
	segmentsTexte,
	type SlotTexte,
	type SpriteLayout,
	styleObjet,
	tailleObjet,
	texteNu,
	type TransformLayout,
} from "./shell/layout-jeu";
export {
	GameCanvas,
	LayoutRender,
	type ProprietesLayout,
	useEchelleCanvas,
} from "./shell/layout-render";
