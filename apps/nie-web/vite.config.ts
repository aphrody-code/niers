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
export default defineConfig({
	plugins: [react()],
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
