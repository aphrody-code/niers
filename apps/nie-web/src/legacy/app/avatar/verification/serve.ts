/** Harnais local seulement : bun apps/azalee/app/avatar/verification/serve.ts */
import { createRequire } from "node:module";
import tailwind from "@tailwindcss/postcss";
const postcss = createRequire(import.meta.resolve("@tailwindcss/postcss"))("postcss");
const build = await Bun.build({ entrypoints: [new URL("./web.tsx", import.meta.url).pathname.replace(/^\/([A-Z]:)/, "$1")], target: "browser", define: { "process.env.NODE_ENV": JSON.stringify("development") } });
if (!build.success) throw new AggregateError(build.logs, "Compilation du harnais impossible");
const js = await build.outputs.find(o => o.path.endsWith(".js"))!.text();
const globals = new URL("../../globals.css", import.meta.url).pathname.replace(/^\/([A-Z]:)/, "$1");
const styles = await postcss([tailwind()]).process(await Bun.file(globals).text(), { from: globals });
const css = styles.css + (await build.outputs.find(o => o.path.endsWith(".css"))?.text() ?? "");
const vendor = Bun.file(new URL("../../../public/vendor/model-viewer.min.js", import.meta.url));
const serveur = Bun.serve({ hostname: "127.0.0.1", port: 8796, async fetch(req) {
	const url = new URL(req.url);
	if (url.pathname === "/web.js") return new Response(js, { headers: { "content-type": "text/javascript" } });
	if (url.pathname === "/web.css") return new Response(css, { headers: { "content-type": "text/css" } });
	if (url.pathname === "/vendor/model-viewer.min.js") return new Response(vendor, { headers: { "content-type": "text/javascript" } });
	if (/^\/vendor\/(draco\/(draco_decoder\.(wasm|js)|draco_wasm_wrapper\.js)|basis\/basis_transcoder\.(wasm|js)|meshopt_decoder\.module\.js)$/.test(url.pathname)) {
		return new Response(Bun.file(new URL(`../../../public${url.pathname}`, import.meta.url)));
	}
	if (url.pathname === "/wasm/nie_wasm_bg.wasm") return new Response(Bun.file(new URL("../../../public/wasm/nie_wasm_bg.wasm", import.meta.url)), { headers: { "content-type": "application/wasm" } });
	if (url.pathname === "/fixture.glb") return new Response(Bun.file(new URL("./fixtures/byron-current.glb", import.meta.url)));
	if (url.pathname === "/fixture.glb.gz") return new Response(Bun.gzipSync(await Bun.file(new URL("./fixtures/byron-current.glb", import.meta.url)).arrayBuffer()));
	if (url.pathname === "/avatar/catalog.json" || url.pathname.startsWith("/model-avatar/") || url.pathname.startsWith("/g4tx/dx11/menu/200_icon/21_icon_avatar/")) {
		const reponse = await fetch(`https://cdn.rosegriffon.fr${url.pathname}${url.search}`);
		return new Response(reponse.body, { status: reponse.status, headers: { "content-type": reponse.headers.get("content-type") || "application/octet-stream" } });
	}
	return new Response(`<!doctype html><html lang="fr"><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>Validation atelier NIE</title><link rel="stylesheet" href="/web.css"><style>body{font:16px system-ui;background:#f1f5f8;margin:20px}button,input{font:inherit}#preuves{white-space:pre-wrap;background:#d9eaf4;padding:12px;max-height:120px;overflow:auto}</style><div id="preuves">Aucun export analysé</div><div id="app"></div><script type="module" src="/web.js"></script></html>`, { headers: { "content-type": "text/html;charset=utf-8" } });
} });
console.log(`Validation locale : ${serveur.url}`);
