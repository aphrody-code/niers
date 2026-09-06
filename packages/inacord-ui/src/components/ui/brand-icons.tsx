import type { SVGProps } from "react";

/**
 * Logos des services tiers, source unique du monorepo.
 *
 * Il en existait trois implémentations concurrentes (`apps/website/src/shared/icons/*`,
 * `app/(auth)/login/_components/social-icons.tsx`, et des SVG inlinés dans le
 * composant de réglages d'azalée) qui divergeaient sur le `viewBox`, la présence
 * d'un `<title>` et la manière de dimensionner. Tout passe désormais par ici.
 *
 * Convention : `fill="currentColor"` (sauf Google, dont le logo est
 * multicolore par définition) et dimensions pilotées par la classe utilitaire
 * (`size-5`), l'attribut ne servant que de repli hors CSS.
 */
type BrandIconProps = SVGProps<SVGSVGElement> & {
	/** Rend l'icône annoncée aux lecteurs d'écran sous ce nom au lieu d'être masquée. */
	title?: string;
};

function svgProps({ title, ...props }: BrandIconProps, viewBox = "0 0 24 24") {
	return {
		height: 24,
		viewBox,
		width: 24,
		xmlns: "http://www.w3.org/2000/svg",
		...(title ? { role: "img" as const } : { "aria-hidden": true }),
		...props,
	};
}

export function DiscordIcon({ title, ...props }: BrandIconProps) {
	return (
		<svg {...svgProps({ title, ...props })} fill="currentColor">
			{title && <title>{title}</title>}
			<path d="M20.317 4.37a19.791 19.791 0 0 0-4.885-1.515.074.074 0 0 0-.079.037c-.21.375-.444.864-.608 1.25a18.27 18.27 0 0 0-5.487 0 12.64 12.64 0 0 0-.617-1.25.077.077 0 0 0-.079-.037A19.736 19.736 0 0 0 3.677 4.37a.07.07 0 0 0-.032.027C.533 9.046-.32 13.58.099 18.057c.002.022.015.043.03.056a19.9 19.9 0 0 0 5.993 3.03.078.078 0 0 0 .084-.028 14.09 14.09 0 0 0 1.226-1.994.076.076 0 0 0-.041-.106 13.107 13.107 0 0 1-1.872-.892.077.077 0 0 1-.008-.128 10.2 10.2 0 0 0 .372-.292.074.074 0 0 1 .077-.01c3.928 1.793 8.18 1.793 12.062 0a.074.074 0 0 1 .078.01c.12.098.246.198.373.292a.077.077 0 0 1-.006.127 12.299 12.299 0 0 1-1.873.892.077.077 0 0 0-.041.107c.36.698.772 1.362 1.225 1.993a.076.076 0 0 0 .084.028 19.839 19.839 0 0 0 6.002-3.03.077.077 0 0 0 .032-.054c.5-5.177-.838-9.674-3.549-13.66a.061.061 0 0 0-.031-.03zM8.02 15.33c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.956-2.419 2.157-2.419 1.21 0 2.176 1.096 2.157 2.42 0 1.333-.956 2.418-2.157 2.418zm7.975 0c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.955-2.419 2.157-2.419 1.21 0 2.176 1.096 2.157 2.42 0 1.333-.946 2.418-2.157 2.418z" />
		</svg>
	);
}

/**
 * Logo Google officiel : quadrichromie imposée par les conditions d'utilisation
 * de la marque, donc les couleurs restent en dur ici — ce sont des constantes de
 * marque, pas des choix de design system.
 */
export function GoogleIcon({ title, ...props }: BrandIconProps) {
	return (
		<svg {...svgProps({ title, ...props }, "0 0 48 48")}>
			{title && <title>{title}</title>}
			<path
				fill="#EA4335"
				d="M24 9.5c3.54 0 6.71 1.22 9.21 3.6l6.85-6.85C35.9 2.38 30.47 0 24 0 14.62 0 6.51 5.38 2.56 13.22l7.98 6.19C12.43 13.72 17.74 9.5 24 9.5z"
			/>
			<path
				fill="#4285F4"
				d="M46.98 24.55c0-1.57-.15-3.09-.38-4.55H24v9.02h12.94c-.58 2.96-2.26 5.48-4.78 7.18l7.73 6c4.51-4.18 7.09-10.36 7.09-17.65z"
			/>
			<path
				fill="#FBBC05"
				d="M10.53 28.59c-.48-1.45-.76-2.99-.76-4.59s.27-3.14.76-4.59l-7.98-6.19C.92 16.46 0 20.12 0 24c0 3.88.92 7.54 2.56 10.78l7.97-6.19z"
			/>
			<path
				fill="#34A853"
				d="M24 48c6.48 0 11.93-2.13 15.89-5.81l-7.73-6c-2.15 1.45-4.92 2.3-8.16 2.3-6.26 0-11.57-4.22-13.47-9.91l-7.98 6.19C6.51 42.62 14.62 48 24 48z"
			/>
		</svg>
	);
}

export function TwitchIcon({ title, ...props }: BrandIconProps) {
	return (
		<svg {...svgProps({ title, ...props })} fill="currentColor">
			{title && <title>{title}</title>}
			<path d="M11.571 4.714h1.715v5.143H11.57zm4.715 0H18v5.143h-1.714zM6 0L1.714 4.286v15.428h5.143V24l4.286-4.286h3.428L22.286 12V0zm14.571 11.143l-3.428 3.428h-3.429l-3 3v-3H6.857V1.714h13.714z" />
		</svg>
	);
}

/**
 * Glyphe TikTok officiel (note de musique stylisée), tracé issu de Simple Icons
 * (licence CC0 1.0), donc réutilisable sans attribution.
 *
 * Il remplace un SVG qui traînait dupliqué dans la barre latérale et le pied de
 * page du site : celui-ci était en `viewBox` 32×32 avec un `fill="#000000"` et
 * des dimensions `800px` en dur, ce qui le rendait invisible en thème sombre et
 * optiquement plus lourd que ses voisins. Ici : `viewBox` 24×24 comme toutes les
 * autres marques, un seul `path`, et la couleur héritée du texte.
 */
export function TikTokIcon({ title, ...props }: BrandIconProps) {
	return (
		<svg {...svgProps({ title, ...props })} fill="currentColor">
			{title && <title>{title}</title>}
			<path d="M12.525.02c1.31-.02 2.61-.01 3.91-.02.08 1.53.63 3.09 1.75 4.17 1.12 1.11 2.7 1.62 4.24 1.79v4.03c-1.44-.05-2.89-.35-4.2-.97-.57-.26-1.1-.59-1.62-.93-.01 2.92.01 5.84-.02 8.75-.08 1.4-.54 2.79-1.35 3.94-1.31 1.92-3.58 3.17-5.91 3.21-1.43.08-2.86-.31-4.08-1.03-2.02-1.19-3.44-3.37-3.65-5.71-.02-.5-.03-1-.01-1.49.18-1.9 1.12-3.72 2.58-4.96 1.66-1.44 3.98-2.13 6.15-1.72.02 1.48-.04 2.96-.04 4.44-.99-.32-2.15-.23-3.02.37-.63.41-1.11 1.04-1.36 1.75-.21.51-.15 1.07-.14 1.61.24 1.64 1.82 3.02 3.5 2.87 1.12-.01 2.19-.66 2.77-1.61.19-.33.4-.67.41-1.06.1-1.79.06-3.57.07-5.36.01-4.03-.01-8.05.02-12.07z" />
		</svg>
	);
}

/**
 * Logo X (ex-Twitter), tracé Simple Icons (CC0).
 *
 * Il existait en trois copies : `apps/website/src/shared/icons/twitter.tsx`, un
 * SVG inliné dans le pied de page et un autre dans la barre latérale. Trois
 * copies, c'est trois occasions de diverger — et elles avaient déjà divergé sur
 * la couleur de survol.
 */
export function XIcon({ title, ...props }: BrandIconProps) {
	return (
		<svg {...svgProps({ title, ...props })} fill="currentColor">
			{title && <title>{title}</title>}
			<path d="M18.901 1.153h3.68l-8.04 9.19L24 22.846h-7.406l-5.8-7.584-6.638 7.584H.474l8.6-9.83L0 1.154h7.594l5.243 6.932ZM17.61 20.644h2.039L6.486 3.24H4.298Z" />
		</svg>
	);
}

/** Logo YouTube, tracé Simple Icons (CC0). */
export function YouTubeIcon({ title, ...props }: BrandIconProps) {
	return (
		<svg {...svgProps({ title, ...props })} fill="currentColor">
			{title && <title>{title}</title>}
			<path d="M23.498 6.186a3.016 3.016 0 0 0-2.122-2.136C19.505 3.545 12 3.545 12 3.545s-7.505 0-9.377.505A3.017 3.017 0 0 0 .502 6.186C0 8.07 0 12 0 12s0 3.93.502 5.814a3.016 3.016 0 0 0 2.122 2.136c1.871.505 9.376.505 9.376.505s7.505 0 9.377-.505a3.015 3.015 0 0 0 2.122-2.136C24 15.93 24 12 24 12s0-3.93-.502-5.814ZM9.545 15.568V8.432L15.818 12l-6.273 3.568Z" />
		</svg>
	);
}

/** Logo Instagram, tracé Simple Icons (CC0). */
export function InstagramIcon({ title, ...props }: BrandIconProps) {
	return (
		<svg {...svgProps({ title, ...props })} fill="currentColor">
			{title && <title>{title}</title>}
			<path d="M12 0C8.74 0 8.333.015 7.053.072 5.775.132 4.905.333 4.14.63c-.789.306-1.459.717-2.126 1.384S.935 3.35.63 4.14C.333 4.905.131 5.775.072 7.053.012 8.333 0 8.74 0 12s.015 3.667.072 4.947c.06 1.277.261 2.148.558 2.913.306.788.717 1.459 1.384 2.126.667.666 1.336 1.079 2.126 1.384.766.296 1.636.499 2.913.558C8.333 23.988 8.74 24 12 24s3.667-.015 4.947-.072c1.277-.06 2.148-.262 2.913-.558.788-.306 1.459-.718 2.126-1.384.666-.667 1.079-1.335 1.384-2.126.296-.765.499-1.636.558-2.913.06-1.28.072-1.687.072-4.947s-.015-3.667-.072-4.947c-.06-1.277-.262-2.149-.558-2.913-.306-.789-.718-1.459-1.384-2.126C21.319 1.347 20.651.935 19.86.63c-.765-.297-1.636-.499-2.913-.558C15.667.012 15.26 0 12 0Zm0 2.16c3.203 0 3.585.016 4.85.071 1.17.055 1.805.249 2.227.415.562.217.96.477 1.382.896.419.42.679.819.896 1.381.164.422.36 1.057.413 2.227.057 1.266.07 1.646.07 4.85s-.015 3.585-.074 4.85c-.061 1.17-.256 1.805-.421 2.227-.224.562-.479.96-.899 1.382-.419.419-.824.679-1.38.896-.42.164-1.065.36-2.235.413-1.274.057-1.649.07-4.859.07-3.211 0-3.586-.015-4.859-.074-1.171-.061-1.816-.256-2.236-.421-.569-.224-.96-.479-1.379-.899-.421-.419-.69-.824-.9-1.38-.165-.42-.359-1.065-.42-2.235-.045-1.26-.061-1.649-.061-4.844 0-3.196.016-3.586.061-4.861.061-1.17.255-1.814.42-2.234.21-.57.479-.96.9-1.381.419-.419.81-.689 1.379-.898.42-.166 1.051-.361 2.221-.421 1.275-.045 1.65-.06 4.859-.06l.045.03Zm0 3.678a6.162 6.162 0 1 0 0 12.324 6.162 6.162 0 0 0 0-12.324ZM12 16a4 4 0 1 1 0-8 4 4 0 0 1 0 8Zm7.846-10.405a1.441 1.441 0 0 1-2.88 0 1.44 1.44 0 0 1 2.88 0Z" />
		</svg>
	);
}

export function PatreonIcon({ title, ...props }: BrandIconProps) {
	return (
		<svg {...svgProps({ title, ...props })} fill="currentColor">
			{title && <title>{title}</title>}
			<path d="M14.82 2.41c-4.834 0-8.766 3.932-8.766 8.766 0 4.82 3.932 8.74 8.766 8.74 4.82 0 8.74-3.92 8.74-8.74 0-4.834-3.92-8.766-8.74-8.766zM.436 21.59h4.293V2.41H.436z" />
		</svg>
	);
}
