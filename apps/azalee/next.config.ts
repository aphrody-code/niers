import type { NextConfig } from "next";
import path from "node:path";
import { withSentryConfig } from "@sentry/nextjs";

// CDN externe — décharge .next/static/ + assets public/ vers cdn.rosegriffon.fr.
const CDN_URL = process.env.NEXT_PUBLIC_CDN_URL?.trim() || "";
const CDN_ORIGIN = CDN_URL ? new URL(CDN_URL).origin : "";
const CDN_LOADER = CDN_URL && process.env.NEXT_PUBLIC_CDN_LOADER === "1";

// Origin du CDN d'assets jeu (modèles 3D GLB, images dump dx11) — toujours présent
// indépendamment du toggle NEXT_PUBLIC_CDN_URL (qui ne sert qu'à offloader .next/static).
const ASSET_CDN_ORIGIN = "https://cdn.rosegriffon.fr";

// Google AdSense — hôtes contactés par `adsbygoogle.js` : le chargeur et les
// créations (`*.googlesyndication.com`), le réseau de diffusion historique
// (`*.doubleclick.net`, `*.googleadservices.com`), la mesure anti-fraude
// (`*.adtrafficquality.google`) et les messages de consentement RGPD
// (`fundingchoicesmessages.google.com`).
// `frame-src` est le piège : une annonce s'affiche dans une iframe, donc avec
// `frame-src 'none'` le script se charge, ne signale rien, et l'emplacement reste
// vide — seule la console dit « Refused to frame ». Le contenu des créations est
// régi par la CSP de ces iframes, pas par la nôtre : inutile d'ouvrir
// 'unsafe-eval' ici.
const ADS_SCRIPT_SRC =
	"https://pagead2.googlesyndication.com https://*.googlesyndication.com https://*.googleadservices.com https://*.doubleclick.net https://*.adtrafficquality.google https://fundingchoicesmessages.google.com https://adservice.google.com https://www.google.com";
const ADS_FRAME_SRC =
	"https://googleads.g.doubleclick.net https://*.doubleclick.net https://*.googlesyndication.com https://*.adtrafficquality.google https://www.google.com https://fundingchoicesmessages.google.com";
const ADS_IMG_SRC =
	"https://*.googlesyndication.com https://*.doubleclick.net https://*.adtrafficquality.google https://www.google.com https://*.googleusercontent.com https://*.gstatic.com";
const ADS_CONNECT_SRC =
	"https://*.googlesyndication.com https://*.doubleclick.net https://*.adtrafficquality.google https://www.google.com https://*.gstatic.com https://fundingchoicesmessages.google.com";

const nextConfig: NextConfig = {
	poweredByHeader: false,

	// Nginx gere la compression (gzip) — desactiver ici pour eviter le double compress
	compress: false,

	// AssetPrefix : Next préfixe TOUTES les URLs /_next/static/* avec CDN_URL.
	...(CDN_URL && { assetPrefix: CDN_URL }),

	// React Compiler — memoization automatique (stable en Next.js 16)
	reactCompiler: true,

	images: {
		contentDispositionType: "attachment",
		deviceSizes: [768, 1024, 1920],
		formats: ["image/avif", "image/webp"],
		imageSizes: [96, 256, 384, 512],
		minimumCacheTTL: 86400,
		qualities: [75, 90, 100],
		remotePatterns: [
			{
				protocol: "https",
				hostname: "cdn.rosegriffon.fr",
				pathname: "/**",
			},
			{
				protocol: "https",
				hostname: "dxi4wb638ujep.cloudfront.net",
				pathname: "/**",
			},
			{
				protocol: "https",
				hostname: "www.inazuma.jp",
				pathname: "/**",
			},
			{
				protocol: "https",
				hostname: "zukan.inazuma.jp",
				pathname: "/**",
			},
			{
				protocol: "https",
				hostname: "inazuma.jp",
				pathname: "/**",
			},
			{
				protocol: "https",
				hostname: "lh3.googleusercontent.com",
				pathname: "/**",
			},
			{
				protocol: "https",
				hostname: "cdn.discordapp.com",
				pathname: "/**",
			},
			{
				protocol: "https",
				hostname: "pbs.twimg.com",
				pathname: "/**",
			},
			{
				protocol: "https",
				hostname: "azalee.rosegriffon.fr",
				pathname: "/storage/**",
			},
		],
		...(CDN_LOADER && {
			loader: "custom",
			loaderFile: "./cdn-loader.ts",
		}),
	},

	// Sortie standalone : l'app est servie par systemd derrière nginx.
	output: "standalone" as const,
	outputFileTracingRoot: path.resolve(__dirname, "../../"),
	turbopack: {
		root: path.resolve(__dirname, "../../"),
	},
	transpilePackages: [
		"@rosegriffon/azalee",
		"@rosegriffon/ui",
		"@rosegriffon/db",
		"@rosegriffon/types",
		"@rosegriffon/config",
		"@rosegriffon/auth",
		"@rosegriffon/assets",
	],

	// Packages Node.js natifs exclus du bundling serveur
	serverExternalPackages: [
		"@rosegriffon/inagle",
		"better-auth",
		"@better-auth/kysely-adapter",
		"kysely",
		"pg",
		"sharp",
		"nodemailer",
		"cheerio",
		"bcryptjs",
		"jsonwebtoken",
		"googleapis",
		"web-push",
		"konva",
		"csv-parse",
	],

	logging:
		process.env.NODE_ENV === "production"
			? { fetches: { fullUrl: false } }
			: { fetches: { fullUrl: true } },

	typescript: {
		ignoreBuildErrors: false,
	},

	compiler: {
		removeConsole:
			process.env.NODE_ENV === "production"
				? {
						exclude: ["error", "warn"],
					}
				: false,
	},

	async redirects() {
		// Les dix prefixes qui partent vers Aphrody.
		//
		// INACTIFS tant que `NEXT_PUBLIC_TOOLS_ORIGIN` n'est pas posee : sans elle la liste est
		// vide, et le wiki continue de servir ses pages comme aujourd'hui. C'est ce qui rend le
		// basculement reversible — une variable, pas un deploiement.
		//
		// Des PREFIXES EXPLICITES, jamais une expression reguliere sur `/tools`. La nuance est
		// tout sauf cosmetique : `/tools/niers/latest.json` est l'endpoint de mise a jour des
		// Inacord deja installes. Une regle large l'attraperait, et les clients cesseraient de
		// se mettre a jour sans qu'aucune page ne semble cassee. `/tools` lui-meme n'est donc
		// PAS redirige, seulement les outils un par un.
		const origineOutils = process.env.NEXT_PUBLIC_TOOLS_ORIGIN;
		const versAphrody = origineOutils
			? [
					"/tools/translator",
					"/tools/stats",
					"/tools/compare",
					"/tools/random-team",
					"/tools/my-team",
					"/gallery",
					"/textures",
					"/modeles",
					"/sons",
					"/videos",
				].map((prefixe) => ({
					// 308 et non 301 : la methode et le corps sont conserves, et le cache des
					// navigateurs ne fige pas la redirection de facon irreversible.
					destination: `${origineOutils}${prefixe}/:path*`,
					permanent: true,
					source: `${prefixe}/:path*`,
				}))
			: [];

		return [
			...versAphrody,
			{
				destination: "/cross",
				permanent: true,
				source: "/cross/catalogue",
			},
			{
				destination: "/dashboard/news/new",
				permanent: true,
				source: "/dashboard/news/edit",
			},
			{
				destination: "/tools/compare",
				permanent: true,
				source: "/compare",
			},
			{
				destination: "/tools/random-team",
				permanent: true,
				source: "/random-team",
			},
			{
				destination: "/news",
				permanent: true,
				source: "/news/tweet",
			},
		];
	},

	async rewrites() {
		// Storage self-host : nginx intercepte déjà /storage/v1/ et le route vers
		// rg-storage.service (127.0.0.1:8810). Ce rewrite reste le filet de repli
		// quand l'app est servie sans nginx devant (dev local, healthcheck direct).
		// Plus de variante interne : sur Vercel, `127.0.0.1` désigne la fonction elle-même.
		const storageUrl = process.env.NEXT_PUBLIC_SUPABASE_URL || "http://127.0.0.1:8811";

		const destination = storageUrl.startsWith("http")
			? `${storageUrl}/storage/v1/:path*`
			: "http://127.0.0.1:8811/storage/v1/:path*";

		return [
			{
				destination,
				source: "/storage/v1/:path*",
			},
			{
				destination: "https://azalee.rosegriffon.fr/zukan-assets-mirror/:path*",
				source: "/zukan/assets/:path*",
			},
		];
	},

	async headers() {
		const isDev = process.env.NODE_ENV === "development";
		const cdnSrc = CDN_ORIGIN ? ` ${CDN_ORIGIN}` : "";

		const cspHeader = [
			"default-src 'self'",
			// 'wasm-unsafe-eval' : requis pour instancier les modules WebAssembly
			// (lecteur de sauvegarde /save, renderer menu webgpu) sous CSP strict.
			`script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval'${cdnSrc} https://www.googletagmanager.com https://www.google-analytics.com ${ADS_SCRIPT_SRC}${isDev ? " 'unsafe-eval'" : ""}`,
			`style-src 'self' 'unsafe-inline'${cdnSrc}`,
			`img-src 'self' blob: data:${cdnSrc} ${ASSET_CDN_ORIGIN} https://dxi4wb638ujep.cloudfront.net https://*.inazuma.jp https://lh3.googleusercontent.com https://cdn.discordapp.com https://pbs.twimg.com ${ADS_IMG_SRC}`,
			`media-src 'self' blob: data:${cdnSrc} ${ASSET_CDN_ORIGIN} https://dxi4wb638ujep.cloudfront.net https://*.inazuma.jp https://video.twimg.com https://pbs.twimg.com ${ADS_IMG_SRC}`,
			`font-src 'self'${cdnSrc} ${ASSET_CDN_ORIGIN} https://*.gstatic.com`,
			// CloudFront est aussi en `connect-src`, pas seulement en `media-src` : le
			// bouton « Télécharger » d'une fiche technique fait un `fetch()` sur le
			// `.webm` pour le récupérer en blob et forcer l'enregistrement
			// (`SkillVideoActions`). Sans cette source, la CSP bloque la requête et le
			// bouton dégrade silencieusement en ouverture d'onglet — le visiteur voit
			// la vidéo se lire au lieu de se télécharger, sans un mot d'erreur.
			`connect-src 'self' blob:${cdnSrc} ${ASSET_CDN_ORIGIN} https://dxi4wb638ujep.cloudfront.net https://www.google-analytics.com https://www.googletagmanager.com ${ADS_CONNECT_SRC}`,
			`frame-src ${ADS_FRAME_SRC}`,
			"frame-ancestors 'none'",
			"base-uri 'self'",
			"form-action 'self'",
		].join("; ");

		return [
			{
				headers: [
					{ key: "X-Content-Type-Options", value: "nosniff" },
					{ key: "X-Frame-Options", value: "DENY" },
					{ key: "Referrer-Policy", value: "strict-origin-when-cross-origin" },
					{ key: "Permissions-Policy", value: "camera=(), microphone=(), geolocation=()" },
					{
						key: "Strict-Transport-Security",
						value: "max-age=63072000; includeSubDomains; preload",
					},
					{ key: "Content-Security-Policy", value: cspHeader },
					{ key: "X-DNS-Prefetch-Control", value: "on" },
				],
				source: "/(.*)",
			},
		];
	},

	experimental: {
		cpus: 1,
		optimizeCss: false,
		optimizePackageImports: [
			"lucide-react",
			"date-fns",
			"recharts",
			"react-hook-form",
			"zod",
			"clsx",
			"tailwind-merge",
			"class-variance-authority",
			"sonner",
			"cmdk",
			"@lexical/react",
			"lexical",
			"@dnd-kit/core",
			"@dnd-kit/sortable",
			"@dnd-kit/utilities",
			"@radix-ui/react-accordion",
			"@radix-ui/react-alert-dialog",
			"@radix-ui/react-avatar",
			"@radix-ui/react-checkbox",
			"@radix-ui/react-collapsible",
			"@radix-ui/react-dialog",
			"@radix-ui/react-dropdown-menu",
			"@radix-ui/react-label",
			"@radix-ui/react-popover",
			"@radix-ui/react-progress",
			"@radix-ui/react-radio-group",
			"@radix-ui/react-select",
			"@radix-ui/react-separator",
			"@radix-ui/react-slider",
			"@radix-ui/react-slot",
			"@radix-ui/react-switch",
			"@radix-ui/react-tabs",
			"@radix-ui/react-toggle",
			"@radix-ui/react-toggle-group",
			"@radix-ui/react-tooltip",
		],
		serverActions: {
			bodySizeLimit: "10mb",
		},
		serverSourceMaps: true,
		turbopackFileSystemCacheForBuild: true,
		viewTransition: false,
	},
};

export default withSentryConfig(nextConfig, {
	org: "rose-griffon",
	project: "rg-azalee",
	silent: !process.env.CI,
	tunnelRoute: "/monitoring",
	widenClientFileUpload: true,
});
