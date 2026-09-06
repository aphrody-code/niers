import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

/**
 * Hote web d'Aphrody. Le bundle est servi par la crate `nie-site` (Axum), qui
 * lit `apps/nie-web/dist` : ne pas changer `outDir` sans changer
 * `NIE_SITE_STATIC_DIR` cote Rust.
 *
 * `nie-site` sert ses fichiers empreintes en `immutable` et sait rendre un
 * `.br`/`.zst` pre-compresse a cote du fichier ; la compression a la volee est
 * volontairement absente des deux cotes.
 */
/*
 * Tailwind v4 est ici pour UNE raison, et elle est mesurée : `packages/inacord-ui` expose
 * 37 primitives (`data-grid`, `tree-rows`, `tabs`, `split-pane`, `tooltip`…) écrites en classes
 * Tailwind, et cet hôte n'en utilisait AUCUNE — il n'importait que les jetons CSS. Les monter
 * sans Tailwind ne lève aucune erreur : les composants se rendent, sans un seul style. C'est le
 * mode d'échec le plus coûteux du dépôt (« une page peut rendre son titre et être un 500 »).
 */
export default defineConfig({
	plugins: [react(), tailwindcss()],
	build: { outDir: "dist", sourcemap: true, assetsDir: "static" },
	server: {
		port: 5175,
		// En dev, tout ce qui n'est pas le bundle part vers nie-site.
		proxy: Object.fromEntries(
			["/api", "/f", "/b", "/assets", "/healthz"].map((p) => [
				p,
				{ target: "http://127.0.0.1:8085", changeOrigin: true },
			]),
		),
	},
});
