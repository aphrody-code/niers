/**
 * L'écran d'attente d'Aphrody : celui du jeu, pendant que l'index du VFS se monte.
 *
 * ## Ce qu'il remplace
 *
 * Le site affichait « Le catalogue est en cours de préparation. Il s'affichera dès qu'il sera
 * prêt. » — une phrase de service dans une page vide. Le jeu, lui, a un écran de chargement, et
 * il est dans le VFS : `loading01`. Le montrer coûte une texture de 11 Ko et dit exactement la
 * même chose, dans la langue de l'application.
 *
 * ## L'écran vient du jeu, et il a été MESURÉ
 *
 * `nie-game --runtime --menu loading01 --export-layout` rend **un** objet :
 * `loading01_01_fade_loading`, sprite `dx11/menu/11_loading/loading01/loading01_01/
 * loading01_01.g4tx` de 784×136, posé au centre d'un canevas de 1280×720 (`x: 640, y: 360`,
 * ancre 0,5), `drawPriority` 300, `drawType` 3. Aucun script, aucun objet muté par le runtime :
 * c'est un calque statique, donc l'export en dit la vérité entière — contrairement à `title00`,
 * qui rend 67 objets dont 21 restent à la position par défaut et dont les sprites sont des
 * atlas entiers (jusqu'à 5828×6840). L'écran de démarrage exploitable est celui-ci.
 *
 * Le fichier exporté est embarqué tel quel (`../donnees/loading01.layout.json`), comme
 * `mainmenu01.layout.json` : un réexport met l'écran à jour sans qu'une ligne d'ici change.
 *
 * ## La texture est SERVIE, jamais copiée
 *
 * `LayoutRender` demande son URL à la source montée par l'hôte, qui applique la convention
 * `/assets/tex/<chemin VFS sans .g4tx>.png` — vérifié : 200, `image/png`, 11 008 octets. Copier
 * le PNG dans `public/` aurait créé un second jeu d'octets qui se serait périmé au premier
 * réexport, sans que rien ne le dise.
 *
 * ## Rien ne progresse, parce que rien ne mesure une progression
 *
 * `/api/v1/health` publie `capacites.vfs` — `en_cours`, `pret` ou `absent` — et rien d'autre
 * tant que l'index se monte : pas de compte partiel, pas de fraction. Une barre qui se remplirait
 * ici n'aurait aucune donnée derrière elle, et la bande bleue du jeu est justement une texture
 * FIXE : la remplir demanderait d'inventer à la fois le chiffre et le dessin. Le seul élément
 * animé de l'écran est le personnage, dont l'animation `waiting` correspond à un état réel.
 *
 * `vfs_entrees` n'est affiché que lorsqu'il est renseigné et non nul — c'est un compte mesuré,
 * pas une estimation.
 */
import type { SanteApi } from "@niers/asset-source/nie-site";
import { CanvasItem, GameCanvas, LayoutRender, lireLayout } from "@niers/inacord-ui";
import { useMemo } from "react";
import brut from "../donnees/loading01.layout.json";
import { type Humeur, PetAphrody } from "./PetAphrody";

/** Le canevas du jeu, en pixels du jeu. Il vient du layout, il n'est pas écrit ici. */
const LAYOUT = lireLayout(brut);

/**
 * Où poser le personnage, au-dessus de la bande.
 *
 * La bande fait 136 px de haut et son ancre est au centre du canevas : elle occupe donc
 * y 292→428. Le personnage est posé au-dessus, ancré par son bas, à 32 px du haut de la bande —
 * la seule contrainte est de ne pas la recouvrir, et cette position est une composition
 * d'Aphrody, pas une mesure du jeu.
 */
const HAUT_BANDE = LAYOUT.canvas.h / 2 - 136 / 2;
const Y_PET = HAUT_BANDE - 32;

/** La ligne de texte, sous la bande. */
const Y_LIGNE = LAYOUT.canvas.h / 2 + 136 / 2 + 40;

/** Ce que l'écran a le droit de dire, pour chaque état que le serveur publie réellement. */
type EtatVfs = SanteApi["capacites"]["vfs"];

/**
 * La phrase de chaque état.
 *
 * Aucune ne chiffre ni ne promet de délai : le serveur n'en publie pas, et une durée annoncée
 * puis dépassée est pire qu'aucune durée.
 */
const PHRASE: Record<EtatVfs, string> = {
	en_cours: "L'index des fichiers du jeu se monte.",
	pret: "Les fichiers du jeu sont joignables.",
	absent: "Les fichiers du jeu ne sont pas joignables.",
};

/** Ce que le personnage exprime, décidé sur l'état mesuré et sur lui seul. */
function humeurPour(etat: EtatVfs | null, panne: boolean): Humeur {
	if (panne || etat === "absent") return "panne";
	if (etat === "pret") return "repos";
	// `null` compris : tant que la mesure n'est pas revenue, on attend — c'est littéralement
	// ce qui se passe, et « repos » laisserait croire que l'écran a fini.
	return "attente";
}

export interface ProprietesChargement {
	/**
	 * La dernière réponse de `/api/v1/health`, ou `null` tant qu'aucune n'est revenue.
	 *
	 * L'écran ne SONDE pas : c'est l'hôte qui interroge le serveur et qui remonte le résultat.
	 * Un composant qui déclencherait son propre `fetch` en ferait un second, désynchronisé de
	 * celui de l'application, et deux mesures du même fait finissent toujours par diverger.
	 */
	etat: SanteApi | null;
	/** Le site ne joint pas ses ressources du tout — l'hôte le sait, l'écran ne le devine pas. */
	panne?: boolean;
	/** Ce que l'écran annonce, quand l'appelant a mieux à dire que l'état du VFS. */
	titre?: string;
}

/**
 * L'écran de chargement : la bande du jeu, le personnage, et la seule phrase que l'on sait vraie.
 *
 * À monter en plein écran (`position: fixed; inset: 0`) : `GameCanvas` mesure la place qu'on lui
 * donne et met le repère 1280×720 à l'échelle. Il occupe la hauteur de son parent, donc un
 * parent sans hauteur lui en donne zéro.
 */
export function Chargement({ etat, panne = false, titre }: ProprietesChargement) {
	const vfs = etat?.capacites.vfs ?? null;
	const humeur = humeurPour(vfs, panne);

	const ligne = useMemo(() => {
		if (panne) return "Le site ne parvient pas à joindre ses ressources.";
		if (!vfs) return "Mesure en cours.";
		return PHRASE[vfs];
	}, [panne, vfs]);

	// Le compte n'est cité que s'il est réellement mesuré : le serveur rend `0` tant que l'index
	// n'existe pas, et « 0 entrée » serait un chiffre faux plutôt qu'une absence de chiffre.
	const entrees = etat?.capacites.vfs_entrees ?? 0;

	return (
		<GameCanvas canvas={LAYOUT.canvas} fond="var(--jeu-ciel-clair)">
			{/* La bande du jeu, dessinée par le rendu générique de layout : ce composant ne
			    connaît pas `loading01`, il pose ce que l'export contient. */}
			<LayoutRender layout={LAYOUT} />

			<CanvasItem x={LAYOUT.canvas.w / 2} y={Y_PET} ancreX={0.5} ancreY={1} z={400}>
				<PetAphrody humeur={humeur} />
			</CanvasItem>

			<CanvasItem x={LAYOUT.canvas.w / 2} y={Y_LIGNE} ancreX={0.5} ancreY={0} z={400}>
				{/* `role="status"` et non une simple ligne de texte : l'écran change d'état sans
				    aucune action de l'utilisateur, et c'est le seul moyen qu'un lecteur d'écran
				    l'apprenne. `aria-live` poli, pour ne pas couper une lecture en cours. */}
				<div
					role="status"
					style={{
						display: "flex",
						flexDirection: "column",
						alignItems: "center",
						gap: "var(--jeu-espace-xs)",
						color: "var(--jeu-texte-doux)",
						font: "500 20px/1.4 system-ui, sans-serif",
						letterSpacing: "var(--jeu-libelle-espacement)",
						textAlign: "center",
					}}
				>
					<span>{titre ?? ligne}</span>
					{entrees > 0 ? (
						<span style={{ fontSize: "15px", opacity: 0.8 }}>
							{entrees.toLocaleString("fr-FR")} entrées indexées
						</span>
					) : null}
				</div>
			</CanvasItem>
		</GameCanvas>
	);
}
