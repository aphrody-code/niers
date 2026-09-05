"use client";

/**
 * Rendu d'un écran de l'éditeur **depuis son layout**, pas depuis des proportions relevées.
 *
 * Le layout est produit par `nie-game --menu <ecran> --from-setting --runtime --export-layout` et
 * servi par `/avatar/layout/<ecran>.json`. Chaque objet y porte son sprite, sa priorité de dessin
 * et sa transformation, celle-ci résolue à partir des points d'attache déclarés par les
 * `CMenuAttachLocator` de l'écran — c'est-à-dire depuis les fichiers du jeu.
 *
 * Le canevas de référence est celui du layout (1280×720) ; tout est converti en pourcentages du
 * cadre, de sorte que la scène reste juste à n'importe quelle taille d'affichage.
 *
 * Les objets hors canevas sont écartés : ce sont les guides de bouton et les objets empruntés à
 * d'autres écrans, que le jeu ne dessine pas ici.
 */

import type { Layout, LayoutObjet } from "./types";

/** Vrai si l'objet a un sprite et tombe dans le canevas. */
function dessinable(o: LayoutObjet, w: number, h: number): boolean {
	const t = o.transform;
	if (!o.sprite?.pngUrl || !t || o.visible === false) return false;
	if (typeof t.x !== "number" || typeof t.y !== "number") return false;
	return t.x >= 0 && t.x <= w && t.y >= 0 && t.y <= h;
}

export function LayoutRender({
	layout,
	cdn,
	className,
}: {
	layout: Layout;
	cdn: string;
	className?: string;
}) {
	const { w, h } = layout.canvas;
	const objets = layout.objects
		.filter((o) => dessinable(o, w, h))
		.slice()
		.sort((a, b) => (a.drawPriority ?? 0) - (b.drawPriority ?? 0));

	return (
		<div className={`
    pointer-events-none absolute inset-0
    ${className ?? ""}
  `} aria-hidden>
			{objets.map((o, i) => {
				const t = o.transform!;
				const sp = o.sprite!;
				// Le sprite est posé à l'échelle du layout puis ramené en pourcentage du cadre ;
				// l'ancre du jeu (0,5 par défaut) devient une translation CSS.
				const largeur = (sp.w * (t.scaleX ?? 1) * 100) / w;
				const hauteur = (sp.h * (t.scaleY ?? 1) * 100) / h;
				const gauche = (t.x! * 100) / w;
				const haut = (t.y! * 100) / h;
				const ax = (t.anchorX ?? 0.5) * 100;
				const ay = (t.anchorY ?? 0.5) * 100;
				return (
					// eslint-disable-next-line @next/next/no-img-element
					<img
						key={`${o.name}-${i}`}
						src={`${cdn}${sp.pngUrl}`}
						alt=""
						style={{
							position: "absolute",
							left: `${gauche}%`,
							top: `${haut}%`,
							width: `${largeur}%`,
							height: `${hauteur}%`,
							transform: `translate(-${ax}%, -${ay}%)${t.rot ? ` rotate(${t.rot}rad)` : ""}`,
						}}
					/>
				);
			})}
		</div>
	);
}
