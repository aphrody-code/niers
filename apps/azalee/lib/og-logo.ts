/**
 * Logo Azalée inliné en data URI pour les `opengraph-image.tsx`.
 *
 * ## Pourquoi ne pas pointer une URL publique
 *
 * `next/og` (Satori) résout les `<img src>` en les FETCHANT. Viser
 * `https://azalee.rosegriffon.fr/logo-og.png` faisait échouer la génération au build
 * (« Unsupported image type: unknown ») dès que le domaine de production n'était pas joignable
 * depuis la machine de build.
 *
 * ## Pourquoi ne plus lire `public/` depuis le disque
 *
 * La lecture par `process.cwd()` suppose une arborescence sur disque à l'exécution. C'est vrai
 * d'un serveur, pas d'une fonction serverless, où `public/` n'est pas garanti à côté du code —
 * et un logo manquant ne lève pas : il produit une image de partage muette, ce que personne ne
 * voit avant qu'un lien ne soit partagé.
 *
 * Le fichier est donc résolu par `import.meta.url`, la voie documentée pour les ressources de
 * `next/og` : le bundler le suit et l'embarque, ici comme sur Vercel. Il vit à côté de ce
 * module pour cette raison — `public/logo-og.png` reste la copie servie aux navigateurs.
 */
let _cached: string | null = null;

/** Le logo en `data:` — mémorisé, la lecture n'a lieu qu'une fois par instance. */
export async function getOgLogoDataUri(): Promise<string> {
	if (_cached) {
		return _cached;
	}
	const octets = await fetch(new URL("./logo-og.png", import.meta.url)).then((r) =>
		r.arrayBuffer(),
	);
	_cached = `data:image/png;base64,${Buffer.from(octets).toString("base64")}`;
	return _cached;
}
