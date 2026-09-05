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
