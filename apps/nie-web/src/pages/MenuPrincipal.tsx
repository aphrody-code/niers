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
 * Le titre, les entrées, la mention légale. Une information n'apparaît qu'à un seul endroit, et
 * aucune ne décrit l'infrastructure : ni service, ni version, ni endpoint, ni compte d'index.
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
	FOND_MENU,
	GameCanvas,
	GLYPHES,
	HeroPanel,
	IconTile,
	PENTE_PANNEAU,
	TileStrip,
} from "@niers/inacord-ui";
import { useMemo } from "react";
import { entreesMenu } from "../entrees";

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
 * logo. Un aplat seul donne un écran plat que rien ne rattache à la capture ; le dégradé part
 * de la teinte mesurée et ne fait que retrouver cette montée, sans inventer de troisième
 * couleur — `--jeu-ciel-brume` est elle aussi une valeur relevée.
 */
const CIEL = `radial-gradient(70% 55% at 88% -8%, var(--jeu-ciel-brume) 0%, ${FOND_MENU} 70%)`;

export function MenuPrincipal({
	vue,
	onChoisir,
	etat,
	pret,
}: {
	/** L'entrée courante, pour marquer la tuile correspondante. */
	vue: string;
	onChoisir: (vue: string) => void;
	/** Ce que le serveur publie. `null` tant qu'il n'a pas répondu. */
	etat: SanteApi | null;
	/** Le catalogue est-il consultable ? Une tuile ne promet pas ce qu'elle ne peut pas montrer. */
	pret: boolean;
}) {
	const entrees = useMemo(() => entreesMenu(etat), [etat]);

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

			{/* --- Le titre, dans la boîte du logo du jeu -------------------------------------
			    Le sous-titre dit ce QU'EST le site. Il portait « Victory Road » — le sous-titre
			    du jeu accolé au nom du site, qui laissait croire qu'Aphrody est le jeu. */}
			<CanvasItem
				x={BOITES.titre.x + BOITES.titre.l / 2}
				y={BOITES.titre.y + 15}
				ancreX={0.5}
				z={10}
			>
				<div style={{ textAlign: "center", lineHeight: 1 }}>
					<h1
						style={{
							margin: 0,
							fontSize: 82,
							fontWeight: 900,
							letterSpacing: "0.04em",
							color: "var(--jeu-nuit-profonde)",
							textShadow: "0 3px 0 var(--jeu-texte-vif), 0 0 18px rgb(165 225 246 / 90%)",
						}}
					>
						APHRODY
					</h1>
					<p
						style={{
							margin: "30px 0 0",
							fontSize: 19,
							fontWeight: 800,
							letterSpacing: "0.14em",
							textTransform: "uppercase",
							color: "var(--jeu-tuile-bas)",
						}}
					>
						Les fichiers du jeu
					</p>
				</div>
			</CanvasItem>

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

			{/* --- L'état, seulement quand il y a quelque chose à dire ------------------------
			    Le jeu place ici un encart d'information. Aphrody n'en met un que si l'écran ne
			    peut pas encore servir : un bandeau permanent qui répète « tout va bien » occupe
			    l'œil sans rien apprendre. Le message ne nomme ni le service ni l'index — il dit
			    ce que l'utilisateur peut faire, et quand. */}
			{!pret ? (
				<CanvasItem x={BOITES.notice.x} y={BOITES.notice.y} largeur={BOITES.notice.l} z={10}>
					<p
						style={{
							margin: 0,
							padding: "8px 14px",
							background: "rgb(255 255 255 / 88%)",
							borderLeft: "4px solid var(--jeu-accent-azur)",
							color: "var(--jeu-nuit-profonde)",
							fontSize: 13,
							fontWeight: 700,
							lineHeight: 1.4,
						}}
					>
						Préparation du catalogue…
					</p>
				</CanvasItem>
			) : null}

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
