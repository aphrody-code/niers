/**
 * Visualiseur de maillage de navigation (`.g4nv`) — 160 fichiers, sur 153 cartes du jeu.
 *
 * Un navmesh décrit où les personnages peuvent marcher : des triangles en coordonnées monde,
 * reliés par un graphe d'arêtes portant un coût de franchissement. Cette vue le projette en
 * plan (X horizontal, Z vertical — la vue de dessus, celle qui correspond à une carte), parce
 * que c'est la seule projection où la topologie d'une zone se lit immédiatement.
 *
 * Les arêtes de **bord** (celles qui ne relient qu'un seul polygone) sont tracées à part : ce
 * sont les murs invisibles de la zone marchable, l'information la plus utile pour comprendre
 * pourquoi un personnage bute quelque part.
 */

import { useEffect, useMemo, useRef, useState } from "react";
import { commands } from "@/lib/bindings";
import type { ApercuNavmDto } from "@/lib/bindings";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";

/** Marge intérieure du tracé, en pixels — évite que le maillage touche les bords. */
const MARGE = 12;

type Vue = { largeur: number; hauteur: number };

/**
 * Le sommet d'index `i`, ou `null` si l'une de ses coordonnées manque.
 *
 * Specta traduit `f32` en `number | null` (un flottant non fini n'est pas représentable en
 * JSON). On ne peut pas filtrer la table des sommets pour autant : les triangles et les arêtes
 * y renvoient par **index**, et retirer une entrée décalerait toute la géométrie. On teste donc
 * à l'usage, et on saute la primitive concernée.
 */
function sommet(apercu: ApercuNavmDto, i: number): [number, number, number] | null {
	const p = apercu.sommets[i];
	if (!p) return null;
	const [x, y, z] = p;
	if (x == null || y == null || z == null) return null;
	return [x, y, z];
}

/** Projette les coordonnées monde en coordonnées écran, vue de dessus (X, Z). */
function projecteur(apercu: ApercuNavmDto, vue: Vue) {
	const minX = apercu.bbox_min[0] ?? 0;
	const minZ = apercu.bbox_min[2] ?? 0;
	const maxX = apercu.bbox_max[0] ?? 0;
	const maxZ = apercu.bbox_max[2] ?? 0;
	// Une carte plate sur un axe (ou un maillage vide) ne doit pas produire une division par
	// zéro : l'échelle retombe alors à 1, le maillage se dessine au centre.
	const etendueX = maxX - minX || 1;
	const etendueZ = maxZ - minZ || 1;
	const echelle = Math.min(
		(vue.largeur - 2 * MARGE) / etendueX,
		(vue.hauteur - 2 * MARGE) / etendueZ,
	);
	const decalageX = (vue.largeur - etendueX * echelle) / 2;
	const decalageZ = (vue.hauteur - etendueZ * echelle) / 2;

	return (p: [number, number, number]): [number, number] => [
		(p[0] - minX) * echelle + decalageX,
		// L'axe Z du monde monte, l'axe Y de l'écran descend : sans inversion, la carte
		// s'affiche en miroir vertical par rapport à ce que montre le jeu.
		vue.hauteur - ((p[2] - minZ) * echelle + decalageZ),
	];
}

/** Le rendu canvas du maillage. */
function Plan({ apercu, montrerTriangles }: { apercu: ApercuNavmDto; montrerTriangles: boolean }) {
	const canvasRef = useRef<HTMLCanvasElement | null>(null);
	const conteneurRef = useRef<HTMLDivElement | null>(null);
	const [vue, setVue] = useState<Vue>({ largeur: 640, hauteur: 420 });

	useEffect(() => {
		const el = conteneurRef.current;
		if (!el) return;
		const obs = new ResizeObserver(() => {
			setVue({ largeur: el.clientWidth, hauteur: el.clientHeight });
		});
		obs.observe(el);
		setVue({ largeur: el.clientWidth, hauteur: el.clientHeight });
		return () => obs.disconnect();
	}, []);

	useEffect(() => {
		const canvas = canvasRef.current;
		if (!canvas || vue.largeur <= 0 || vue.hauteur <= 0) return;
		const ctx = canvas.getContext("2d");
		if (!ctx) return;

		// Rendu net sur écran haute densité : sans cela le tracé est flou et les arêtes fines
		// disparaissent.
		const dpr = window.devicePixelRatio || 1;
		canvas.width = vue.largeur * dpr;
		canvas.height = vue.hauteur * dpr;
		canvas.style.width = `${vue.largeur}px`;
		canvas.style.height = `${vue.hauteur}px`;
		ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
		ctx.clearRect(0, 0, vue.largeur, vue.hauteur);

		const projette = projecteur(apercu, vue);

		if (montrerTriangles) {
			ctx.fillStyle = "rgba(96, 165, 250, 0.10)";
			ctx.strokeStyle = "rgba(96, 165, 250, 0.35)";
			ctx.lineWidth = 0.5;
			for (const [a, b, c] of apercu.triangles) {
				const pa = sommet(apercu, a);
				const pb = sommet(apercu, b);
				const pc = sommet(apercu, c);
				if (!pa || !pb || !pc) continue;
				const [xa, ya] = projette(pa);
				const [xb, yb] = projette(pb);
				const [xc, yc] = projette(pc);
				ctx.beginPath();
				ctx.moveTo(xa, ya);
				ctx.lineTo(xb, yb);
				ctx.lineTo(xc, yc);
				ctx.closePath();
				ctx.fill();
				ctx.stroke();
			}
		}

		// Les bords passent en dernier et en plus épais : c'est le contour de la zone
		// marchable, l'information qu'on vient chercher.
		for (const passe of [false, true]) {
			ctx.strokeStyle = passe ? "rgba(248, 113, 113, 0.85)" : "rgba(148, 163, 184, 0.30)";
			ctx.lineWidth = passe ? 1.4 : 0.5;
			ctx.beginPath();
			for (const arete of apercu.aretes) {
				if (arete.bord !== passe) continue;
				const pa = sommet(apercu, arete.a);
				const pb = sommet(apercu, arete.b);
				if (!pa || !pb) continue;
				const [xa, ya] = projette(pa);
				const [xb, yb] = projette(pb);
				ctx.moveTo(xa, ya);
				ctx.lineTo(xb, yb);
			}
			ctx.stroke();
		}
	}, [apercu, vue, montrerTriangles]);

	return (
		<div ref={conteneurRef} className="min-h-0 flex-1 rounded border border-border/50 bg-background">
			<canvas ref={canvasRef} />
		</div>
	);
}

/** Le visualiseur complet, pour un chemin VFS de `.g4nv`. */
export function NavmeshView({ path, gameDir }: { path: string; gameDir?: string | null }) {
	const [apercu, setApercu] = useState<ApercuNavmDto | null>(null);
	const [erreur, setErreur] = useState<string | null>(null);
	const [montrerTriangles, setMontrerTriangles] = useState(true);

	useEffect(() => {
		let annule = false;
		setApercu(null);
		setErreur(null);
		commands
			.vfsApercuNavmesh(path, gameDir ?? null)
			.then((r) => {
				if (annule) return;
				if (r.status === "ok") setApercu(r.data);
				else setErreur(String(r.error));
			})
			.catch((e) => {
				if (!annule) setErreur(String(e));
			});
		return () => {
			annule = true;
		};
	}, [path, gameDir]);

	const dimensions = useMemo(() => {
		if (!apercu) return null;
		return {
			x: (apercu.bbox_max[0] ?? 0) - (apercu.bbox_min[0] ?? 0),
			y: (apercu.bbox_max[1] ?? 0) - (apercu.bbox_min[1] ?? 0),
			z: (apercu.bbox_max[2] ?? 0) - (apercu.bbox_min[2] ?? 0),
		};
	}, [apercu]);

	const bords = useMemo(
		() => (apercu ? apercu.aretes.filter((a) => a.bord).length : 0),
		[apercu],
	);

	// Les deux états d'attente passent par `Alert`, comme les quatorze autres vues de l'appli :
	// une erreur de décodage s'affichait ici en simple texte rouge, sans titre ni cadre, donc
	// sans rien qui la distingue d'une légende.
	if (erreur) {
		return (
			<div className="p-3">
				<Alert variant="destructive">
					<AlertTitle>Navmesh illisible</AlertTitle>
					<AlertDescription>{erreur}</AlertDescription>
				</Alert>
			</div>
		);
	}
	if (!apercu) {
		return (
			<div className="p-3">
				<Alert>
					<AlertTitle>Décodage du navmesh…</AlertTitle>
					<AlertDescription>Lecture des sommets, polygones et arêtes du fichier.</AlertDescription>
				</Alert>
			</div>
		);
	}

	return (
		<div className="flex h-full flex-col gap-2 p-3">
			<div className="flex flex-wrap items-center gap-2">
				<Badge variant="secondary">{apercu.sommets.length} sommets</Badge>
				<Badge variant="secondary">{apercu.polygones} polygones</Badge>
				<Badge variant="secondary">{apercu.aretes.length} arêtes</Badge>
				<Badge variant="outline" className="text-red-400">
					{bords} de bord
				</Badge>
				{dimensions && (
					<Badge variant="outline">
						{dimensions.x.toFixed(1)} × {dimensions.z.toFixed(1)} m (h {dimensions.y.toFixed(1)})
					</Badge>
				)}
				<Button
					variant="outline"
					size="sm"
					className="h-6 px-2 text-[11px]"
					onClick={() => setMontrerTriangles((v) => !v)}
				>
					{montrerTriangles ? "masquer" : "afficher"} les triangles
				</Button>
			</div>

			{/* Un maillage plafonné n'est pas une coquetterie d'affichage : ce qui est à l'écran
			    n'est PAS le fichier. Un `<span>` ambre au milieu d'une rangée de badges se lisait
			    comme une étiquette de plus. */}
			{apercu.tronque && (
				<Alert>
					<AlertTitle>Aperçu plafonné</AlertTitle>
					<AlertDescription>
						Le maillage affiché est partiel — les compteurs ci-dessus décrivent le fichier, le
						tracé n'en montre qu'une part.
					</AlertDescription>
				</Alert>
			)}

			<Plan apercu={apercu} montrerTriangles={montrerTriangles} />

			<div className="text-[11px] text-muted-foreground">
				Vue de dessus (X horizontal, Z vertical). En rouge, les arêtes de bord : le contour de
				la zone marchable.
			</div>
		</div>
	);
}

export default NavmeshView;
