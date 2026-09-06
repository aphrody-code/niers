export interface NavItem {
	label: string;
	path: string;
	imageUrl?: string;
	commonSprite?: string;
	/** Cle d'une VRAIE icone de menu du jeu (cf. config/game-icons.ts). */
	gameIcon?: string;
	icon: string;
	adminOnly?: boolean;
	defaultOpen?: boolean;
	children?: NavItem[];
	/**
	 * Entête de groupe : l'item n'est PAS une destination, seulement un
	 * déclencheur de repli. Les quatre groupes du dashboard (`content`,
	 * `social`, `tools`, `security`) n'ont aucune route derrière eux — sans ce
	 * drapeau, `app-sidebar.tsx` en faisait des liens qui tombaient en 404.
	 */
	groupOnly?: boolean;
	/** Additional path prefixes that mark this item as active (for mobile nav) */
	activePaths?: string[];
}

/**
 * Une route est-elle « sous » une entrée de menu ?
 *
 * Comparaison par SEGMENT, jamais par préfixe de chaîne : `/modeles` commence par
 * `/mode` sans être dedans, et les trois barres de navigation (colonne, rail,
 * barre du bas) testaient `pathname.startsWith(item.path)` — la page des modèles
 * allumait donc aussi l'entrée des modes. Symétriquement, un simple `===`
 * n'allumait rien du tout sur une fiche : `/chara/c05028001` laissait « Joueurs »
 * éteint. Les deux défauts se corrigent au même endroit.
 *
 * La racine est un cas à part : tout chemin lui est subordonné, seule l'égalité
 * la rend active.
 */
export function estRouteActive(pathname: string, path: string): boolean {
	if (path === "/") return pathname === "/";
	return pathname === path || pathname.startsWith(`${path}/`);
}

/**
 * Une entrée de menu est active quand on est sur sa route, sur une de ses routes
 * annexes (`activePaths`), ou sur la route d'un de ses enfants — un parent replié
 * doit se signaler quand la page courante est dans sa section.
 */
export function estItemActif(pathname: string, item: NavItem): boolean {
	if (item.activePaths?.some((p) => estRouteActive(pathname, p))) return true;
	if (!item.groupOnly && estRouteActive(pathname, item.path)) return true;
	return Boolean(item.children?.some((enfant) => estItemActif(pathname, enfant)));
}

export const navigationItems: NavItem[] = [
	{
		icon: "home",
		label: "nav.home",
		path: "/",
	},
	{
		icon: "sports_soccer",
		label: "nav.cross",
		path: "/cross",
	},
	{
		children: [
			{ label: "nav.news.articles", path: "/news", icon: "feed" },
			{ label: "nav.news.patch_notes", path: "/patch-notes", icon: "update" },
		],
		icon: "newspaper",
		label: "nav.news",
		path: "/news",
	},
	// — Base de données (wiki curé) —
	{
		commonSprite: "chara",
		icon: "groups",
		label: "nav.wiki.players",
		path: "/chara",
	},
	{
		// Techniques, tactiques, passifs (capacités hors hyper).
		children: [
			{ label: "nav.wiki.skills", path: "/skill", icon: "sports_martial_arts", commonSprite: "skill" },
			{ label: "nav.wiki.tactics", path: "/tactic", icon: "strategy" },
			{ label: "nav.wiki.passives", path: "/passive", icon: "auto_awesome", commonSprite: "tension" },
		],
		icon: "sports_martial_arts",
		label: "nav.section.abilities",
		path: "/skill",
	},
	{
		// Hyper-techniques (5 familles d'auras).
		children: [
			{
				label: "nav.wiki.auras.keshin",
				path: "/aura/esprits-guerriers",
				icon: "swords",
				commonSprite: "keshin",
			},
			{
				label: "nav.wiki.auras.soul",
				path: "/aura/totems",
				icon: "pets",
				commonSprite: "soul",
			},
			{
				label: "nav.wiki.auras.miximax",
				path: "/aura/miximax",
				icon: "group_add",
				commonSprite: "miximax",
			},
			{
				label: "nav.wiki.auras.awakening",
				path: "/aura/eveil",
				icon: "wb_twilight",
				commonSprite: "eveil",
			},
			{
				label: "nav.wiki.auras.mode",
				path: "/aura/changement-mode",
				icon: "change_circle",
				commonSprite: "mode_change",
			},
		],
		icon: "auto_awesome",
		label: "nav.wiki.hyper_skills",
		path: "/aura",
	},
	{
		// Équipes, entraîneurs, stades.
		children: [
			{ label: "nav.wiki.teams", path: "/equipe", icon: "shield", gameIcon: "emblem" },
			{ label: "nav.wiki.coaches", path: "/entraineur", icon: "sports", gameIcon: "medal" },
			{ label: "nav.wiki.stadiums", path: "/stade", icon: "stadium", gameIcon: "stadium" },
		],
		gameIcon: "emblem",
		icon: "shield",
		label: "nav.section.clubs",
		path: "/equipe",
	},
	{
		// Objets & moyens d'obtention (boutiques, capsules, drops).
		children: [
			{ label: "nav.wiki.items", path: "/item", icon: "backpack", gameIcon: "chest" },
			{ label: "nav.wiki.shops", path: "/boutique", icon: "storefront", gameIcon: "moneybag" },
			{ label: "nav.wiki.capsules", path: "/capsule", icon: "casino", gameIcon: "ticket" },
			{ label: "nav.wiki.invocation", path: "/invocation", icon: "stars", gameIcon: "star-token" },
			{ label: "nav.wiki.drops", path: "/drops", icon: "redeem" },
		],
		gameIcon: "chest",
		icon: "backpack",
		label: "nav.section.items",
		path: "/item",
	},
	{
		icon: "task_alt",
		label: "nav.wiki.quests",
		path: "/quete",
	},
	{
		// Table d'expérience du jeu (`inagle_exp_table`) : courbe + calculateurs.
		icon: "trending_up",
		label: "nav.wiki.levels",
		path: "/niveau",
	},
	{
		icon: "emoji_events",
		label: "nav.wiki.trophies",
		path: "/succes",
	},
	{
		// Les collections média et les cinq outils du wiki ont migré vers l'explorateur de
		// bureau (`docs/MIGRATION-EXPLORATEUR.md`). Ne reste ici que sa page de
		// téléchargement : un menu ne doit pointer que vers des routes servies.
		icon: "build",
		label: "nav.tools",
		path: "/tools/niers",
	},
	{
		icon: "settings",
		label: "nav.settings",
		path: "/settings",
	},
];

export const mobileNavItems: NavItem[] = [
	{ icon: "home", label: "nav.home", path: "/" },
	{
		activePaths: [
			"/chara",
			"/skill",
			"/item",
			"/boutique",
			"/capsule",
			"/invocation",
			"/equipe",
			"/aura",
			"/passive",
			"/tactic",
			"/quete",
			"/succes",
			"/entraineur",
			"/stade",
			"/drops",
			"/niveau",
			"/wiki",
			"/search",
		],
		icon: "search",
		label: "nav.wiki",
		path: "/search",
	},
	{
		activePaths: ["/news", "/patch-notes"],
		icon: "newspaper",
		label: "nav.news",
		path: "/news",
	},
	{
		activePaths: ["/tools"],
		icon: "build",
		label: "nav.tools",
		path: "/tools/niers",
	},
	{
		activePaths: ["/settings"],
		icon: "settings",
		label: "nav.settings",
		path: "/settings",
	},
];

export const dashboardItems: NavItem[] = [
	{
		adminOnly: true,
		icon: "dashboard",
		label: "nav.dashboard.overview",
		path: "/dashboard",
	},
	{
		adminOnly: true,
		children: [
			{
				label: "nav.dashboard.content.news",
				path: "/dashboard/news",
				icon: "newspaper",
			},
			{
				label: "nav.dashboard.content.database",
				path: "/dashboard/database",
				icon: "database",
			},
			{
				label: "nav.dashboard.content.verification",
				path: "/dashboard/database/verification",
				icon: "verified",
			},
		],
		icon: "edit_note",
		groupOnly: true,
		label: "nav.dashboard.content",
		path: "/dashboard/content",
	},
	{
		adminOnly: true,
		children: [
			{
				label: "nav.dashboard.social.tweets",
				path: "/dashboard/tweets",
				icon: "tag",
			},
		],
		icon: "share",
		groupOnly: true,
		label: "nav.dashboard.social",
		path: "/dashboard/social",
	},
	{
		adminOnly: true,
		children: [
			{
				label: "nav.dashboard.tools.import_doc",
				path: "/dashboard/import-google-doc",
				icon: "description",
			},
			{
				label: "nav.dashboard.tools.import_sheet",
				path: "/dashboard/import-sheet",
				icon: "table_chart",
			},
		],
		icon: "build",
		groupOnly: true,
		label: "nav.dashboard.tools",
		path: "/dashboard/tools",
	},
	{
		adminOnly: true,
		children: [
			{
				label: "nav.dashboard.security.audit",
				path: "/dashboard/audit",
				icon: "policy",
			},
		],
		icon: "shield",
		groupOnly: true,
		label: "nav.dashboard.security",
		path: "/dashboard/security",
	},
	{
		adminOnly: true,
		icon: "group",
		label: "nav.dashboard.users",
		path: "/dashboard/users",
	},
	{
		adminOnly: true,
		icon: "settings",
		label: "nav.dashboard.settings",
		path: "/dashboard/settings",
	},
];
