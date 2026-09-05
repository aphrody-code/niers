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
 * ## Pourquoi ne pas le fetcher non plus par `import.meta.url`
 *
 * `fetch(new URL("./logo-og.png", import.meta.url))` a été essayé, et il casse le build :
 * `file:` n'est pas un schéma que `fetch` doit servir, et Bun le refuse explicitement
 * (« not implemented... yet... »). L'échec n'est pas discret — il interrompt l'export de
 * `/aura/opengraph-image` et le build entier s'arrête — mais il n'apparaît qu'au build, donc
 * après la relecture du diff.
 *
 * Le logo est donc une **constante embarquée** (`og-logo-data.ts`, régénérable depuis
 * `logo-og.png`) : aucune I/O, aucun schéma d'URL, le même comportement sur le VPS, sous Bun
 * et sur Vercel. `public/logo-og.png` reste la copie servie aux navigateurs.
 */
import { LOGO_OG_BASE64 } from "./og-logo-data";

const DATA_URI = `data:image/png;base64,${LOGO_OG_BASE64}`;

/** Le logo en `data:` — constante embarquée, ni lecture disque ni requête réseau. */
// eslint-disable-next-line @typescript-eslint/require-await
export async function getOgLogoDataUri(): Promise<string> {
	return DATA_URI;
}
