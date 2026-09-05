/**
 * **Fallback caméra de cut-in** — caméra scénarisée par plan, pilotée par le *timing réel* du
 * `.g4cm` (cf. `lib/cutin/g4cm.ts`). Comme les *valeurs* de keyframes du g4cm sont opaques, on ne
 * rejoue pas les positions brutes : on reconstruit une suite de **plans** (orbit / dolly / push-in)
 * coupés exactement aux frames du fichier, ce qui restitue le *rythme* du cut-in d'origine.
 *
 * Sans g4cm (ou parse échoué), `defaultCutinShots()` fournit un storyboard générique de ~7,7 s.
 *
 * Module pur / client-safe : sortie consommable par un mixer R3F (`useFrame`) ou une timeline
 * Theatre.js. Aucune dépendance three ici (juste des tuples) pour rester testable.
 */
import type { G4cmParsed } from "./g4cm";

/** Type de mouvement d'un plan. */
export type CutinShotMove = "orbit" | "dolly-in" | "dolly-out" | "static" | "crane" | "whip-pan";

/** Un plan de caméra prêt à jouer (temps normalisés + paramètres de cadrage). */
export interface CutinShot {
	/** Nom du plan (repris du g4cm, ex. `c0010`). */
	name: string;
	/** Début du plan en secondes (depuis t=0 du cut-in). */
	tStart: number;
	/** Fin du plan en secondes. */
	tEnd: number;
	/** Mouvement de caméra. */
	move: CutinShotMove;
	/** Rayon de départ → fin (distance au centre du modèle, en unités scène). */
	radius: [number, number];
	/** Azimut de départ → fin (radians, autour de Y). */
	azimuth: [number, number];
	/** Élévation de départ → fin (radians, depuis le plan horizontal). */
	elevation: [number, number];
	/** Hauteur visée (Y du point regardé), départ → fin. */
	targetY: [number, number];
	/** FOV (degrés), départ → fin. */
	fov: [number, number];
}

/** Storyboard complet d'un cut-in. */
export interface CutinStoryboard {
	shots: CutinShot[];
	/** Durée totale (s). */
	duration: number;
	/** Cadence cible pour l'export vidéo. */
	fps: number;
	/** `true` si le timing vient d'un vrai `.g4cm`, `false` si générique. */
	fromG4cm: boolean;
}

const TAU = Math.PI * 2;

/** Cadrages-types cyclés sur les plans, pour varier l'intérêt visuel d'un cut-in. */
const SHOT_PRESETS: Omit<CutinShot, "name" | "tStart" | "tEnd">[] = [
	// Plan large d'établissement, légère orbite.
	{
		move: "orbit",
		radius: [4.2, 3.8],
		azimuth: [-0.5, 0.4],
		elevation: [0.18, 0.12],
		targetY: [1.0, 1.05],
		fov: [38, 36],
	},
	// Resserrage dramatique (push-in) sur le buste.
	{
		move: "dolly-in",
		radius: [3.6, 2.2],
		azimuth: [0.3, 0.1],
		elevation: [0.1, 0.05],
		targetY: [1.15, 1.25],
		fov: [40, 30],
	},
	// Plan bas en contre-plongée (crane up), héroïque.
	{
		move: "crane",
		radius: [2.8, 2.6],
		azimuth: [-0.2, -0.6],
		elevation: [-0.05, 0.25],
		targetY: [0.9, 1.3],
		fov: [34, 32],
	},
	// Whip-pan rapide (transition d'impact).
	{
		move: "whip-pan",
		radius: [2.4, 2.4],
		azimuth: [-0.8, 0.8],
		elevation: [0.1, 0.1],
		targetY: [1.2, 1.2],
		fov: [45, 45],
	},
	// Recul final (dolly-out) qui rouvre sur le large.
	{
		move: "dolly-out",
		radius: [2.6, 4.6],
		azimuth: [0.2, 0.9],
		elevation: [0.12, 0.2],
		targetY: [1.2, 1.05],
		fov: [32, 40],
	},
];

/**
 * Construit le storyboard depuis un `.g4cm` parsé : un plan par segment du fichier, coupé aux
 * frames réelles. Le *type* de mouvement est attribué par preset cyclique (les valeurs exactes
 * du g4cm étant opaques), mais les **durées et coupes** sont fidèles à l'original.
 */
export function buildCutinShots(parsed: G4cmParsed): CutinStoryboard {
	const fps = parsed.assumedFps || 60;
	if (!parsed.shots.length) return defaultCutinShots();

	const t0 = Math.min(...parsed.shots.map((s) => s.startFrame));
	const shots: CutinShot[] = parsed.shots.map((seg, i) => {
		const preset = SHOT_PRESETS[i % SHOT_PRESETS.length];
		return {
			...preset,
			name: seg.name,
			tStart: (seg.startFrame - t0) / fps,
			tEnd: (seg.endFrame - t0 + 1) / fps,
		};
	});

	return {
		shots,
		duration: shots[shots.length - 1].tEnd,
		fps,
		fromG4cm: true,
	};
}

/** Storyboard générique (pas de g4cm) : ~7,7 s, 5 plans façon cut-in IEVR. */
export function defaultCutinShots(duration = 7.7, fps = 60): CutinStoryboard {
	const segs = [0.26, 0.3, 0.14, 0.1, 0.2]; // proportions ~ ev60_00340
	let acc = 0;
	const shots: CutinShot[] = segs.map((frac, i) => {
		const tStart = acc * duration;
		acc += frac;
		return {
			...SHOT_PRESETS[i % SHOT_PRESETS.length],
			name: `c00${(i + 1) * 10}`,
			tStart,
			tEnd: acc * duration,
		};
	});
	return { shots, duration, fps, fromG4cm: false };
}

/** État caméra interpolé à l'instant `t` (s) : position monde + cible + FOV. */
export interface CutinCameraState {
	position: [number, number, number];
	target: [number, number, number];
	fov: number;
	/** Plan actif (debug / overlay). */
	shotName: string;
}

/** Lissage cubique (easeInOut) — adoucit l'intérieur de chaque plan. */
function ease(x: number): number {
	return x < 0.5 ? 4 * x * x * x : 1 - (-2 * x + 2) ** 3 / 2;
}

const lerp = (a: number, b: number, t: number) => a + (b - a) * t;

/**
 * Échantillonne la caméra au temps `t` (s). Centre supposé en `[0, centerY, 0]` (le GLB waza est
 * recentré au chargement). Retourne position/cible/FOV — à appliquer sur une `PerspectiveCamera`.
 */
export function sampleCutinCamera(
	story: CutinStoryboard,
	t: number,
	centerY = 1.0
): CutinCameraState {
	const shot = story.shots.find((s) => t >= s.tStart && t < s.tEnd) ?? story.shots[story.shots.length - 1];
	const span = Math.max(shot.tEnd - shot.tStart, 1e-3);
	const local = ease(Math.min(Math.max((t - shot.tStart) / span, 0), 1));

	// whip-pan : easing plus brutal (impact) sur l'azimut.
	const azT = shot.move === "whip-pan" ? local ** 0.4 : local;

	const r = lerp(shot.radius[0], shot.radius[1], local);
	const az = lerp(shot.azimuth[0], shot.azimuth[1], azT);
	const el = lerp(shot.elevation[0], shot.elevation[1], local);
	const ty = lerp(shot.targetY[0], shot.targetY[1], local);
	const fov = lerp(shot.fov[0], shot.fov[1], local);

	// Orbite continue pour les plans "orbit" (l'azimut tourne en plus de la base).
	const orbit = shot.move === "orbit" ? local * (TAU * 0.18) : 0;
	const a = az + orbit;

	const cy = ty || centerY;
	const px = Math.sin(a) * Math.cos(el) * r;
	const pz = Math.cos(a) * Math.cos(el) * r;
	const py = cy + Math.sin(el) * r;

	return {
		position: [px, py, pz],
		target: [0, cy, 0],
		fov,
		shotName: shot.name,
	};
}
