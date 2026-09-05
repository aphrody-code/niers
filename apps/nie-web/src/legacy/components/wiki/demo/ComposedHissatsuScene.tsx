// oxlint-disable react/no-unknown-property -- react-three-fiber : `position`, `args`,
// `intensity`, `rotation`… sont des propriétés de la scène 3D, pas du DOM. La règle les
// juge inconnues parce qu'elle ne connaît que HTML.
/**
 * Scène R3F COMPOSÉE « joueur + super-technique » (cible d'un `next/dynamic { ssr:false }`).
 *
 * PAS de `"use client"` ici : ce module n'est chargé QUE côté client (via le wrapper
 * `DemoExperience`, qui l'importe en `next/dynamic { ssr:false }`), donc déjà hors SSR — même
 * convention que `CutinScene.tsx`.
 *
 * Différence FONDAMENTALE avec `CutinScene` : `CutinScene.fitObject` normalise CHAQUE GLB à ~2 u
 * INDIVIDUELLEMENT (recentré sur l'origine) → deux GLB se superposeraient au même point. Ici on
 * fait du **placement explicite par objet** :
 *  - le joueur (`model-full`) est normalisé par sa HAUTEUR, pieds calés exactement à y=0 (Box3) ;
 *  - l'effet waza est mis à l'échelle RELATIVE au joueur et centré autour de son torse.
 * → le résultat est le personnage entouré de son hissatsu, dans une seule scène.
 *
 * La dynamique vient de la caméra cinématique (storyboard g4cm), des animations du GLB d'effet et
 * du Bloom — le joueur reste en pose statique (anim squelettique HORS-scope, cf. spec §7).
 *
 * Note CSP : pas de `<Environment preset>` (fetch HDR externe → violerait `connect-src 'self'`).
 * Éclairage 100 % local. `@theatre/studio` reste DEV-only (import conditionnel).
 */
import { Bloom, EffectComposer } from "@react-three/postprocessing";
import { ContactShadows, OrbitControls, useGLTF, useTexture } from "@react-three/drei";
import { Canvas, useFrame, useThree } from "@react-three/fiber";
import { getProject } from "@theatre/core";
import type { ISequence } from "@theatre/core";
import { SheetProvider } from "@theatre/r3f";
import { Suspense, useEffect, useMemo, useRef } from "react";
import * as THREE from "three";
import { sampleCutinCamera, type CutinStoryboard } from "@/lib/cutin/camera";

// On réexpose EXACTEMENT le même handle que `CutinScene` → `DemoExperience.handleExport` et
// `createTheatreSequencer` fonctionnent sans aucune modification (cf. spec §0).
export type { CutinSceneHandle } from "@/components/wiki/cutin/CutinScene";
import type { CutinSceneHandle } from "@/components/wiki/cutin/CutinScene";

// Valeur de l'enum `KernelSize.LARGE` de `postprocessing` (= 3) — cf. note dans `CutinScene.tsx`.
const KERNEL_SIZE_LARGE = 3;

// --- Heuristiques de placement (transforms exacts du jeu inconnus, cf. spec §7). ---
/** Hauteur normalisée du joueur, en unités scène. */
const PLAYER_HEIGHT = 1.8;
/** L'enveloppe de l'effet ≈ 1,7× la hauteur du joueur. */
const EFFECT_RATIO = 1.7;
/** Centre vertical de l'effet = mi-hauteur du joueur (torse). */
const EFFECT_VY = PLAYER_HEIGHT * 0.5;
/** Léger décalage de l'effet (devant le joueur). */
const EFFECT_OFFSET = { x: 0, y: 0, z: 0.15 };

export interface ComposedHissatsuSceneProps {
	/** GLB model-full du joueur (corps + visage + uniforme), ex. `/model-full/c01001900.glb`. */
	playerUrl: string;
	/** GLB waza assemblé de la technique, ex. `/model-chr/waza/ev60_00340.glb`. */
	effectUrl: string;
	/** Storyboard caméra (g4cm réel via `loadCutinStoryboard`, fallback `defaultCutinShots`). */
	storyboard: CutinStoryboard;
	/** Backdrop stade optionnel (URL CDN webp). OFF par défaut (risque CORS → canvas tainté). */
	backdropUrl?: string | null;
	/** Bloom actif (défaut true). */
	bloom?: boolean;
	/** OrbitControls (exclusif du rig scénarisé). */
	orbit?: boolean;
	/** Lecture en cours (avance le playhead). Ignoré si `scrubSec` non-null. */
	playing?: boolean;
	/** Si non-null, fige `sequence.position = scrubSec`. */
	scrubSec: number | null;
	/** Notifié quand le plan caméra courant change (overlay). */
	onShot?: (shotName: string) => void;
	/** Remonte le MÊME `CutinSceneHandle` que `CutinScene` → export inchangé. */
	onReady?: (handle: CutinSceneHandle) => void;
}

/** `eventId` (clé du sheet Theatre) déduit du nom de fichier de l'effet, ex. `ev60_00340`. */
function eventIdFromEffectUrl(url: string): string {
	const base = url.split("/").pop() ?? url;
	return base.replace(/\.glb$/i, "") || "demo-composed";
}

/** Joueur : normalisé par la HAUTEUR (jamais maxDim), pieds exactement à y=0. */
function PlayerModel({ url }: { url: string }): React.JSX.Element {
	const gltf = useGLTF(url);
	const object = useMemo(() => {
		const clone = gltf.scene.clone(true);
		const box = new THREE.Box3().setFromObject(clone);
		const size = new THREE.Vector3();
		const center = new THREE.Vector3();
		box.getSize(size);
		box.getCenter(center);
		const scale = PLAYER_HEIGHT / (size.y || 1);
		clone.scale.setScalar(scale);
		// Recentre en X/Z, pose la base (pieds) sur le sol (y=0).
		clone.position.set(-center.x * scale, -box.min.y * scale, -center.z * scale);
		return clone;
	}, [gltf.scene]);
	return <primitive object={object} />;
}

/** Effet waza : échelle relative au joueur, centré sur son torse, émissif boosté (brille au Bloom). */
function EffectModel({
	url,
	mixerRef,
}: {
	url: string;
	mixerRef: React.RefObject<THREE.AnimationMixer | null>;
}): React.JSX.Element {
	const gltf = useGLTF(url);
	const object = useMemo(() => {
		const clone = gltf.scene.clone(true);
		const box = new THREE.Box3().setFromObject(clone);
		const size = new THREE.Vector3();
		const center = new THREE.Vector3();
		box.getSize(size);
		box.getCenter(center);
		const scale = (PLAYER_HEIGHT * EFFECT_RATIO) / (size.y || 1);
		clone.scale.setScalar(scale);
		// Recentré sur l'axe X/Z du joueur, centre vertical calé au torse + léger offset.
		clone.position.set(
			-center.x * scale + EFFECT_OFFSET.x,
			-center.y * scale + EFFECT_VY + EFFECT_OFFSET.y,
			-center.z * scale + EFFECT_OFFSET.z,
		);
		// Émissif boosté + hors tonemapping → l'effet « brille » dans le Bloom.
		clone.traverse((child) => {
			const mesh = child as THREE.Mesh;
			if (!mesh.isMesh) return;
			const mats = Array.isArray(mesh.material) ? mesh.material : [mesh.material];
			for (const m of mats) {
				const mat = m as THREE.MeshStandardMaterial;
				if ("emissiveIntensity" in mat) mat.emissiveIntensity = 2.5;
				mat.toneMapped = false;
			}
		});
		return clone;
	}, [gltf.scene]);

	// Mixer des animations de l'effet : piloté par `setTime(t)` dans `CameraRig` (reproductible
	// pour l'export). Le clip ev60_00340 peut être vide → no-op sûr.
	useEffect(() => {
		if (!gltf.animations.length) {
			mixerRef.current = null;
			return;
		}
		const mixer = new THREE.AnimationMixer(object);
		for (const clip of gltf.animations) mixer.clipAction(clip).play();
		mixerRef.current = mixer;
		return () => {
			mixer.stopAllAction();
			mixerRef.current = null;
		};
	}, [object, gltf.animations, mixerRef]);

	return <primitive object={object} />;
}

/** Backdrop stade (plan frontal lointain). OFF par défaut (cf. note CORS dans la spec). */
function BackdropPlane({ url }: { url: string }): React.JSX.Element {
	const texture = useTexture(url);
	return (
		<mesh position={[0, 4, -8]}>
			<planeGeometry args={[24, 13.5]} />
			<meshBasicMaterial map={texture} toneMapped={false} />
		</mesh>
	);
}

/** Pilote la caméra scénarisée + le playhead de la timeline maître + le mixer de l'effet. */
function CameraRig({
	storyboard,
	sequence,
	scrubSec,
	playing,
	enabled,
	onShot,
	mixerRef,
}: {
	storyboard: CutinStoryboard;
	sequence: ISequence;
	scrubSec: number | null;
	playing: boolean;
	enabled: boolean;
	onShot?: (name: string) => void;
	mixerRef: React.RefObject<THREE.AnimationMixer | null>;
}): null {
	const lastShot = useRef<string>("");
	useFrame((state, delta) => {
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
		const cam = state.camera;
		cam.position.set(s.position[0], s.position[1], s.position[2]);
		cam.lookAt(s.target[0], s.target[1], s.target[2]);
		const pc = cam as THREE.PerspectiveCamera;
		if (pc.isPerspectiveCamera && pc.fov !== s.fov) {
			pc.fov = s.fov;
			pc.updateProjectionMatrix();
		}
		// `setTime` (PAS `update(delta)`) → l'effet est figé au même `t` que la caméra, donc
		// reproductible pendant l'export (où `advance(t, true)` rejoue ce `useFrame`).
		const mixer = mixerRef.current;
		if (mixer) mixer.setTime(t % (storyboard.duration || 1));
		if (onShot && s.shotName !== lastShot.current) {
			lastShot.current = s.shotName;
			onShot(s.shotName);
		}
	});
	return null;
}

/** Remonte le handle de rendu (gl/scene/camera/advance/sequence) au wrapper — identique CutinScene. */
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
function SceneContent(props: ComposedHissatsuSceneProps): React.JSX.Element {
	const { playerUrl, effectUrl, storyboard, backdropUrl, bloom = true, orbit = false } = props;
	const eventId = useMemo(() => eventIdFromEffectUrl(effectUrl), [effectUrl]);
	const sheet = useMemo(() => getProject("azalee-cutin").sheet(eventId), [eventId]);
	const sequence = sheet.sequence;
	const mixerRef = useRef<THREE.AnimationMixer | null>(null);

	// Réinitialise le playhead au montage / changement de durée (cf. CutinScene).
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

	return (
		<SheetProvider sheet={sheet}>
			{/* Éclairage 100 % local (jamais d'Environment HDR externe → CSP). */}
			<hemisphereLight args={[0xffffff, 0x33384a, 1.1]} />
			<ambientLight intensity={0.35} />
			<directionalLight position={[3, 5, 4]} intensity={1.6} castShadow />
			<directionalLight position={[-4, 2, -3]} intensity={0.8} color={0xbcd0ff} />

			{/* Sol procédural + ombre de contact ancrée au plan des pieds (y=0). */}
			<mesh rotation-x={-Math.PI / 2} position-y={0} receiveShadow>
				<planeGeometry args={[40, 40]} />
				<meshStandardMaterial color="#2e6b3a" />
			</mesh>
			<ContactShadows
				position={[0, 0.001, 0]}
				opacity={0.55}
				scale={10}
				blur={2.4}
				far={4}
				resolution={1024}
				color="#05210f"
			/>

			<Suspense fallback={null}>
				{/* Joueur (pieds au sol) + son effet (échelle relative, autour du torse). */}
				<PlayerModel url={playerUrl} />
				<EffectModel url={effectUrl} mixerRef={mixerRef} />
				{backdropUrl ? <BackdropPlane url={backdropUrl} /> : null}
			</Suspense>

			<CameraRig
				storyboard={storyboard}
				sequence={sequence}
				scrubSec={props.scrubSec}
				playing={props.playing ?? false}
				enabled={!orbit}
				onShot={props.onShot}
				mixerRef={mixerRef}
			/>
			{orbit && <OrbitControls makeDefault enableDamping target={[0, EFFECT_VY, 0]} />}

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

/**
 * Canvas R3F de la scène composée. `preserveDrawingBuffer` IMPÉRATIF : sans lui, la capture
 * `new VideoFrame(canvas)` de l'export renverrait un cadre NOIR.
 */
export default function ComposedHissatsuScene(
	props: ComposedHissatsuSceneProps,
): React.JSX.Element {
	return (
		<Canvas
			frameloop="always"
			shadows
			gl={{ preserveDrawingBuffer: true }}
			camera={{ fov: 38, position: [0, 1, 4] }}
			dpr={[1, 2]}
		>
			<SceneContent {...props} />
		</Canvas>
	);
}
