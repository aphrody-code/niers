/**
 * Visualiseur de caméra de cinématique (`.g4cm`) — 1 215 fichiers dans le jeu.
 *
 * Une caméra du jeu est une collection de canaux échantillonnés : sa position (`PosX/Y/Z`),
 * son point visé (`RefX/Y/Z`) et son champ de vision (`Fov`). Cette vue les trace côte à côte
 * sur une échelle de frames commune, ce qui rend une trajectoire lisible d'un coup d'œil —
 * là où le JSON brut du décodeur demande de reconstituer mentalement des milliers de nombres.
 *
 * Les canaux non résolus sont affichés comme tels, jamais tracés : l'encodage 2 octets de
 * certains flux n'est pas élucidé, et une courbe inventée aurait l'air d'une vraie.
 */

import { useEffect, useMemo, useState } from "react";
import { commands } from "@/lib/bindings";
import type { ApercuCameraDto, PisteCameraDto } from "@/lib/bindings";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Badge } from "@/components/ui/badge";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";

/** Couleur par famille de canal — position, visée, champ de vision. */
const COULEURS: Record<string, string> = {
	PosX: "#f87171",
	PosY: "#4ade80",
	PosZ: "#60a5fa",
	RefX: "#fb923c",
	RefY: "#a3e635",
	RefZ: "#38bdf8",
	Fov: "#c084fc",
};

/** Couleur d'un canal, avec un gris neutre pour les canaux non nommés. */
function couleur(canal: string): string {
	return COULEURS[canal] ?? "#94a3b8";
}

/** Une courbe SVG pour une piste, normalisée sur la plage de frames et sa propre amplitude. */
function Courbe({
	piste,
	frameMin,
	frameMax,
	largeur,
	hauteur,
}: {
	piste: PisteCameraDto;
	frameMin: number;
	frameMax: number;
	largeur: number;
	hauteur: number;
}) {
	const chemin = useMemo(() => {
		// Specta traduit tout `f32` en `number | null` : un flottant non fini n'est pas
		// représentable en JSON. On écarte donc les échantillons incomplets au lieu de les
		// remplacer par 0, qui creuserait un pic vers l'origine au milieu d'une trajectoire.
		const points: Array<[number, number]> = [];
		const n = Math.min(piste.temps.length, piste.valeurs.length);
		for (let i = 0; i < n; i++) {
			const t = piste.temps[i];
			const v = piste.valeurs[i];
			if (t == null || v == null) continue;
			points.push([t, v]);
		}
		if (points.length < 2) return "";

		let min = Infinity;
		let max = -Infinity;
		for (const [, v] of points) {
			if (v < min) min = v;
			if (v > max) max = v;
		}
		// Une piste constante n'a pas d'amplitude : la tracer au milieu plutôt que de diviser
		// par zéro, ce qui produirait un NaN et une courbe invisible sans dire pourquoi.
		const amplitude = max - min || 1;
		const plage = frameMax - frameMin || 1;

		const segments: string[] = [];
		points.forEach(([t, v], i) => {
			const x = ((t - frameMin) / plage) * largeur;
			const y = hauteur - ((v - min) / amplitude) * hauteur;
			segments.push(`${i === 0 ? "M" : "L"}${x.toFixed(1)},${y.toFixed(1)}`);
		});
		return segments.join(" ");
	}, [piste, frameMin, frameMax, largeur, hauteur]);

	if (!chemin) return null;
	return (
		<path d={chemin} fill="none" stroke={couleur(piste.canal)} strokeWidth={1.4} />
	);
}

/** Une ligne du tableau : le canal, son objet, son amplitude et sa courbe. */
function LignePiste({
	piste,
	frameMin,
	frameMax,
}: {
	piste: PisteCameraDto;
	frameMin: number;
	frameMax: number;
}) {
	const bornes = useMemo(() => {
		let min = Infinity;
		let max = -Infinity;
		for (const v of piste.valeurs) {
			if (v == null) continue;
			if (v < min) min = v;
			if (v > max) max = v;
		}
		return min <= max ? { min, max } : null;
	}, [piste.valeurs]);

	return (
		<div className="flex items-center gap-3 border-b border-border/40 py-1.5">
			<div className="w-40 shrink-0 truncate text-xs text-muted-foreground" title={piste.objet}>
				{piste.objet}
			</div>
			<div className="w-16 shrink-0">
				<span
					className="rounded px-1.5 py-0.5 text-[11px] font-medium"
					style={{ color: couleur(piste.canal), background: `${couleur(piste.canal)}1a` }}
				>
					{piste.canal}
				</span>
			</div>
			<div className="w-44 shrink-0 text-right font-mono text-[11px] text-muted-foreground">
				{piste.resolu && bornes
					? `${bornes.min.toFixed(2)} → ${bornes.max.toFixed(2)}`
					: "flux non résolu"}
			</div>
			<div className="min-w-0 flex-1">
				{piste.resolu ? (
					<svg width="100%" height={28} viewBox="0 0 320 28" preserveAspectRatio="none">
						<Courbe
							piste={piste}
							frameMin={frameMin}
							frameMax={frameMax}
							largeur={320}
							hauteur={28}
						/>
					</svg>
				) : (
					<div className="h-7 rounded bg-muted/30" />
				)}
			</div>
			<div className="w-14 shrink-0 text-right font-mono text-[11px] text-muted-foreground">
				{piste.temps.length}
			</div>
		</div>
	);
}

/** Le visualiseur complet, pour un chemin VFS de `.g4cm`. */
export function CameraTrackView({ path, gameDir }: { path: string; gameDir?: string | null }) {
	const [apercu, setApercu] = useState<ApercuCameraDto | null>(null);
	const [erreur, setErreur] = useState<string | null>(null);
	const [filtre, setFiltre] = useState<string>("");

	useEffect(() => {
		let annule = false;
		setApercu(null);
		setErreur(null);
		commands
			.vfsApercuCamera(path, gameDir ?? null)
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

	const pistes = useMemo(() => {
		if (!apercu) return [];
		if (!filtre) return apercu.pistes;
		return apercu.pistes.filter((p) => p.canal === filtre);
	}, [apercu, filtre]);

	// Même traitement des états d'attente que le reste de l'application (`Alert`) : ces deux
	// lignes étaient du texte nu, sans cadre ni titre.
	if (erreur) {
		return (
			<div className="p-3">
				<Alert variant="destructive">
					<AlertTitle>Caméra illisible</AlertTitle>
					<AlertDescription>{erreur}</AlertDescription>
				</Alert>
			</div>
		);
	}
	if (!apercu) {
		return (
			<div className="p-3">
				<Alert>
					<AlertTitle>Décodage de la caméra…</AlertTitle>
					<AlertDescription>Lecture des canaux, objets et clips d'animation.</AlertDescription>
				</Alert>
			</div>
		);
	}

	const canaux = [...new Set(apercu.pistes.map((p) => p.canal))].sort();

	return (
		<div className="flex h-full flex-col gap-2 p-3">
			<div className="flex flex-wrap items-center gap-2">
				<Badge variant="secondary">
					{apercu.canaux} canaux — {apercu.canaux_resolus} résolus
				</Badge>
				<Badge variant="secondary">{apercu.objets.length} objets</Badge>
				<Badge variant="secondary">{apercu.clips.length} clips</Badge>
				<Badge variant="outline">
					frames {(apercu.frame_min ?? 0).toFixed(0)} → {(apercu.frame_max ?? 0).toFixed(0)}
				</Badge>
			</div>

			{/* Ce que la vue NE montre PAS mérite un cadre, pas une note en petits caractères
			    au bout d'une rangée de badges : un canal non tracé peut se lire comme un canal
			    vide. */}
			{apercu.canaux_resolus < apercu.canaux && (
				<Alert>
					<AlertTitle>
						{apercu.canaux - apercu.canaux_resolus} canaux non tracés sur {apercu.canaux}
					</AlertTitle>
					<AlertDescription>
						Leur encodage n'est pas élucidé — ils existent dans le fichier, mais rien n'en est
						dessiné ci-dessous.
					</AlertDescription>
				</Alert>
			)}

			{/* Filtre par canal — `ToggleGroup`, le même sélecteur exclusif que `CfgbinViewer`.
			    C'étaient des `<button>` avec un état actif peint à la main (`bg-accent`), sans
			    sémantique de pression pour l'accessibilité. */}
			<ToggleGroup
				value={[filtre]}
				onValueChange={(v: string[]) => setFiltre(v[0] ?? "")}
				className="flex-wrap"
			>
				<ToggleGroupItem value="">tous</ToggleGroupItem>
				{canaux.map((c) => (
					<ToggleGroupItem key={c} value={c} style={{ color: couleur(c) }}>
						{c}
					</ToggleGroupItem>
				))}
			</ToggleGroup>

			<ScrollArea className="min-h-0 flex-1 rounded border border-border/50">
				<div className="px-2">
					{pistes.map((p, i) => (
						<LignePiste
							key={`${p.objet}-${p.canal}-${i}`}
							piste={p}
							frameMin={apercu.frame_min ?? 0}
							frameMax={apercu.frame_max ?? 0}
						/>
					))}
					{pistes.length === 0 && (
						<div className="p-4 text-sm text-muted-foreground">
							{filtre
								? `Aucune piste sur le canal « ${filtre} ».`
								: "Aucune piste tracée — aucun canal de ce fichier n'est résolu."}
						</div>
					)}
				</div>
			</ScrollArea>
		</div>
	);
}

export default CameraTrackView;
