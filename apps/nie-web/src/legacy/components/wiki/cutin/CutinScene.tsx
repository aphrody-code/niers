// oxlint-disable react/no-unknown-property -- react-three-fiber : `position`, `args`,
// `intensity`, `rotation`… sont des propriétés de la scène 3D, pas du DOM. La règle les
// juge inconnues parce qu'elle ne connaît que HTML.
/**
 * Scène R3F du viewer de cut-in (cible d'un `next/dynamic { ssr:false }`).
 *
 * PAS de `"use client"` ici : ce module n'est chargé QUE côté client (via le wrapper
 * `CutinViewer`), donc déjà hors SSR. Il compose :
 *  - le mesh waza (+ d'éventuelles couches d'effet g4pk décodées en GLB) ;
 *  - une caméra **scénarisée** pilotée par le storyboard (rythme du `.g4cm`, valeurs
 *    de trajectoire opaques → presets, cf. `lib/cutin/camera.ts`) ;
 *  - une timeline maître `@theatre/core` (la `sequence` sert de playhead pour le scrub
 *    ET l'export vidéo déterministe) ;
 *  - un Bloom optionnel (`@react-three/postprocessing`).
 *
 * Note CSP : on n'utilise PAS `<Environment preset>` de drei (il fetch un HDR externe →
 * violerait `connect-src 'self'`, inchangé). L'éclairage est 100 % local (hemisphere +
 * directionnelles). `@theatre/studio` reste DEV-only (import conditionnel).
 */
import { Bloom, EffectComposer } from "@react-three/postprocessing";
import { OrbitControls, useGLTF } from "@react-three/drei";
import { Canvas, useFrame, useThree } from "@react-three/fiber";
import type { RootState } from "@react-three/fiber";
import { getProject } from "@theatre/core";
import type { ISequence } from "@theatre/core";
import { SheetProvider } from "@theatre/r3f";
import { Suspense, useEffect, useMemo, useRef } from "react";
import * as THREE from "three";
import { sampleCutinCamera, type CutinStoryboard } from "@/lib/cutin/camera";
import type { CutinGlbLayer, CutinManifest } from "@/lib/cutin/types";

// Valeur de l'enum `KernelSize.LARGE` de `postprocessing` (= 3). On ne l'importe pas
// depuis "postprocessing" : ce paquet (dép. transitive de @react-three/postprocessing)
// n'est pas résolvable au niveau racine sous le linker Bun, et la prop `kernelSize` de
// `<Bloom>` est typée `any`. Cf. enum postprocessing : VERY_SMALL=0…HUGE=5, LARGE=3.
const KERNEL_SIZE_LARGE = 3;

/** Handle remonté au wrapper (export vidéo + pilotage du frameloop). */
export interface CutinSceneHandle {
	gl: RootState["gl"];
	scene: RootState["scene"];
	camera: RootState["camera"];
	/** Avance manuellement le frameloop (rend les `useFrame`, même en `frameloop="never"`). */
	advance: RootState["advance"];
	/** Timeline maître (scrub + export). */
	sequence: ISequence;
	setFrameloop: (mode: "always" | "never") => void;
}

export interface CutinSceneProps {
	manifest: CutinManifest;
	storyboard: CutinStoryboard;
	/** Couches déjà décodées (waza + effets ; peut n'être que `[waza]`). */
	glbLayers: CutinGlbLayer[];
	/** `id` → visible (piloté par le wrapper). */
	visibility: Record<string, boolean>;
	/** Bloom actif (défaut true). */
	bloom?: boolean;
	/** OrbitControls actif (exclusif du rig scénarisé). */
	orbit?: boolean;
	/** Lecture en cours (avance le playhead). Ignoré si `scrubSec` non-null. */
	playing?: boolean;
	/** Si non-null, fige `sequence.position = scrubSec`. */
	scrubSec: number | null;
	/** Notifié quand le plan caméra courant change (overlay). */
	onShot?: (shotName: string) => void;
	/** Remonte le handle de rendu une fois la scène montée. */
	onReady?: (handle: CutinSceneHandle) => void;
}

/** Recentre + normalise un GLB cloné (bbox → poser au sol, mise à l'échelle ~2 u.). */
function fitObject(object: THREE.Object3D, emissiveBoost: boolean): void {
	const box = new THREE.Box3().setFromObject(object);
	const size = new THREE.Vector3();
	const center = new THREE.Vector3();
	box.getSize(size);
	box.getCenter(center);
	const maxDim = Math.max(size.x, size.y, size.z) || 1;
	const scale = 2 / maxDim;
	object.scale.setScalar(scale);
	// Recentre en X/Z, pose la base sur le sol (y=0).
	object.position.set(-center.x * scale, -box.min.y * scale, -center.z * scale);

	if (emissiveBoost) {
		// Couches d'effet : émissif boosté + hors tonemapping pour « briller » dans le Bloom.
		object.traverse((child) => {
			const mesh = child as THREE.Mesh;
			if (!mesh.isMesh) return;
			const mats = Array.isArray(mesh.material) ? mesh.material : [mesh.material];
			for (const m of mats) {
				const mat = m as THREE.MeshStandardMaterial;
				if ("emissiveIntensity" in mat) mat.emissiveIntensity = 2.5;
				mat.toneMapped = false;
			}
		});
	}
}

/** Un modèle GLB monté (waza ou effet), recentré et mis à l'échelle. */
function LayerModel({ layer }: { layer: CutinGlbLayer }): React.JSX.Element {
	const gltf = useGLTF(layer.blobUrl);
	// Clone (un GLB peut être monté plusieurs fois) + fit, mémoïsé sur la source.
	const object = useMemo(() => {
		const clone = gltf.scene.clone(true);
		fitObject(clone, layer.fromEffect);
		return clone;
	}, [gltf.scene, layer.fromEffect]);
	return <primitive object={object} />;
}

/** Pilote la caméra scénarisée + avance le playhead de la timeline maître. */
function CameraRig({
	storyboard,
	sequence,
	scrubSec,
	playing,
	enabled,
	onShot,
}: {
	storyboard: CutinStoryboard;
	sequence: ISequence;
	scrubSec: number | null;
	playing: boolean;
	enabled: boolean;
	onShot?: (name: string) => void;
}): null {
	const lastShot = useRef<string>("");
	useFrame((_state, delta) => {
		if (!enabled) return;
		let t: number;
		if (scrubSec != null) {
			sequence.position = scrubSec;
			t = scrubSec;
		} else {
			if (playing) {
				let p = sequence.position + delta;
				if (storyboard.duration > 0 && p > storyboard.duration) p %= storyboard.duration;
				sequence.position = p;
			}
			t = sequence.position;
		}
		const s = sampleCutinCamera(storyboard, t);
		const cam = _state.camera;
		cam.position.set(s.position[0], s.position[1], s.position[2]);
		cam.lookAt(s.target[0], s.target[1], s.target[2]);
		const pc = cam as THREE.PerspectiveCamera;
		if (pc.isPerspectiveCamera && pc.fov !== s.fov) {
			pc.fov = s.fov;
			pc.updateProjectionMatrix();
		}
		if (onShot && s.shotName !== lastShot.current) {
			lastShot.current = s.shotName;
			onShot(s.shotName);
		}
	});
	return null;
}

/** Remonte le handle de rendu (gl/scene/camera/advance/sequence) au wrapper. */
function ReadyReporter({
	sequence,
	onReady,
}: {
	sequence: ISequence;
	onReady?: (h: CutinSceneHandle) => void;
}): null {
	const gl = useThree((s) => s.gl);
	const scene = useThree((s) => s.scene);
	const camera = useThree((s) => s.camera);
	const advance = useThree((s) => s.advance);
	const setFrameloop = useThree((s) => s.setFrameloop);
	useEffect(() => {
		onReady?.({
			gl,
			scene,
			camera,
			advance,
			sequence,
			setFrameloop: (mode) => setFrameloop(mode),
		});
	}, [gl, scene, camera, advance, sequence, setFrameloop, onReady]);
	return null;
}

/** Contenu de la scène (intérieur du `<Canvas>`). */
function SceneContent(props: CutinSceneProps): React.JSX.Element {
	const { manifest, storyboard, glbLayers, visibility, bloom = true, orbit = false } = props;
	const sheet = useMemo(
		() => getProject("azalee-cutin").sheet(manifest.eventId),
		[manifest.eventId],
	);
	const sequence = sheet.sequence;

	// Réinitialise le playhead au montage / changement de durée. La longueur de timeline
	// effective pour l'export est `storyboard.duration` (la `length` de l'ISequence n'est pas
	// réglable directement en @theatre/core 0.7 ; on pilote `position` manuellement).
	useEffect(() => {
		sequence.position = 0;
	}, [sequence, storyboard.duration]);

	// `@theatre/studio` : DEV-only, jamais dans le bundle prod.
	useEffect(() => {
		if (process.env.NODE_ENV !== "development") return;
		let active = true;
		void import("@theatre/studio").then((mod) => {
			if (active) mod.default.initialize();
		});
		return () => {
			active = false;
		};
	}, []);

	const visibleLayers = glbLayers.filter((l) => visibility[l.id] !== false);

	return (
		<SheetProvider sheet={sheet}>
			<hemisphereLight args={[0xffffff, 0x33384a, 1.1]} />
			<ambientLight intensity={0.35} />
			<directionalLight position={[3, 5, 4]} intensity={1.6} />
			<directionalLight position={[-4, 2, -3]} intensity={0.8} color={0xbcd0ff} />

			<Suspense fallback={null}>
				{visibleLayers.map((layer) => (
					<LayerModel key={layer.id} layer={layer} />
				))}
			</Suspense>

			<CameraRig
				storyboard={storyboard}
				sequence={sequence}
				scrubSec={props.scrubSec}
				playing={props.playing ?? false}
				enabled={!orbit}
				onShot={props.onShot}
			/>
			{orbit && <OrbitControls makeDefault enableDamping />}

			{bloom && (
				<EffectComposer>
					<Bloom
						mipmapBlur
						luminanceThreshold={1}
						intensity={1.4}
						kernelSize={KERNEL_SIZE_LARGE}
					/>
				</EffectComposer>
			)}

			<ReadyReporter sequence={sequence} onReady={props.onReady} />
		</SheetProvider>
	);
}

/** Canvas R3F du cut-in. `preserveDrawingBuffer` IMPÉRATIF (capture VideoFrame non-noire). */
export default function CutinScene(props: CutinSceneProps): React.JSX.Element {
	return (
		<Canvas
			frameloop="always"
			gl={{ preserveDrawingBuffer: true }}
			camera={{ fov: 38, position: [0, 1, 4] }}
			dpr={[1, 2]}
		>
			<SceneContent {...props} />
		</Canvas>
	);
}
