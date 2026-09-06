/**
 * Aphrody, le personnage du site — animé, et attentif à ce qui se passe réellement.
 *
 * ## Ce qu'il remplace
 *
 * L'accueil portait « APHRODY » en 82 px et « LES FICHIERS DU JEU » en dessous : le nom du site
 * écrit sur le site, à l'endroit où le jeu met son logo. Le personnage dont le site porte le nom
 * y est plus juste — il dit la même chose sans l'écrire, et il vit.
 *
 * ## Il ne joue pas la comédie
 *
 * Chaque animation correspond à un état mesuré, jamais à un minuteur décoratif :
 *
 * | Ce qui se passe | Animation |
 * |---|---|
 * | le site ne joint pas ses ressources | `failed` |
 * | le catalogue se prépare | `waiting` |
 * | la souris bouge dans la page | une des seize poses de `look-directions` |
 * | au repos | `idle` |
 * | la souris survole le personnage | `waving` |
 * | on le clique | `jumping` |
 *
 * C'est aussi pour cela que le regard passe devant l'ambiance : suivre le curseur est la seule
 * réaction qui dépende du visiteur, et une mascotte qui salue toute seule dans le vide est une
 * décoration, pas une réponse.
 *
 * ## Le sprite vient du serveur, pas du dossier public
 *
 * `nie-aphrody` embarque le package validé au build ; `nie-site` le republie à `/pet/*`. Copier
 * l'atlas dans `public/` aurait créé un second jeu d'octets sans le condensé qui l'accompagne —
 * il se serait périmé au premier réexport, sans que rien ne le dise.
 *
 * ## Deux temps, parce que l'atlas pèse 1,5 Mo
 *
 * L'atlas porte 74 frames : il faut l'avoir en entier pour animer, et rien avant. Attendre son
 * décodage laisserait le centre de l'accueil vide le temps du transfert — c'est ce qui s'est vu
 * à la première capture, où la page était complète et la place du personnage blanche. Le premier
 * rendu affiche donc la POSE de repos seule, servie en PNG par `/pet/frame/…` : 30 Ko contre
 * 1,5 Mo, la même image au pixel près puisqu'elle est découpée dans le même atlas. L'animation
 * prend le relais dès que l'atlas est décodé, sans saut visible — la pose de repos est justement
 * ce que la boucle affiche au repos.
 */
import { useCallback, useEffect, useRef, useState } from "react";

/** Une frame : son coin dans l'atlas, sa durée, sa direction. */
interface Frame {
	x: number;
	y: number;
	ms?: number;
	direction?: string;
}

/** Une animation séquencée du package. */
interface Animation {
	duree_ms: number;
	role?: string;
	frames: Frame[];
}

/** Le manifeste réduit servi par `/pet/aphrody.json`. */
interface ManifestePet {
	pet: { id: string; nom: string; version: number };
	atlas: { url: string; largeur: number; hauteur: number; cellule_l: number; cellule_h: number };
	animations: Record<string, Animation>;
	poses: Record<string, Frame>;
	regard?: { pas_degres: number; poses: Frame[] };
}

/** Ce que le personnage doit exprimer, décidé par l'appelant sur des faits. */
export type Humeur = "repos" | "attente" | "panne";

/** L'animation d'ambiance de chaque humeur, dans les noms du package. */
const AMBIANCE: Record<Humeur, string> = {
	repos: "idle",
	attente: "waiting",
	panne: "failed",
};

/** Combien de temps une pose de regard survit au dernier mouvement de souris. */
const REGARD_MS = 2500;

/** Combien de temps l'animation déclenchée par un clic tient avant de rendre la main. */
const REACTION_MS = 900;

/** La pose servie seule au premier rendu, avant que l'atlas ne soit là. */
const POSE_REPOS = "look-neutral";

/** L'URL de la pose de repos en PNG — 30 Ko, découpés dans l'atlas par le serveur. */
const URL_POSE_REPOS = `/pet/frame/${POSE_REPOS}/0.png`;

/**
 * Charge le manifeste une seule fois pour toute la page.
 *
 * Un `useEffect` par instance rechargerait le manifeste à chaque montage — deux personnages à
 * l'écran, deux requêtes pour le même document. La promesse est mémorisée au niveau du module :
 * le navigateur ne voit qu'un aller-retour, et les rendus suivants sont synchrones.
 */
let promesse: Promise<ManifestePet> | null = null;
function chargerManifeste(): Promise<ManifestePet> {
	promesse ??= fetch("/pet/aphrody.json").then((r) => {
		if (!r.ok) throw new Error(String(r.status));
		return r.json() as Promise<ManifestePet>;
	});
	return promesse;
}

/**
 * La pose de regard qui vise un angle donné.
 *
 * L'angle est celui du package : 0° vers le HAUT, puis sens horaire. Ce n'est pas la convention
 * de `Math.atan2`, qui part de l'axe des x vers la droite et tourne dans l'autre sens à l'écran
 * — la conversion est faite une fois, ici, plutôt que répétée à chaque appel.
 */
function poseVers(
	regard: NonNullable<ManifestePet["regard"]>,
	dx: number,
	dy: number,
): Frame | undefined {
	const degres = (Math.atan2(dx, -dy) * 180) / Math.PI;
	const normalise = ((degres % 360) + 360) % 360;
	const index = Math.round(normalise / regard.pas_degres) % regard.poses.length;
	return regard.poses[index] ?? regard.poses[0];
}

/** La frame courante d'une animation, selon l'horloge. */
function frameA(animation: Animation, ecouleMs: number): Frame | undefined {
	const cycle = ecouleMs % Math.max(animation.duree_ms, 1);
	let debut = 0;
	for (const frame of animation.frames) {
		debut += frame.ms ?? 0;
		if (cycle < debut) return frame;
	}
	return animation.frames[animation.frames.length - 1];
}

export function PetAphrody({
	humeur,
	echelle = 1,
	onClick,
}: {
	humeur: Humeur;
	/** Facteur d'affichage appliqué à la cellule de 192×208. */
	echelle?: number;
	/** Ce que le clic déclenche en plus de l'animation — souvent rien. */
	onClick?: () => void;
}) {
	const [manifeste, setManifeste] = useState<ManifestePet | null>(null);
	const [frame, setFrame] = useState<Frame | null>(null);
	// L'atlas est-il décodé ? Tant qu'il ne l'est pas, on montre la pose seule : un
	// `background-image` qui n'a pas fini de charger n'affiche RIEN, sans le dire.
	const [atlasPret, setAtlasPret] = useState(false);
	// Les trois signaux qui décident de ce qu'on voit. Des `ref` et non des `state` : ils sont
	// lus soixante fois par seconde dans la boucle d'animation, et un rendu React par mouvement
	// de souris coûterait bien plus cher que le dessin lui-même.
	const survol = useRef(false);
	const reactionJusqua = useRef(0);
	const curseur = useRef<{ dx: number; dy: number; a: number } | null>(null);
	const zone = useRef<HTMLButtonElement | null>(null);

	useEffect(() => {
		let vivant = true;
		chargerManifeste()
			.then((m) => {
				if (vivant) setManifeste(m);
			})
			.catch(() => {
				// Le personnage est un ornement : son absence ne doit rien casser, et surtout
				// pas afficher un message d'erreur là où l'on attend un visage.
			});
		return () => {
			vivant = false;
		};
	}, []);

	// Le décodage de l'atlas est attendu explicitement. `img.decode()` rend la main quand les
	// pixels sont prêts, là où `onload` ne dit que « les octets sont arrivés » — et c'est bien
	// le décodage qui manquait au premier affichage.
	useEffect(() => {
		if (!manifeste) return;
		let vivant = true;
		const image = new Image();
		image.src = manifeste.atlas.url;
		image
			.decode()
			.then(() => {
				if (vivant) setAtlasPret(true);
			})
			.catch(() => {
				// L'atlas est illisible : la pose fixe reste affichée, ce qui vaut mieux qu'une
				// place vide au centre de l'accueil.
			});
		return () => {
			vivant = false;
		};
	}, [manifeste]);

	// Le curseur est suivi au niveau de la FENÊTRE, pas de l'élément : le regard n'a d'intérêt
	// que s'il porte au-delà du personnage.
	useEffect(() => {
		const bouge = (e: MouseEvent) => {
			const boite = zone.current?.getBoundingClientRect();
			if (!boite) return;
			curseur.current = {
				dx: e.clientX - (boite.left + boite.width / 2),
				// La cible du regard vise le haut du corps, pas son centre géométrique : le
				// personnage a des jambes, et un angle mesuré depuis le milieu de l'image le
				// fait loucher vers le bas dès que la souris est proche.
				dy: e.clientY - (boite.top + boite.height * 0.35),
				a: performance.now(),
			};
		};
		window.addEventListener("mousemove", bouge, { passive: true });
		return () => window.removeEventListener("mousemove", bouge);
	}, []);

	// La boucle : une seule, en `requestAnimationFrame`, qui ne pose un état que lorsque la
	// frame CHANGE. Sans cette comparaison, React re-rendrait soixante fois par seconde pour
	// afficher exactement la même cellule.
	useEffect(() => {
		if (!manifeste || !atlasPret) return;
		let brut = 0;
		const debut = performance.now();
		const ambiance = manifeste.animations[AMBIANCE[humeur]] ?? manifeste.animations.idle;
		const boucle = (maintenant: number) => {
			brut = requestAnimationFrame(boucle);
			const ecoule = maintenant - debut;
			let suivante: Frame | undefined;
			if (maintenant < reactionJusqua.current) {
				// Une réaction volontaire l'emporte sur tout le reste : elle répond à un geste.
				const reaction = manifeste.animations.jumping;
				if (reaction) suivante = frameA(reaction, ecoule);
			} else if (survol.current && manifeste.animations.waving) {
				suivante = frameA(manifeste.animations.waving, ecoule);
			} else if (
				manifeste.regard &&
				curseur.current &&
				maintenant - curseur.current.a < REGARD_MS
			) {
				suivante = poseVers(manifeste.regard, curseur.current.dx, curseur.current.dy);
			} else if (ambiance) {
				suivante = frameA(ambiance, ecoule);
			}
			if (suivante) {
				const posee = suivante;
				setFrame((connue) =>
					connue && connue.x === posee.x && connue.y === posee.y ? connue : posee,
				);
			}
		};
		brut = requestAnimationFrame(boucle);
		return () => cancelAnimationFrame(brut);
	}, [manifeste, humeur, atlasPret]);

	const auClic = useCallback(() => {
		reactionJusqua.current = performance.now() + REACTION_MS;
		onClick?.();
	}, [onClick]);

	if (!manifeste) {
		// Rien plutôt qu'un cadre vide : la place du personnage reste tenue par la mise en page
		// qui l'entoure, et un rectangle gris pendant le chargement se remarque plus que son
		// absence.
		return null;
	}
	const { atlas } = manifeste;
	const pose = frame ?? manifeste.poses[POSE_REPOS] ?? { x: 0, y: 0 };
	const l = atlas.cellule_l * echelle;
	const h = atlas.cellule_h * echelle;
	// Avant le décodage, la pose seule remplit la case ; après, c'est l'atlas décalé.
	const fond = atlasPret
		? {
				backgroundImage: `url(${atlas.url})`,
				backgroundSize: `${atlas.largeur * echelle}px ${atlas.hauteur * echelle}px`,
				backgroundPosition: `-${pose.x * echelle}px -${pose.y * echelle}px`,
			}
		: {
				backgroundImage: `url(${URL_POSE_REPOS})`,
				backgroundSize: `${l}px ${h}px`,
				backgroundPosition: "0 0",
			};

	return (
		<button
			type="button"
			ref={zone}
			onMouseEnter={() => {
				survol.current = true;
			}}
			onMouseLeave={() => {
				survol.current = false;
			}}
			onClick={auClic}
			// Le nom vient du manifeste, pas d'une chaîne écrite ici : c'est le package qui sait
			// comment le personnage s'appelle.
			aria-label={manifeste.pet.nom}
			style={{
				width: l,
				height: h,
				padding: 0,
				border: 0,
				backgroundColor: "transparent",
				backgroundRepeat: "no-repeat",
				...fond,
				cursor: "pointer",
				// Le canevas du menu agrandit tout son contenu ; sans consigne, un fond
				// redimensionné est interpolé au plus vite et le sprite crénelle. `high-quality`
				// demande le meilleur filtrage disponible — c'est le seul réglage qui porte sur
				// une image de FOND, `imageRendering` sur un `<img>` ne s'appliquerait pas ici.
				imageRendering: "high-quality",
				// Le personnage n'est pas un bouton de formulaire : il ne doit pas hériter d'un
				// contour de focus rectangulaire autour de sa cellule transparente.
				outline: "none",
			}}
		/>
	);
}
