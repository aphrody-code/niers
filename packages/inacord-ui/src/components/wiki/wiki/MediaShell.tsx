import { Link } from "../../../compat/next";
import { Icon } from "../../../components/wiki/ui/Icon";

/**
 * Ossature commune des pages média — `/gallery`, `/textures`, `/sons`, `/videos`.
 *
 * Ces quatre pages sont des vues d'un même fonds : les médias du jeu. Elles avaient pourtant
 * chacune leur en-tête, leur état vide et leur ligne de compte, écrits séparément — et surtout
 * rien ne les reliait : une collection ajoutée restait invisible depuis les trois autres, à
 * commencer par la galerie d'illustrations, qui en est la porte d'entrée naturelle.
 *
 * Tout est ici : un titre, une sous-navigation, un état de source indisponible, une ligne de
 * compte. Composants serveur — aucun état, rien que des liens.
 */

/**
 * Une collection média, avec le volume réel de son fonds (mesuré sur le VFS du jeu).
 *
 * Ces volumes sont un **instantané**, pas une mesure vivante : les afficher exigerait
 * d'interroger le VFS à chaque rendu de page média, pour un simple indice sous un lien.
 * Ils dérivent donc à chaque mise à jour du jeu — au 15/08/2026 l'écart accumulé était de
 * −1 077 textures, −109 banques, −2 vidéos et −1 mode, sans que rien ne le signale.
 *
 * Pour les rafraîchir, depuis le dépôt `niers` (les extensions sont celles du VFS, pas des
 * catégories de l'UI) :
 *
 * ```
 * niers vfs stats --top 40     # .g4tx = textures, .acb = banques de sons, .usm = vidéos
 * ```
 *
 * Deux pièges de comptage : les `.usm` sont **dupliqués** sous `data/dx11/movie` ET
 * `data/common/movie` (194 fichiers pour 98 vidéos distinctes — compter les basenames
 * uniques) ; et `niers vfs find` **plafonne à 102 résultats** sans `-n`, ce qui donne un
 * faux compte rond. Le nombre de modes vient de `data/modes.json`.
 */
const COLLECTIONS = [
	{ path: "/gallery", label: "Illustrations", icon: "image", hint: "sélection in-game" },
	{ path: "/textures", label: "Textures", icon: "texture", hint: "54 203 fichiers" },
	{ path: "/sons", label: "Sons & voix", icon: "graphic_eq", hint: "5 512 banques" },
	{ path: "/videos", label: "Vidéos", icon: "movie", hint: "98 cinématiques" },
	{ path: "/modeles", label: "Modèles 3D", icon: "deployed_code", hint: "6 236 modèles" },
	{ path: "/mode", label: "Modes de jeu", icon: "stadia_controller", hint: "11 modes" },
	{ path: "/avatar", label: "Éditeur d'avatar", icon: "face", hint: "502 parts" },
] as const;

/** Chemin d'une des collections média. */
export type MediaPath = (typeof COLLECTIONS)[number]["path"];

/**
 * Sous-navigation des collections média.
 *
 * @param active chemin de la page courante — l'entrée correspondante est mise en avant et rendue
 *   non cliquable : on ne propose pas d'aller là où on est déjà.
 */
export function MediaNav({ active }: { active: MediaPath }) {
	return (
		<nav aria-label="Collections média" className="flex flex-wrap gap-2">
			{COLLECTIONS.map((c) => {
				const courante = c.path === active;
				const contenu = (
					<>
						<Icon name={c.icon} size={18} className="shrink-0" />
						<span className="font-medium">{c.label}</span>
						<span className={courante ? "text-xs opacity-80" : "text-xs text-on-surface-variant"}>
							{c.hint}
						</span>
					</>
				);
				return courante ? (
					<span
						key={c.path}
						aria-current="page"
						className="flex items-center gap-2 rounded-full bg-primary px-3.5 py-2 text-sm text-on-primary"
					>
						{contenu}
					</span>
				) : (
					<Link
						key={c.path}
						href={c.path}
						className="
        flex items-center gap-2 rounded-full border border-outline-variant bg-surface-container-low px-3.5 py-2 text-sm
        text-on-surface transition
        hover:border-primary/50 hover:bg-surface-container
      "
					>
						{contenu}
					</Link>
				);
			})}
		</nav>
	);
}

/**
 * En-tête commun : titre, une phrase de description, puis la sous-navigation.
 *
 * `children` accueille ce qui est propre à la page (fil d'Ariane des textures, rôle du dossier)
 * et se place SOUS la description, avant la navigation.
 */
export function MediaHeader({
	title,
	description,
	active,
	children,
}: {
	title: string;
	description: React.ReactNode;
	active: MediaPath;
	children?: React.ReactNode;
}) {
	return (
		<div className="space-y-4">
			<header className="space-y-1">
				<h1 className="text-fluid-headline-md font-extrabold text-on-surface">{title}</h1>
				<p className="max-w-2xl text-sm text-on-surface-variant">{description}</p>
				{children}
			</header>
			<MediaNav active={active} />
		</div>
	);
}

/**
 * État « rien à montrer », dans le même cadre pour les quatre pages.
 *
 * Distingue deux cas que l'utilisateur ne doit pas confondre : le fonds est vide (`raison`
 * absente) ou la source est injoignable — une page qui rend son cadre et le dit vaut mieux
 * qu'une 500.
 */
export function MediaEmpty({ children }: { children: React.ReactNode }) {
	return (
		<div className="rounded-[32px] border border-dashed border-outline-variant bg-surface-container-low py-20 text-center">
			<p className="italic text-on-surface-variant">{children}</p>
		</div>
	);
}

/**
 * Bouton de retour d'une vue imbriquée.
 *
 * Un fil d'Ariane dit *où l'on est* ; il ne remplace pas une cible de retour franche, surtout
 * sur mobile où ses segments sont minuscules. Chaque vue imbriquée — dossier de textures,
 * conteneur, fiche de modèle — en porte un.
 */
export function MediaBack({ href, label }: { href: string; label: string }) {
	return (
		<Link
			href={href}
			className="
     inline-flex items-center gap-1.5 rounded-full border border-outline-variant bg-surface-container-low px-3.5 py-2
     text-sm text-on-surface transition
     hover:border-primary/50 hover:bg-surface-container
   "
		>
			<Icon name="arrow_back" size={16} className="shrink-0" />
			{label}
		</Link>
	);
}

/**
 * Titre d'une entrée média, où le code de fichier passe en DERNIER.
 *
 * Un identifiant comme `c01000010` ou `ev01_00050` ne dit rien à un lecteur. La règle est :
 * un vrai nom s'il existe dans les données du jeu, sinon le contexte (épisode, famille,
 * rôle du dossier), et le code seulement en repli — relégué en petit sous le titre, jamais
 * à sa place.
 */
export function MediaTitle({
	title,
	code,
	context,
}: {
	title: string;
	code?: string;
	context?: string;
}) {
	return (
		<span className="min-w-0">
			<span className="block truncate text-sm font-medium text-on-surface" title={title}>
				{title}
			</span>
			{(context || code) && (
				<span className="block truncate text-[11px] text-on-surface-variant">
					{context}
					{context && code && " · "}
					{code && <span className="font-mono">{code}</span>}
				</span>
			)}
		</span>
	);
}

/** Ligne de compte au-dessus d'une grille : ce qui est montré, sur quel total. */
export function MediaCount({ left, right }: { left: React.ReactNode; right?: React.ReactNode }) {
	return (
		<div className="
    flex items-center justify-between gap-3 px-1 text-xs font-medium uppercase tracking-wider text-on-surface-variant
  ">
			<span>{left}</span>
			{right && <span>{right}</span>}
		</div>
	);
}
