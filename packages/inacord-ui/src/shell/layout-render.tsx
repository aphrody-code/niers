/**
 * Le rendu d'un layout de menu du jeu, dans le navigateur comme dans Inacord.
 *
 * ## Le layout est une DONNEE
 *
 * Ce composant ne connait aucun ecran en particulier. On lui passe le JSON produit par
 * `nie-game --runtime --menu <ecran> --export-layout`, et il le dessine. Un reexport met donc
 * l'interface a jour sans qu'une ligne de composant change — c'est toute la manoeuvre : ce qui
 * vient du jeu reste dans le jeu, et le code ne fait que le poser.
 *
 * ## Il ne connait pas non plus son hote
 *
 * L'URL d'une texture est demandee a la source montee par l'hote (`useAssetSource`). Aphrody
 * la sert par `/assets/tex/...`, Inacord la decode en natif : le composant ne sait pas lequel
 * des deux l'heberge, et n'a pas a le savoir.
 *
 * ## Ce qu'il ne fait PAS
 *
 * Il ne corrige aucune position. Sur `mainmenu01`, 24 objets sur 34 sont a la position par
 * defaut et 5 sortent du canevas : le mode `diagnostic` les DESIGNE au lieu de les taire, mais
 * rien ne les deplace. Cf. `layout-jeu.ts` pour la mesure.
 */
import type { CSSProperties, ReactNode } from "react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useAssetSource } from "../source";
import {
	auCentreParDefaut,
	type CanvasLayout,
	cheminVfsSprite,
	dansCanvas,
	echellePourZone,
	estMuet,
	type LayoutJeu,
	type ObjetLayout,
	objetsTries,
	segmentsTexte,
	styleObjet,
} from "./layout-jeu";

/**
 * L'echelle courante d'une zone qui doit contenir le canevas, mesuree et non supposee.
 *
 * `ResizeObserver` plutot qu'un ecouteur de `resize` : la zone peut changer de taille sans que
 * la fenetre bouge (panneau lateral replie, barre d'etat qui apparait), et un menu qui ne se
 * remet a l'echelle qu'au redimensionnement de la fenetre reste faux jusqu'au prochain geste.
 *
 * Rend `0` tant que rien n'a ete mesure : l'appelant sait alors qu'il n'affiche pas encore une
 * mesure, au lieu de croire a une echelle de 1 qui serait fausse.
 */
export function useEchelleCanvas(canvas: CanvasLayout) {
	const zone = useRef<HTMLDivElement | null>(null);
	const [echelle, setEchelle] = useState(0);
	useEffect(() => {
		const noeud = zone.current;
		if (!noeud) return;
		const mesurer = () => {
			const r = noeud.getBoundingClientRect();
			setEchelle(echellePourZone(r.width, r.height, canvas));
		};
		mesurer();
		if (typeof ResizeObserver === "undefined") return;
		const ro = new ResizeObserver(mesurer);
		ro.observe(noeud);
		return () => ro.disconnect();
	}, [canvas]);
	return { zone, echelle };
}

/**
 * La scene : un canevas aux dimensions du jeu, mis a l'echelle de la place disponible.
 *
 * Les enfants travaillent donc TOUJOURS en pixels du jeu (1280x720 pour `mainmenu01`), quelle
 * que soit la taille de l'ecran. C'est la seule facon de poser une coordonnee exportee sans la
 * convertir a chaque usage — et une conversion repetee dans dix composants finit toujours par
 * diverger dans l'un d'eux.
 */
export function GameCanvas({
	canvas,
	children,
	fond,
	className,
}: {
	canvas: CanvasLayout;
	children: ReactNode;
	/** Le fond de la zone, hors du canevas mis a l'echelle. */
	fond?: string;
	className?: string;
}) {
	const { zone, echelle } = useEchelleCanvas(canvas);
	return (
		<div
			ref={zone}
			className={className}
			style={{
				position: "relative",
				width: "100%",
				height: "100%",
				overflow: "hidden",
				background: fond ?? "var(--jeu-fond-abysse)",
				display: "grid",
				placeItems: "center",
			}}
		>
			<div
				style={{
					position: "relative",
					width: `${canvas.w}px`,
					height: `${canvas.h}px`,
					flex: "0 0 auto",
					// Tant que la zone n'est pas mesuree, l'echelle vaut 0 : afficher le canevas a
					// taille reelle pendant une frame provoquerait un saut visible. On le garde
					// invisible plutot que faux.
					transform: `scale(${echelle || 1})`,
					visibility: echelle > 0 ? "visible" : "hidden",
				}}
			>
				{children}
			</div>
		</div>
	);
}

/** Ce que le mode diagnostic met en evidence sur un objet. */
function teinteDiagnostic(objet: ObjetLayout, canvas: CanvasLayout): string | null {
	if (!dansCanvas(objet, canvas)) return "var(--jeu-accent-brique)";
	if (estMuet(objet)) return "var(--jeu-accent-ambre)";
	if (auCentreParDefaut(objet, canvas)) return "var(--jeu-accent-azur)";
	return "var(--jeu-accent-turquoise)";
}

/** Un objet du layout : sa texture en fond, ses fentes de texte par-dessus. */
function ObjetRendu({
	objet,
	canvas,
	url,
	diagnostic,
	baseZ,
	onTexture,
}: {
	objet: ObjetLayout;
	canvas: CanvasLayout;
	url: string | null;
	diagnostic: boolean;
	baseZ: number;
	onTexture?: (nom: string, chargee: boolean) => void;
}) {
	const style = styleObjet(objet, { baseZ });
	const fentes = (objet.text ?? []).filter((t) => t.text.trim() !== "");
	const teinte = diagnostic ? teinteDiagnostic(objet, canvas) : null;
	return (
		<div
			data-objet={objet.name}
			data-priorite={objet.drawPriority}
			title={diagnostic ? `${objet.name} · p${objet.drawPriority}` : undefined}
			style={{
				...style,
				outline: teinte ? `1px solid ${teinte}` : undefined,
				// Le layout est un CALQUE : il ne doit jamais intercepter un clic destine aux
				// entrees posees par-dessus. Une texture de 4 px de large qui avale le clic d'un
				// bouton est un defaut que rien ne rend visible.
				pointerEvents: "none",
			}}
		>
			{url ? (
				<img
					src={url}
					alt=""
					width={objet.sprite?.w ?? undefined}
					height={objet.sprite?.h ?? undefined}
					decoding="async"
					loading="lazy"
					onLoad={onTexture ? () => onTexture(objet.name, true) : undefined}
					// Une texture qui ne repond pas laisse un objet SANS pixel, et rien dans la page
					// ne le dit : c'est le mode d'echec a compter, pas a ignorer.
					onError={onTexture ? () => onTexture(objet.name, false) : undefined}
					style={{
						display: "block",
						width: "100%",
						height: "100%",
						// Ces textures sont des tranches de 4 a 300 px : le lissage du navigateur
						// les rend floues des qu'on agrandit le canevas.
						imageRendering: "pixelated",
					}}
				/>
			) : null}
			{fentes.length > 0 ? (
				<div
					style={{
						position: "absolute",
						inset: 0,
						display: "flex",
						flexDirection: "column",
						alignItems: "center",
						justifyContent: "center",
						gap: 2,
						// L'export ne donne PAS la position d'une fente dans son objet : elle est
						// donc centree, ce qui est une approximation assumee et non une mesure.
						textAlign: "center",
						color: "var(--jeu-texte-vif)",
						textShadow: "0 1px 2px rgb(15 16 17 / 90%)",
						fontSize: 13,
						fontWeight: 700,
						lineHeight: 1.1,
						whiteSpace: "nowrap",
					}}
				>
					{fentes.map((fente) => (
						<span key={fente.slot} data-fente={fente.slot}>
							{segmentsTexte(fente.text).map((seg, i) => (
								// biome-ignore lint/suspicious/noArrayIndexKey: l'ordre EST l'identite d'un segment
								<span key={i} data-couleur={seg.couleur ?? undefined}>
									{seg.texte}
								</span>
							))}
						</span>
					))}
				</div>
			) : null}
		</div>
	);
}

/** Reglages du rendu d'un layout. */
export interface ProprietesLayout {
	/** Le layout a dessiner, deja valide par `lireLayout`. */
	layout: LayoutJeu;
	/**
	 * Construit l'URL d'une texture depuis son chemin VFS.
	 *
	 * Absent : celle de la source montee par l'hote. Le passer sert a rendre un layout hors
	 * ligne (capture, test visuel) sans monter de fournisseur.
	 */
	urlTexture?: (cheminVfs: string) => string;
	/** Encadre chaque objet et le nomme : sert a lire ce que l'export dit vraiment. */
	diagnostic?: boolean;
	/** N'affiche que les objets marques `visible`. Vrai par defaut. */
	visiblesSeules?: boolean;
	/** Opacite du calque entier, pour le poser sous une autre couche. */
	opacite?: number;
	/** Decalage de `z-index`, quand plusieurs calques cohabitent. */
	baseZ?: number;
	/**
	 * Appele pour chaque texture, avec le resultat REEL de son chargement.
	 *
	 * C'est la seule mesure de couverture qui vaille : un chemin peut etre bien construit et la
	 * ressource absente, et l'inverse. Compter dans le navigateur evite d'annoncer une
	 * couverture etablie ailleurs, sur un autre etat du serveur.
	 */
	onTexture?: (nom: string, chargee: boolean) => void;
	style?: CSSProperties;
}

/**
 * Dessine un layout exporte, dans le repere du canevas.
 *
 * A poser dans un [`GameCanvas`] : ce composant ne gere pas l'echelle, il occupe le repere qu'on
 * lui donne. Separer les deux permet d'empiler plusieurs layouts et une interface propre dans
 * le MEME repere, sans les remettre a l'echelle chacun de son cote.
 */
export function LayoutRender({
	layout,
	urlTexture,
	diagnostic = false,
	visiblesSeules = true,
	opacite = 1,
	baseZ = 0,
	onTexture,
	style,
}: ProprietesLayout) {
	const source = useAssetSource();
	const construire = urlTexture ?? source.urlTexture;

	const objets = useMemo(() => {
		const retenus = visiblesSeules ? layout.objects.filter((o) => o.visible) : layout.objects;
		return objetsTries(retenus);
	}, [layout, visiblesSeules]);

	return (
		<div
			data-layout={layout.screen}
			style={{ position: "absolute", inset: 0, opacity: opacite, ...style }}
		>
			{objets.map((objet) => {
				const sprite = objet.sprite;
				const url =
					sprite && construire ? construire(cheminVfsSprite(sprite.logicalPath)) : null;
				return (
					<ObjetRendu
						// Le nom est l'identite d'un objet du layout ; l'export ne les duplique pas.
						key={objet.name}
						objet={objet}
						canvas={layout.canvas}
						url={url}
						diagnostic={diagnostic}
						baseZ={baseZ}
						onTexture={onTexture}
					/>
				);
			})}
		</div>
	);
}
