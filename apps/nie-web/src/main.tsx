import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import "./base.css";
// Les surfaces des écrans du jeu (FILTRES, barre d'onglets, guides de touches), engendrées
// depuis `nie-ui` sur les captures de `data/menu`. Importées ici et non dans `base.css` :
// c'est un module, pas une remise à zéro, et les composants `Game*` du paquet partagé sont
// ce qui la consomme.
import "@niers/inacord-ui/shell/game-screens.css";

const racine = document.getElementById("racine");
if (!racine) throw new Error("#racine absent de index.html");
createRoot(racine).render(
	<StrictMode>
		<App />
	</StrictMode>,
);
