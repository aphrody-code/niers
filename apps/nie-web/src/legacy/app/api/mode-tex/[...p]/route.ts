/**
 * Les textures d'un mode, décodées en PNG.
 *
 * Un `.g4tx` n'est pas une image : c'est un conteneur de textures DDS, que le navigateur ne sait
 * pas lire. `niers convert --toutes` les a décodées une par une hors ligne ; cette route se
 * contente de servir le résultat, posé sous `DATA_PATH/mode-tex/<mode>/<fichier>__<texture>.png`.
 *
 * Aucune conversion à la volée ici : le décodage BC1/BC3/BC7 coûte trop cher pour être fait à
 * chaque requête, et la sortie est immuable — elle ne change que si le jeu est mis à jour.
 */
import fs from "node:fs/promises";
import path from "node:path";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

const RACINE = path.resolve(
	path.join(process.env.DATA_PATH ?? "/home/ubuntu/niers/data", "mode-tex"),
);

export async function GET(
	_req: Request,
	{ params }: { params: Promise<{ p?: string[] }> },
): Promise<Response> {
	const { p } = await params;
	const cible = path.resolve(RACINE, (p ?? []).join("/"));
	// Anti-traversal : le chemin résolu doit rester sous la racine, et ne peut être qu'un PNG.
	if (!cible.startsWith(RACINE + path.sep) || !cible.endsWith(".png")) {
		return new Response("chemin refusé", { status: 404 });
	}
	try {
		const octets = await fs.readFile(cible);
		return new Response(new Uint8Array(octets), {
			headers: {
				"content-type": "image/png",
				"cache-control": "public, max-age=86400, immutable",
			},
		});
	} catch {
		return new Response("texture absente", { status: 404 });
	}
}
