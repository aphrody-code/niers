/**
 * Les trois portes de Next, rendues portables.
 *
 * ## Pourquoi ce module existe
 *
 * `apps/azalee/components` porte 184 composants dont la valeur est réelle — listes filtrables,
 * fiches, éditeur, tableaux de bord — et **143 imports de `next/*`** qui les clouent au sol :
 * mesuré le 2026-09-06, `next/link` 61 fois, `next/image` 53, `next/navigation` 27. Ce paquet
 * est monté par deux hôtes dont **aucun n'est Next** : Aphrody est un Vite/SPA, Inacord une
 * application Tauri. Chacun de ces imports y échoue à la résolution.
 *
 * Trois adaptateurs suffisent donc à en débloquer 141 sur 143 — les deux restants
 * (`next/font/local`, `next/dynamic`) tiennent à un composant chacun.
 *
 * ## Ce qu'ils ne font pas
 *
 * Ils ne réimplémentent pas Next : ils rendent le geste **à l'hôte**. Un lien est un `<a>`
 * quand l'hôte navigue par URL, et un rappel quand il navigue par état ; la navigation lit
 * l'URL réelle plutôt que le routeur d'un framework absent. Le composant garde sa forme, l'hôte
 * garde sa manière.
 */
import * as React from "react";

/**
 * L'hôte décide comment on navigue.
 *
 * Aphrody pousse une entrée d'historique et change d'écran sans recharger ; un hôte sans
 * routeur laisse le navigateur suivre le `href`. Le défaut — `null` — est le comportement d'un
 * `<a>` ordinaire, qui est correct partout.
 */
const ContexteNavigation = React.createContext<{
	naviguer?: (href: string) => void;
} | null>(null);

/** Installe la navigation de l'hôte pour tous les composants portés. */
export function FournisseurNavigation({
	naviguer,
	children,
}: {
	naviguer: (href: string) => void;
	children: React.ReactNode;
}) {
	const valeur = React.useMemo(() => ({ naviguer }), [naviguer]);
	return <ContexteNavigation value={valeur}>{children}</ContexteNavigation>;
}

/**
 * Le remplaçant de `next/link`.
 *
 * Reste un vrai `<a href>` : c'est ce qui garde le clic milieu, le « ouvrir dans un onglet »,
 * le survol qui montre la cible, et l'indexation. L'hôte n'intercepte que le clic simple, et
 * seulement s'il sait faire mieux — un `Link` qui rendrait un `<div onClick>` casserait les
 * trois premiers sans que rien ne le signale.
 */
export function Link({
	href,
	children,
	prefetch: _prefetch,
	replace: _replace,
	scroll: _scroll,
	...props
}: React.ComponentProps<"a"> & {
	href: string;
	/** Accepté et ignoré : il n'y a rien à précharger sans routeur de framework. */
	prefetch?: boolean;
	replace?: boolean;
	scroll?: boolean;
}) {
	const ctx = React.useContext(ContexteNavigation);
	return (
		<a
			href={href}
			onClick={(e) => {
				props.onClick?.(e);
				// Un clic modifié (Ctrl, ⌘, milieu) appartient au navigateur : l'intercepter
				// empêcherait d'ouvrir dans un onglet, sans message d'erreur.
				if (
					!ctx?.naviguer ||
					e.defaultPrevented ||
					e.metaKey ||
					e.ctrlKey ||
					e.shiftKey ||
					e.altKey ||
					e.button !== 0 ||
					href.startsWith("http")
				) {
					return;
				}
				e.preventDefault();
				ctx.naviguer(href);
			}}
			{...props}
		>
			{children}
		</a>
	);
}

/**
 * Le remplaçant de `next/image`.
 *
 * `next/image` optimise, redimensionne et sert du WebP par un serveur d'images. Il n'y en a pas
 * ici : les octets viennent du VFS, déjà décodés par `nie-model-serve`. On rend donc un `<img>`
 * avec `loading="lazy"` — la seule des optimisations de Next qui soit native au navigateur — et
 * on ACCEPTE les props qui n'ont plus d'objet plutôt que de faire échouer le composant.
 *
 * `fill` mérite un mot : sous Next il fait remplir le parent positionné. Ici il devient un
 * `position:absolute; inset:0`, ce qui donne le même rendu tant que le parent est positionné —
 * et le composant porté l'est, sans quoi il ne marchait pas non plus sous Next.
 */
export function Image({
	src,
	alt,
	fill,
	priority,
	quality: _quality,
	unoptimized: _unoptimized,
	style,
	...props
}: Omit<React.ComponentProps<"img">, "src"> & {
	src: string | { src: string };
	fill?: boolean;
	priority?: boolean;
	quality?: number;
	unoptimized?: boolean;
}) {
	return (
		<img
			src={typeof src === "string" ? src : src.src}
			alt={alt ?? ""}
			// `priority` sous Next veut dire « précharge-la » : l'équivalent natif est de NE PAS
			// différer le chargement. Le traduire en `eager` garde l'intention.
			loading={priority ? "eager" : "lazy"}
			decoding="async"
			style={fill ? { position: "absolute", inset: 0, ...style } : style}
			{...props}
		/>
	);
}

/**
 * Le remplaçant de `next/navigation`.
 *
 * Lit l'URL **réelle** plutôt que l'état d'un routeur absent, et écrit par l'API History. Les
 * composants portés s'en servent pour trois choses : lire un paramètre, poser un paramètre,
 * rafraîchir. Les trois marchent sans framework.
 */
export function useRouter() {
	const ctx = React.useContext(ContexteNavigation);
	return React.useMemo(
		() => ({
			push: (href: string) => {
				if (ctx?.naviguer) ctx.naviguer(href);
				else window.location.assign(href);
			},
			replace: (href: string) => window.location.replace(href),
			back: () => window.history.back(),
			forward: () => window.history.forward(),
			// Sans serveur à re-interroger, « rafraîchir » est un rechargement. C'est le
			// comportement honnête : mentir en ne faisant rien laisserait croire à une mise à
			// jour qui n'a pas eu lieu.
			refresh: () => window.location.reload(),
			prefetch: () => {},
		}),
		[ctx],
	);
}

/** Le chemin courant, sans le préfixe de langue ni la query. */
export function usePathname(): string {
	return typeof window === "undefined" ? "/" : window.location.pathname;
}

/** Les paramètres de l'URL courante, en lecture. */
export function useSearchParams(): URLSearchParams {
	return new URLSearchParams(
		typeof window === "undefined" ? "" : window.location.search,
	);
}
