import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";

const racine = document.getElementById("racine");
if (!racine) throw new Error("#racine absent de index.html");
createRoot(racine).render(
	<StrictMode>
		<App />
	</StrictMode>,
);
