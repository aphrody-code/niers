/**
 * L'accueil d'Aphrody : le menu principal, et rien d'autre.
 *
 * ## Ce que cet écran a cessé d'afficher, et pourquoi
 *
 * Il portait, tous en même temps : le total des ressources à trois endroits (l'encart d'état,
 * la plaque centrale, le panneau gauche), le compte de chaque catalogue à deux (le panneau
 * gauche et l'appoint des tuiles), un bouton « Calque » à trois, deux chemins vers l'explorateur
 * et deux vers le flux Atom. S'y ajoutaient le nom et la version du service (« nie-site 0.5.9 »),
 * le nombre d'entrées indexées du VFS, une tuile vers `/api/v1/health`, des bannières vers
 * `sitemap.xml`, `llms.txt` et GitHub, le domaine du site écrit sur le site lui-même, et deux
 * guides de touches — « F » et « V » — que rien n'écoutait : aucun raccourci clavier n'existe
 * dans cette application.
 *
 * Le calque exporté du jeu, enfin, était rendu SOUS l'interface à 18 % d'opacité. Son texte
 * restait dans le document : « Photos commémoratives disponibles après », « Exclure plusieurs
 * joueurs », « Fusion rapide », « Bonus d'équipe » — des libellés d'un autre écran du jeu, lus
 * par les lecteurs d'écran et les moteurs, superposés au milieu de la page d'accueil. C'est un
 * outil de comparaison ; sa place est dans la validation, pas en façade.
 *
 * ## Ce qui reste
 *
 * Le personnage, les entrées, la mention légale. Une information n'apparaît qu'à un seul
 * endroit, et aucune ne décrit l'infrastructure : ni service, ni version, ni endpoint, ni compte
 * d'index. Le nom du site lui-même n'est plus écrit au centre de l'écran — il est dans l'onglet,
 * dans la barre des écrans secondaires, et c'est le personnage qui le porte ici.
 *
 * ## La géométrie reste mesurée — une seule position est recalculée, et elle le dit
 *
 * Les positions viennent de `BOITES`, mesurées sur une capture du jeu par
 * `scripts/validation/mesurer-mainmenu.py` — pas de l'œil. Panneaux, logo et mention légale
 * gardent la place que la mesure leur donne. La rangée de tuiles fait exception : le jeu la
 * pose haut parce que trois blocs la suivent, et ces blocs n'existent plus ici. Elle est donc
 * recentrée dans l'espace libre, entre deux bornes qui restent, elles, des mesures (§
 * [`Y_RANGEE`]). Un écart assumé et écrit, plutôt qu'une fidélité affichée et fausse.
 */
import type { SanteApi } from "@niers/asset-source";
import {
	BOITES,
	CanvasItem,
	ECART_TUILE,
	GameCanvas,
	type GameHint,
	GameHintBar,
	GameInfoWindow,
	GameSearchBar,
	GLYPHES,
	HeroPanel,
	IconTile,
	PENTE_PANNEAU,
	TileStrip,
} from "@niers/inacord-ui";
import { useMemo, useState } from "react";
import { EXPLORATEUR, entreesMenu } from "../entrees";
import { type Humeur, PetAphrody } from "./PetAphrody";

/**
 * Les touches de l'accueil — reprises de `data/menu/main_menu.png`.
 *
 * « X » ouvre l'encart d'informations, comme le bandeau « X Informations » du jeu. « F » donne
 * le focus à la recherche : le jeu n'a pas de recherche sur son menu (« Alt » y ouvre l'Inazuma
 * Post), et emprunter « X » à la Banque aurait fait deux gestes pour une touche. Chaque touche
 * dessinée en bas d'écran est branchée ici — c'est la règle que le dépôt s'est donnée après
 * avoir affiché des « F » et des « V » que rien n'écoutait.
 */
const TOUCHE_INFORMATIONS = "x";
const TOUCHE_RECHERCHE = "f";

/** Le canevas du menu, en pixels du jeu. Les enfants travaillent tous dans ce repère. */
const CANEVAS = { w: 1280, h: 720 };

/** Le décalage du bord intérieur d'un panneau, entre son haut et son bas. */
const BISEAU_PANNEAU = Math.round(PENTE_PANNEAU * BOITES.panneaux.h);

/**
 * La largeur des deux panneaux, déduite de leurs bords mesurés.
 *
 * Le panneau gauche va du bord de l'écran à son bord intérieur le plus large (en bas) ; le
 * droit part du sien jusqu'au bord droit. Les 228 px qu'ils laissent entre eux sont ce qui
 * donne sa place au logo.
 */
const PANNEAU_GAUCHE_L = BOITES.panneauGaucheBord.bas;
const PANNEAU_DROIT_X = BOITES.panneauDroitBord.bas;
const PANNEAU_DROIT_L = CANEVAS.w - PANNEAU_DROIT_X;

/** Le centre mesuré de la rangée principale — la rangée du jeu n'est pas centrée sur l'écran. */
const CENTRE_RANGEE = BOITES.rangee.x + BOITES.rangee.l / 2;

/**
 * La hauteur où poser la rangée, recalculée au lieu d'être reprise telle quelle.
 *
 * `BOITES.rangee.y` vaut 377 : dans le jeu, la rangée est haute parce que TROIS blocs
 * l'accompagnent en dessous — un bandeau, une seconde rangée de trois tuiles, et la pile de
 * bannières du coin. Ces blocs contenaient ici des liens vers l'API, le plan du site, `llms.txt`
 * et GitHub ; les retirer laissait 212 px de vide sous la rangée, et une géométrie qui ne
 * décrivait plus l'écran qu'elle habillait.
 *
 * La rangée est donc centrée dans l'espace réellement libre — entre le bas des panneaux et le
 * haut de la mention légale. Les bornes restent des mesures ; seule leur combinaison change.
 */
const BAS_PANNEAUX = BOITES.panneaux.y + BOITES.panneaux.h;
const Y_RANGEE = Math.round(
	BAS_PANNEAUX + (BOITES.mention.y - BAS_PANNEAUX - BOITES.rangee.h) / 2,
);

/**
 * Le ciel du menu.
 *
 * La quantification par région rend `#f9fdf9` — la teinte dominante, celle de `FOND_MENU`. Mais
 * la référence n'est pas un aplat : elle s'éclaire en bleu vers le haut à droite, derrière le
 * logo. Un aplat seul donne un écran plat que rien ne rattache à la capture ; le dégradé ne
 * fait que retrouver cette montée, sans inventer de troisième couleur — `--jeu-ciel-brume` est
 * elle aussi une valeur relevée.
 *
 * Le point de départ n'est plus `FOND_MENU` mais `--jeu-ciel-clair`. Les deux disent la même
 * chose à un cheveu près (`#f9fdf9` verdâtre contre `#f9f6f4` crème), et c'est justement le
 * problème : le canevas portait l'un, le `<body>` l'autre, et la démarcation se voyait en bas
 * d'écran sous la forme d'une bande d'une autre teinte. La palette de `nie-aphrody` est la
 * source unique des couleurs du site ; une constante de géométrie n'a pas à en porter une
 * seconde.
 */
/**
 * Diamètre du halo posé derrière le personnage, en pixels du canevas.
 *
 * 288 : la cellule du sprite fait 208 px de haut, le halo doit donc déborder d'environ 40 px de
 * chaque côté pour que le dégradé s'éteigne AVANT le bord du dessin — sinon la coupure du
 * cercle se voit derrière les pieds.
 */
const HALO = 288;

const CIEL = `radial-gradient(70% 55% at 88% -8%, var(--jeu-ciel-brume) 0%, var(--jeu-ciel-clair) 70%)`;

/**
 * L'échelle du personnage sur le canevas du menu.
 *
 * Elle valait 1,35 — la hauteur de la boîte du logo (287 px) remplie à deux pixels près. Mais
 * le canevas est lui-même mis à l'échelle : sur une fenêtre de 1440 px, 1280 devient 1440, soit
 * ×1,125 de plus. Le sprite de 208 px finissait donc affiché à 316 px, un agrandissement de
 * 1,52 qui rendait chaque pixel de la source visible.
 *
 * À 1,0 le personnage est rendu à sa taille native dans le canevas, et le seul agrandissement
 * qui reste est celui du canevas — le même que celui des tuiles et du reste de l'écran. Il est
 * plus petit que la boîte du logo, et c'est le prix de sa netteté.
 */
const ECHELLE_PET = 1;

/**
 * Retirer un élément de la vue SANS le retirer du document.
 *
 * `display: none` et `visibility: hidden` le retireraient aussi de l'arbre d'accessibilité :
 * la page perdrait son `h1`, ce qui est précisément ce qu'on veut éviter. Un rectangle de un
 * pixel écrêté reste lu.
 */
const HORS_VUE = {
	position: "absolute",
	width: 1,
	height: 1,
	margin: -1,
	padding: 0,
	overflow: "hidden",
	clip: "rect(0 0 0 0)",
	whiteSpace: "nowrap",
	border: 0,
} as const;

export function MenuPrincipal({
	vue,
	onChoisir,
	etat,
	pret,
	panne,
}: {
	/** L'entrée courante, pour marquer la tuile correspondante. */
	vue: string;
	onChoisir: (vue: string) => void;
	/** Ce que le serveur publie. `null` tant qu'il n'a pas répondu. */
	etat: SanteApi | null;
	/** Le catalogue est-il consultable ? Une tuile ne promet pas ce qu'elle ne peut pas montrer. */
	pret: boolean;
	/** Le site joint-il ses ressources ? C'est ce que le personnage exprime en premier. */
	panne: boolean;
}) {
	const entrees = useMemo(() => entreesMenu(etat), [etat]);
	// Le personnage ne joue pas la comédie : son animation est décidée par l'état MESURÉ du
	// service, dans cet ordre — une panne prime sur une attente, une attente sur le repos.
	const humeur: Humeur = panne ? "panne" : pret ? "repos" : "attente";
	// L'encart d'informations est replié par défaut : le jeu ne montre que son bandeau tant
	// qu'on n'appuie pas sur X, et une carte permanente qui répète le menu occupe l'œil pour rien.
	const [informations, setInformations] = useState(false);
	const [recherche, setRecherche] = useState("");

	/**
	 * Chercher depuis l'accueil mène à l'explorateur, en portée « partout ».
	 *
	 * L'état de l'explorateur vit dans l'URL (`?q=&partout=1`) : l'écrire AVANT de changer de
	 * vue suffit, `setVue` ne touche que le chemin et l'explorateur lit sa requête au montage.
	 */
	const chercher = (q: string) => {
		if (!q) return;
		const url = new URL(window.location.href);
		url.search = `q=${encodeURIComponent(q)}&partout=1`;
		window.history.replaceState(window.history.state, "", url);
		onChoisir(EXPLORATEUR);
	};

	const touches = useMemo<GameHint[]>(
		() => [
			{
				key: TOUCHE_INFORMATIONS,
				label: "Informations",
				onActivate: () => setInformations((v) => !v),
			},
			{
				key: TOUCHE_RECHERCHE,
				label: "Chercher",
				onActivate: () => {
					document.querySelector<HTMLInputElement>("#accueil-recherche input")?.focus();
				},
			},
		],
		[],
	);

	return (
		<GameCanvas canvas={CANEVAS} fond={CIEL}>
			{/* --- Les deux panneaux, de part et d'autre du titre -----------------------------
			    Ils sont DÉCORATIFS : c'est leur forme qui signe l'écran, et le jeu y met des
			    illustrations, pas des données. Leur donner un titre et un clic dupliquait deux
			    entrées de la rangée juste en dessous — « Ressources » ouvrait le premier
			    catalogue, « Explorer » l'explorateur, tous deux déjà là. `aria-hidden` parce
			    qu'un décor annoncé est du bruit pour qui écoute la page. */}
			<CanvasItem x={0} y={BOITES.panneaux.y} largeur={PANNEAU_GAUCHE_L} z={5}>
				<div aria-hidden="true">
					<HeroPanel cote="gauche" penche={BISEAU_PANNEAU} />
				</div>
			</CanvasItem>
			<CanvasItem x={PANNEAU_DROIT_X} y={BOITES.panneaux.y} largeur={PANNEAU_DROIT_L} z={5}>
				<div aria-hidden="true">
					<HeroPanel cote="droite" penche={BISEAU_PANNEAU} />
				</div>
			</CanvasItem>

			{/* --- Le personnage, dans la boîte du logo du jeu --------------------------------
			    Le nom du site n'y est plus écrit. Il portait « APHRODY » en 82 px et « LES
			    FICHIERS DU JEU » dessous — le nom du site sur le site, à l'endroit où le jeu met
			    son logo, et un sous-titre qui répétait la description déjà servie dans l'en-tête
			    du document. Le personnage dont le site porte le nom dit la même chose sans
			    l'écrire, et il réagit à l'état réel du service. */}
			{/* Le halo. Le personnage porte une tenue CLAIRE — mesuré sur la pose servie : sa
			    dominante est un blanc cassé — et le ciel du menu est passé au crème de la palette.
			    Posé tel quel, il disparaissait purement et simplement : le DOM était juste, la
			    capture vide, et rien dans le code n'était faux. Le halo n'est pas une décoration,
			    c'est ce qui le rend visible. Il est `aria-hidden` : il ne dit rien. */}
			<CanvasItem
				x={BOITES.titre.x + BOITES.titre.l / 2}
				y={BOITES.titre.y + Math.round((BOITES.titre.h - HALO) / 2)}
				ancreX={0.5}
				z={9}
			>
				<div
					aria-hidden="true"
					style={{
						width: HALO,
						height: HALO,
						borderRadius: "50%",
						background:
							"radial-gradient(circle, var(--jeu-ciel-brume) 0%, var(--jeu-surface-glace) 48%, transparent 70%)",
					}}
				/>
			</CanvasItem>

			<CanvasItem
				x={BOITES.titre.x + BOITES.titre.l / 2}
				y={BOITES.titre.y + Math.round((BOITES.titre.h - 208 * ECHELLE_PET) / 2)}
				ancreX={0.5}
				z={10}
			>
				<PetAphrody humeur={humeur} echelle={ECHELLE_PET} />
			</CanvasItem>

			{/* Le titre reste dans le document, hors de la vue : la page a besoin d'un `h1` —
			    le serveur en rend un, React remplace ce qu'il a rendu, et un écran qui n'en a
			    plus se présente sans niveau de titre à qui l'écoute. */}
			<h1 style={HORS_VUE}>Aphrody</h1>

			{/* --- La rangée principale : les entrées du site, et la seule zone interactive ---
			    Sans compte sous le libellé : le jeu n'en met pas, et le chiffre était déjà écrit
			    dans le panneau de gauche. Une tuile sert à choisir une destination, pas à
			    publier un inventaire. */}
			<CanvasItem x={CENTRE_RANGEE} y={Y_RANGEE} ancreX={0.5} z={20}>
				<nav aria-label="Entrées du site">
					<TileStrip ecart={ECART_TUILE}>
						{entrees.map((entree) => (
							<IconTile
								key={entree.vue}
								icone={GLYPHES[entree.glyphe]}
								libelle={entree.libelle}
								actif={entree.vue === vue}
								// En sourdine tant que le catalogue n'est pas consultable : la tuile
								// ne promet pas un contenu qu'elle ne peut pas encore montrer.
								sourdine={!pret}
								onClick={() => onChoisir(entree.vue)}
							/>
						))}
					</TileStrip>
				</nav>
			</CanvasItem>

			{/* --- L'encart d'informations, à la place de celui du jeu ------------------------
			    Le jeu pose ici une carte d'actualité et son bandeau « X Informations ». Le
			    bandeau est permanent, la carte s'ouvre à la touche. Ce qu'elle dit ne décrit ni
			    service, ni version, ni index — seulement ce que l'écran permet, et quand : tant
			    que le catalogue se prépare, c'est la première ligne. */}
			<CanvasItem x={BOITES.notice.x + 8} y={BOITES.notice.y} largeur={BOITES.notice.l} z={10}>
				<GameInfoWindow
					title={pret ? "Bienvenue" : "Préparation du catalogue…"}
					action={{
						keyLabel: "X",
						label: informations ? "Fermer" : "Informations",
						onActivate: () => setInformations((v) => !v),
					}}
				>
					{informations ? (
						<>
							<div>Médias : textures, modèles, sons et vidéos du jeu.</div>
							<div>Explorer : l'arborescence complète, dossier par dossier.</div>
						</>
					) : null}
				</GameInfoWindow>
			</CanvasItem>

			{/* --- La recherche, dans l'encart haut-droit ---------------------------------------
			    Le jeu y met « Alt Inazuma Post » ; Aphrody y met la seule question qu'on pose à
			    un site de ressources : où est ce fichier. La barre porte sa touche, et la touche
			    est branchée par la barre de guides du bas. */}
			<CanvasItem
				x={BOITES.encartHautDroit.x}
				y={BOITES.encartHautDroit.y}
				largeur={BOITES.encartHautDroit.l}
				z={10}
			>
				<div id="accueil-recherche">
					<GameSearchBar
						value={recherche}
						onChange={setRecherche}
						onSubmit={chercher}
						placeholder="Chercher un fichier"
						label="Chercher un fichier du jeu"
						hotkey={TOUCHE_RECHERCHE}
					/>
				</div>
			</CanvasItem>

			{/* --- Le guide de touches du bas-centre, à la place de celui du jeu --------------- */}
			<CanvasItem
				x={BOITES.aide.x + BOITES.aide.l / 2}
				y={BOITES.aide.y}
				ancreX={0.5}
				z={20}
			>
				<GameHintBar hints={touches} />
			</CanvasItem>

			{/* --- La mention légale, à la place où le jeu met la sienne --- */}
			<CanvasItem
				x={BOITES.mention.x + BOITES.mention.l}
				y={BOITES.mention.y}
				ancreX={1}
				z={20}
			>
				<span
					style={{
						fontSize: 17,
						fontWeight: 700,
						color: "var(--jeu-tuile-bas)",
						whiteSpace: "nowrap",
					}}
				>
					Ressources © LEVEL-5 Inc.
				</span>
			</CanvasItem>
		</GameCanvas>
	);
}
