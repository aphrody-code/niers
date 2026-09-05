/**
 * Pont **rendu déterministe** entre une séquence Theatre.js (`@theatre/core`) et le renderer
 * three.js d'une scène R3F (`@react-three/fiber`), pour l'export vidéo frame-par-frame.
 *
 * Principe : pendant l'export, R3F NE doit PAS rendre via `requestAnimationFrame` (sinon le timing
 * dépend de la machine). On met le canvas en `frameloop="never"`, et pour CHAQUE frame on :
 *   1. fige la position de la timeline : `sequence.position = timeSec` (Theatre.js applique alors
 *      synchroniquement toutes les valeurs interpolées sur les objets pilotés `useSheetObject`) ;
 *   2. avance l'horloge R3F + force un rendu unique : `gl.render(scene, camera)`.
 *
 * Cela donne un `renderAt(timeSec)` injectable dans {@link exportCutinVideo} qui produit un rendu
 * bit-stable quel que soit le débit machine.
 *
 * Client-safe (les `@theatre/*` et `three` sont des deps client). À n'utiliser que dans un îlot
 * `"use client"` dont le `<Canvas>` est chargé via `next/dynamic { ssr:false }`.
 */

import { type ISequence, val } from "@theatre/core";
import type { RootState } from "@react-three/fiber";

/** Sous-ensemble du `RootState` R3F nécessaire au rendu manuel (renderer + scène + caméra + clock). */
export interface R3fRenderHandle {
	gl: RootState["gl"];
	scene: RootState["scene"];
	camera: RootState["camera"];
	/** Optionnel : horloge R3F, avancée manuellement pour que les `useFrame` éventuels soient cohérents. */
	advance?: RootState["advance"];
}

/** Options du séquenceur déterministe. */
export interface TheatreSequencerOptions {
	/** La séquence Theatre.js du sheet du cut-in (`sheet.sequence`). */
	sequence: ISequence;
	/** Handle de rendu R3F (récupéré via `useThree(s => ({ gl, scene, camera, advance }))`). */
	r3f: R3fRenderHandle;
	/**
	 * Callback synchrone optionnel exécuté APRÈS le positionnement de la timeline et AVANT le rendu
	 * (ex. mutations dépendantes du temps non pilotées par Theatre : shake de caméra, uniforms).
	 */
	onBeforeRender?: (timeSec: number) => void;
}

/**
 * Construit un `renderAt(timeSec)` déterministe à passer à `exportCutinVideo({ renderAt })`.
 *
 * Le canvas R3F doit être en `frameloop="never"` pendant l'export (à rétablir ensuite). La durée à
 * exporter = `sequence.length` (en secondes).
 */
export function createTheatreSequencer(options: TheatreSequencerOptions): (timeSec: number) => void {
	const { sequence, r3f, onBeforeRender } = options;
	const { gl, scene, camera, advance } = r3f;

	return (timeSec: number) => {
		// 1) Fige la timeline → Theatre applique les valeurs interpolées sur les objets pilotés.
		sequence.position = timeSec;
		// 2) Avance l'horloge R3F au temps absolu (les `useFrame` voient un delta cohérent).
		advance?.(timeSec, true);
		// 3) Mutations hors-Theatre éventuelles.
		onBeforeRender?.(timeSec);
		// 4) Rendu unique et synchrone dans le canvas WebGL (source des VideoFrame).
		gl.render(scene, camera);
	};
}

/**
 * Durée d'export recommandée (s) = longueur de la séquence Theatre.js.
 * Lue via `val(sequence.pointer.length)` (la longueur n'est pas exposée en propriété directe sur
 * `ISequence` en `@theatre/core` 0.7 ; elle vit dans le Pointer d'état, en secondes).
 */
export function sequenceDurationSec(sequence: ISequence): number {
	return val(sequence.pointer.length);
}
