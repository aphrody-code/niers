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
