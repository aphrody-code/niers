/**
 * Pré-compresse le bundle en `.br` et `.zst`, après le build.
 *
 * `nie-site` sait servir ces variantes telles quelles (`routes/static_files.rs`) : il négocie
 * `Accept-Encoding`, sert le fichier voisin s'il existe, et ne recompresse JAMAIS à la volée.
 * Sans cette étape, la capacité restait inutilisée — le serveur avait le code, le bundle
 * n'avait pas les fichiers.
 *
 * Compresser au BUILD plutôt qu'à la requête change la nature du calcul : il a lieu une fois,
 * au niveau maximal, au lieu d'être refait pour chaque visiteur au niveau le plus bas que le
 * temps de réponse tolère.
 *
 * Une variante n'est écrite que si elle est PLUS PETITE que l'original : sur un fichier déjà
 * compact, la compression produit parfois un fichier plus gros, et le servir serait une
 * régression silencieuse.
 */
import { readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { brotliCompressSync, constants, zstdCompressSync } from "node:zlib";

// `URL.pathname` renvoie « /C:/Users/… » sous Windows : `readdirSync` échoue alors sur un
// chemin que rien ne signale comme malformé, et le build meurt APRÈS que vite a écrit `dist`.
// `fileURLToPath` est la seule conversion correcte des deux côtés.
const DIST = fileURLToPath(new URL("../dist", import.meta.url));

/** Extensions qui gagnent à être compressées. Une image l'est déjà. */
const CIBLES = [".js", ".css", ".html", ".json", ".svg", ".map", ".txt"];

function fichiers(dir: string, acc: string[] = []): string[] {
	for (const e of readdirSync(dir)) {
		const p = join(dir, e);
		if (statSync(p).isDirectory()) fichiers(p, acc);
		else if (CIBLES.some((x) => p.endsWith(x))) acc.push(p);
	}
	return acc;
}

let ecrits = 0;
let octetsOriginaux = 0;
let octetsBrotli = 0;

for (const f of fichiers(DIST)) {
	const brut = readFileSync(f);
	// Sous ~1 Ko, l'en-tête de compression et le coût de négociation annulent le gain.
	if (brut.length < 1024) continue;
	octetsOriginaux += brut.length;

	const br = brotliCompressSync(brut, {
		params: {
			[constants.BROTLI_PARAM_QUALITY]: constants.BROTLI_MAX_QUALITY,
			[constants.BROTLI_PARAM_SIZE_HINT]: brut.length,
		},
	});
	if (br.length < brut.length) {
		writeFileSync(`${f}.br`, br);
		ecrits++;
		octetsBrotli += br.length;
	} else {
		octetsBrotli += brut.length;
	}

	const zst = zstdCompressSync(brut);
	if (zst.length < brut.length) {
		writeFileSync(`${f}.zst`, zst);
		ecrits++;
	}
}

const ko = (n: number) => `${(n / 1024).toFixed(1)} ko`;
console.log(
	`pre-compression : ${ecrits} fichiers ecrits · ${ko(octetsOriginaux)} -> ${ko(octetsBrotli)} en brotli`,
);
