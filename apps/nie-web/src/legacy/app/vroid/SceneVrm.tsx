/**
 * Rendu three.js d'un modèle VRM (cible d'un `next/dynamic { ssr: false }`).
 *
 * PAS de `"use client"` ici : ce module n'est chargé QUE par
 * `VisionneuseVrm`, déjà client et déjà hors SSR. Le garder sans directive
 * évite que Turbopack ne l'agrège au bundle client des pages qui ne
 * l'affichent pas — `three` + `@pixiv/three-vrm` pèsent plusieurs centaines
 * de kilo-octets.
 *
 * three.js « à la main » plutôt que react-three-fiber : la visionneuse n'a
 * qu'une scène, un modèle et une caméra orbitale, et `VRMLoaderPlugin`
 * s'enregistre sur un `GLTFLoader` — le passer par `useLoader` n'apporterait
 * qu'une couche de suspense à déboguer.
 *
 * Le `.vrm` arrive par `/api/vroid/vrm/{id}`, qui relaie le flux sans le
 * stocker. Rien n'est mis en cache ici non plus : l'`ArrayBuffer` est libéré
 * avec la scène.
 */
import { useEffect, useRef, useState } from "react";
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { GLTFLoader } from "three/examples/jsm/loaders/GLTFLoader.js";
import { VRM, VRMLoaderPlugin, VRMUtils } from "@pixiv/three-vrm";

export interface SceneVrmProps {
	/** Identifiant VRoid Hub du modèle à charger. */
	idModele: string;
	/** Nom affiché dans le message d'erreur, à défaut d'identifiant lisible. */
	nom: string;
}

/** États de la visionneuse, dans leur ordre d'apparition. */
type EtatScene =
	| { phase: "chargement" }
	| { phase: "pret"; specification: string }
	| { phase: "erreur"; message: string };

/**
 * Cadre la caméra sur le modèle chargé.
 *
 * Un VRM est à l'échelle humaine (≈ 1,5 m) mais son origine est aux pieds :
 * viser le centre de la boîte englobante évite de regarder le sol.
 */
function cadrer(camera: THREE.PerspectiveCamera, controles: OrbitControls, objet: THREE.Object3D): void {
	const boite = new THREE.Box3().setFromObject(objet);
	const taille = boite.getSize(new THREE.Vector3());
	const centre = boite.getCenter(new THREE.Vector3());

	const hauteur = Math.max(taille.y, 0.1);
	const distance = hauteur * 1.6;

	camera.position.set(centre.x, centre.y + hauteur * 0.08, centre.z + distance);
	camera.near = distance / 100;
	camera.far = distance * 100;
	camera.updateProjectionMatrix();

	controles.target.copy(centre);
	controles.minDistance = distance * 0.3;
	controles.maxDistance = distance * 4;
	controles.update();
}

export default function SceneVrm({ idModele, nom }: SceneVrmProps) {
	const conteneurRef = useRef<HTMLDivElement | null>(null);
	const [etat, setEtat] = useState<EtatScene>({ phase: "chargement" });

	useEffect(() => {
		const conteneur = conteneurRef.current;
		if (!conteneur) return;

		const abandon = new AbortController();
		let anime = 0;
		let vrm: VRM | null = null;
		let detruit = false;

		const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
		renderer.setPixelRatio(Math.min(globalThis.devicePixelRatio || 1, 2));
		renderer.setSize(conteneur.clientWidth, conteneur.clientHeight);
		renderer.outputColorSpace = THREE.SRGBColorSpace;
		conteneur.appendChild(renderer.domElement);

		const scene = new THREE.Scene();
		const camera = new THREE.PerspectiveCamera(
			30,
			conteneur.clientWidth / Math.max(conteneur.clientHeight, 1),
			0.1,
			100
		);

		// Éclairage 100 % local : la CSP d'azalée interdit d'aller chercher un
		// HDR distant, et un MToon se contente très bien d'un ciel + une clé.
		scene.add(new THREE.HemisphereLight(0xffffff, 0x9aa0b4, 2.2));
		const cle = new THREE.DirectionalLight(0xffffff, 1.4);
		cle.position.set(1, 2, 1.5);
		scene.add(cle);

		const controles = new OrbitControls(camera, renderer.domElement);
		controles.enableDamping = true;
		controles.enablePan = false;
		camera.position.set(0, 1.3, 2.2);
		controles.target.set(0, 1.1, 0);
		controles.update();

		const horloge = new THREE.Clock();
		const boucle = () => {
			anime = requestAnimationFrame(boucle);
			const delta = horloge.getDelta();
			// `update` fait vivre les os à ressort et les contraintes du VRM.
			vrm?.update(delta);
			controles.update();
			renderer.render(scene, camera);
		};

		const redimensionner = () => {
			if (detruit || !conteneur.clientWidth) return;
			camera.aspect = conteneur.clientWidth / Math.max(conteneur.clientHeight, 1);
			camera.updateProjectionMatrix();
			renderer.setSize(conteneur.clientWidth, conteneur.clientHeight);
		};
		const observateur = new ResizeObserver(redimensionner);
		observateur.observe(conteneur);

		(async () => {
			try {
				const reponse = await fetch(`/api/vroid/vrm/${encodeURIComponent(idModele)}`, {
					signal: abandon.signal,
				});

				if (!reponse.ok) {
					const details = (await reponse.json().catch(() => null)) as { erreur?: string } | null;
					throw new Error(details?.erreur ?? `Chargement refusé (HTTP ${reponse.status}).`);
				}

				const octets = await reponse.arrayBuffer();
				if (detruit) return;

				const chargeur = new GLTFLoader();
				chargeur.register((parser) => new VRMLoaderPlugin(parser));

				const gltf = await chargeur.parseAsync(octets, "");
				if (detruit) return;

				const charge = gltf.userData.vrm as VRM | undefined;
				if (!charge) throw new Error("Le fichier reçu n'est pas un modèle VRM.");

				vrm = charge;

				// Un VRM 0.0 regarde vers +Z : sans cette rotation, le modèle
				// tourne le dos à la caméra.
				VRMUtils.rotateVRM0(charge);
				// Fusionne les squelettes redondants — moins d'appels de dessin.
				VRMUtils.combineSkeletons(charge.scene);
				// Le regard suivrait la caméra par défaut : figé, c'est plus lisible.
				if (charge.lookAt) charge.lookAt.autoUpdate = false;

				scene.add(charge.scene);
				cadrer(camera, controles, charge.scene);

				setEtat({
					phase: "pret",
					specification: charge.meta.metaVersion === "0" ? "VRM 0.0" : "VRM 1.0",
				});
				boucle();
			} catch (cause) {
				if (detruit || abandon.signal.aborted) return;
				setEtat({
					phase: "erreur",
					message: cause instanceof Error ? cause.message : `Chargement de « ${nom} » impossible.`,
				});
			}
		})();

		return () => {
			detruit = true;
			abandon.abort();
			cancelAnimationFrame(anime);
			observateur.disconnect();
			controles.dispose();
			// `deepDispose` libère géométries, matériaux ET textures du VRM :
			// sans lui, chaque modèle visionné fuit plusieurs Mo de VRAM.
			if (vrm) VRMUtils.deepDispose(vrm.scene);
			renderer.dispose();
			renderer.domElement.remove();
		};
	}, [idModele, nom]);

	return (
		<div className="relative aspect-square w-full overflow-hidden rounded-2xl bg-surface-container-high">
			<div ref={conteneurRef} className="absolute inset-0" />

			{etat.phase === "chargement" && (
				<div className="absolute inset-0 flex flex-col items-center justify-center gap-3 text-on-surface-variant">
					<div className="size-8 animate-spin rounded-full border-b-2 border-primary" />
					<p className="text-sm">Chargement du modèle…</p>
				</div>
			)}

			{etat.phase === "erreur" && (
				<div className="absolute inset-0 flex items-center justify-center p-6 text-center">
					<p className="text-sm text-on-surface-variant">{etat.message}</p>
				</div>
			)}

			{etat.phase === "pret" && (
				<span className="absolute bottom-2 right-3 rounded-full bg-surface-container-low/80 px-2 py-0.5 text-xs text-on-surface-variant">
					{etat.specification}
				</span>
			)}
		</div>
	);
}
