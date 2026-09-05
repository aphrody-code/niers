// Viewport 3D TEMPS RÉEL — le cœur du mode éditeur.
//
// Les aperçus VFS et CPK brut convergent ici : le GLB assemblé (géométrie + textures embarquées)
// est chargé dans un VRAI moteur temps réel WebGL — caméra libre, éclairage, sélection de
// maillage, wireframe : le viewport d'un éditeur, pas une planche-contact.
//
// three.js est importé depuis le paquet npm (bundlé par Vite) — aucun CDN, l'app reste
// intégralement hors ligne comme le reste de niers (même contrainte que `monacoSetup.ts`).
//
// Les transformations faites au gizmo sont LOCALES À LA SESSION : elles ne sont écrites nulle
// part et disparaissent au rechargement. Aucun encodeur géométrique n'existe côté Rust — `g4mg.rs`,
// `g4md.rs` et `g4sk.rs` n'exposent que du décodage — donc aucune affordance d'enregistrement n'est
// proposée ici : un bouton « sauvegarder » mentirait sur ce que le dépôt sait faire.
//
// La scène porte PLUSIEURS assets simultanément (`assets`), chacun indexé par son chemin VFS :
// c'est ce qui distingue un éditeur d'une visionneuse, et ce qui impose de libérer le GPU asset par
// asset plutôt qu'en bloc.
import { useEffect, useRef, useState } from "react";
import * as THREE from "three";
import { GLTFLoader } from "three/examples/jsm/loaders/GLTFLoader.js";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { TransformControls } from "three/examples/jsm/controls/TransformControls.js";

import { b64ToBytes } from "@/lib/bytes";

/** Un asset chargé dans la scène. Plusieurs coexistent — cf. en-tête. */
export interface ViewportAsset {
  /** Chemin VFS : clé stable de l'asset dans la scène et préfixe des identifiants de noeuds. */
  key: string;
  /** GLB en base64 (`api.glbBytesB64`). */
  glbB64: string;
}

/** Un noeud de la scène chargée, à plat — alimente l'outliner. */
export interface SceneNode {
  /** `<clé d'asset>#<index>` : deux assets portent volontiers les mêmes noms de noeuds. */
  id: string;
  /** Asset porteur, pour grouper la hiérarchie. */
  assetKey: string;
  name: string;
  type: string;
  depth: number;
  /** Nombre de triangles pour un maillage (0 sinon) — l'info que tout éditeur affiche. */
  triangles: number;
}

export interface ViewportStats {
  meshes: number;
  triangles: number;
  vertices: number;
  materials: number;
}

/** Mode du gizmo. `none` : aucun gizmo, seule la sélection au clic reste active. */
export type GizmoMode = "none" | "translate" | "rotate" | "scale";

/** Transformation locale d'un noeud — rotation en radians, ordre d'Euler XYZ. */
export interface NodeTransform {
  position: [number, number, number];
  rotation: [number, number, number];
  scale: [number, number, number];
}

export interface Viewport3DProps {
  /** Assets composant la scène, dans l'ordre d'affichage. Vide = viewport vide. */
  assets: ViewportAsset[];
  /** Identifiant du noeud à mettre en surbrillance (depuis l'outliner). */
  selectedId: string | null;
  onSelect?: (id: string | null) => void;
  onSceneLoaded?: (nodes: SceneNode[], stats: ViewportStats) => void;
  /** Émis à chaque manipulation du gizmo, et une fois à la sélection (état initial). */
  onTransform?: (id: string, trs: NodeTransform) => void;
  gizmoMode?: GizmoMode;
  /** Message affiché en surimpression quand la scène est vide (asset non assemblable, erreur…). */
  notice?: string | null;
  wireframe?: boolean;
  showGrid?: boolean;
  className?: string;
}

/** État three.js persistant du viewport — hors React, cf. `gl` plus bas. */
interface GlState {
  renderer: THREE.WebGLRenderer;
  scene: THREE.Scene;
  camera: THREE.PerspectiveCamera;
  controls: OrbitControls;
  gizmo: TransformControls;
  gizmoHelper: ReturnType<TransformControls["getHelper"]>;
  /** Noeud attaché au gizmo — `TransformControls.object` vaut la caméra tant qu'on n'a pas attaché. */
  gizmoTarget: THREE.Object3D | null;
  grid: THREE.GridHelper;
  /** Racine de chaque asset, par chemin VFS. */
  assets: Map<string, THREE.Group>;
  byId: Map<string, THREE.Object3D>;
  selectionBox: THREE.BoxHelper | null;
  raf: number;
}

function readTransform(o: THREE.Object3D): NodeTransform {
  return {
    position: [o.position.x, o.position.y, o.position.z],
    rotation: [o.rotation.x, o.rotation.y, o.rotation.z],
    scale: [o.scale.x, o.scale.y, o.scale.z],
  };
}

/** `Material.dispose()` ne libère PAS les textures qu'il référence (three ≥ r152) : sans ce
 * parcours, chaque asset retiré de la scène laisse ses images en mémoire GPU. */
function disposeMaterial(material: THREE.Material) {
  for (const value of Object.values(material as unknown as Record<string, unknown>)) {
    if (value && (value as THREE.Texture).isTexture) (value as THREE.Texture).dispose();
  }
  material.dispose();
}

/** Retire un asset de la scène et libère sa mémoire GPU. Par asset : la scène en porte plusieurs,
 * un `dispose()` global libérerait les modèles restés à l'écran. */
function disposeAsset(state: GlState, key: string) {
  const group = state.assets.get(key);
  if (!group) return;
  if (state.gizmoTarget && group.getObjectById(state.gizmoTarget.id)) {
    state.gizmo.detach();
    state.gizmoTarget = null;
  }
  state.scene.remove(group);
  disposeObject(group);
  state.assets.delete(key);
}

/** Libère également un chargement terminé après l'annulation de sa requête. */
function disposeObject(group: THREE.Object3D) {
  group.traverse((o) => {
    const mesh = o as THREE.Mesh;
    if (mesh.geometry) mesh.geometry.dispose();
    const mat = mesh.material;
    if (Array.isArray(mat)) mat.forEach(disposeMaterial);
    else if (mat) disposeMaterial(mat as THREE.Material);
  });
}

/** Cadre la caméra sur la boîte englobante de TOUS les assets — sans ça, un modèle de 2 unités et
 * un modèle de 200 unités s'affichent l'un microscopique, l'autre hors champ, et l'ajout d'un
 * second asset laisserait la caméra collée au premier. */
function frameObjects(
  objects: Iterable<THREE.Object3D>,
  camera: THREE.PerspectiveCamera,
  controls: OrbitControls,
) {
  const box = new THREE.Box3();
  for (const o of objects) box.union(new THREE.Box3().setFromObject(o));
  if (box.isEmpty()) return;
  const size = box.getSize(new THREE.Vector3());
  const center = box.getCenter(new THREE.Vector3());
  const maxDim = Math.max(size.x, size.y, size.z) || 1;
  const fitDist = (maxDim / 2) / Math.tan((camera.fov * Math.PI) / 360);

  controls.target.copy(center);
  camera.position.set(center.x + fitDist * 0.9, center.y + maxDim * 0.35, center.z + fitDist * 1.4);
  camera.near = maxDim / 1000;
  camera.far = maxDim * 1000;
  camera.updateProjectionMatrix();
  controls.update();
}

export function Viewport3D({
  assets,
  selectedId,
  onSelect,
  onSceneLoaded,
  onTransform,
  gizmoMode = "none",
  notice = null,
  wireframe = false,
  showGrid = true,
  className,
}: Viewport3DProps) {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  // Objets three.js persistants entre rendus React — dans une ref, jamais dans un état : les
  // toucher ne doit provoquer aucun re-rendu, et la boucle d'animation doit survivre aux
  // changements de props.
  const gl = useRef<GlState | null>(null);

  // Les écouteurs three.js sont posés une seule fois, au montage : ils doivent lire les props du
  // rendu COURANT, pas celles capturées à ce moment-là.
  const assetsRef = useRef(assets);
  assetsRef.current = assets;
  const onTransformRef = useRef(onTransform);
  onTransformRef.current = onTransform;
  const onSceneLoadedRef = useRef(onSceneLoaded);
  onSceneLoadedRef.current = onSceneLoaded;
  const wireframeRef = useRef(wireframe);
  wireframeRef.current = wireframe;

  // Signature de la scène : l'identité du tableau `assets` change à chaque rendu du parent, pas son
  // contenu. Le GLB d'une clé donnée ne change jamais — les clés suffisent donc à décider.
  const assetsKey = JSON.stringify(assets.map((a) => a.key));

  // Montage : renderer, scène, caméra, éclairage, boucle. Une seule fois — le contexte WebGL est
  // coûteux à recréer et le perdre à chaque changement de fichier ferait clignoter le viewport.
  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;

    // `new WebGLRenderer()` LÈVE quand aucun contexte WebGL n'est disponible — WebView2 sans
    // accélération matérielle, pilote refusé, trop de contextes vivants. Une exception jetée dans
    // un effet démonte tout l'arbre React : sans ce `try`, un poste sans WebGL n'obtient pas un
    // viewport vide mais une FENÊTRE BLANCHE. On dit ce qui manque, et le reste de l'éditeur
    // (outliner, navigateur de contenu, détails) continue de servir.
    let renderer: THREE.WebGLRenderer;
    try {
      renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
    } catch (e) {
      setError(
        "WebGL indisponible sur ce poste : le viewport temps réel ne peut pas démarrer " +
          `(${e instanceof Error ? e.message : String(e)}). Les aperçus rendus côté Rust ` +
          "(bouton « Aperçu 3D » du panneau de détail) restent disponibles.",
      );
      return;
    }
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    renderer.outputColorSpace = THREE.SRGBColorSpace;
    host.appendChild(renderer.domElement);
    renderer.domElement.style.width = "100%";
    renderer.domElement.style.height = "100%";
    renderer.domElement.style.display = "block";

    const scene = new THREE.Scene();
    const camera = new THREE.PerspectiveCamera(50, 1, 0.01, 1000);
    camera.position.set(2, 1.5, 3);

    const controls = new OrbitControls(camera, renderer.domElement);
    controls.enableDamping = true;
    controls.dampingFactor = 0.08;

    // Éclairage neutre à trois sources : les modèles du jeu n'embarquent pas de lumières, un
    // simple ambient les rendrait plats.
    scene.add(new THREE.AmbientLight(0xffffff, 1.1));
    const key = new THREE.DirectionalLight(0xffffff, 1.6);
    key.position.set(3, 5, 4);
    scene.add(key);
    const fill = new THREE.DirectionalLight(0x99bbff, 0.6);
    fill.position.set(-4, 2, -3);
    scene.add(fill);

    const grid = new THREE.GridHelper(10, 20, 0x3b82f6, 0x2a2c3a);
    (grid.material as THREE.Material).transparent = true;
    (grid.material as THREE.Material).opacity = 0.35;
    scene.add(grid);

    // Gizmo de transformation. En three 0.185 `TransformControls` n'est PLUS un `Object3D` (il
    // dérive de `Controls`) : `scene.add(gizmo)` lève — c'est `getHelper()` qui porte la
    // représentation visuelle.
    const gizmo = new TransformControls(camera, renderer.domElement);
    const gizmoHelper = gizmo.getHelper();
    scene.add(gizmoHelper);
    gizmo.detach();

    const state: GlState = {
      renderer,
      scene,
      camera,
      controls,
      gizmo,
      gizmoHelper,
      gizmoTarget: null,
      grid,
      assets: new Map(),
      byId: new Map(),
      selectionBox: null,
      raf: 0,
    };
    gl.current = state;

    // Sans ça, OrbitControls tourne la caméra en même temps qu'on tire sur une poignée du gizmo :
    // les deux contrôleurs écoutent le même canvas.
    gizmo.addEventListener("dragging-changed", (e) => {
      controls.enabled = !(e.value as boolean);
    });
    // Un `BoxHelper` est figé à sa construction : sans `update()` le cadre de sélection reste sur
    // l'ancienne position pendant toute la manipulation.
    gizmo.addEventListener("objectChange", () => {
      state.selectionBox?.update();
      const target = state.gizmoTarget;
      if (target) onTransformRef.current?.(target.userData.nieId as string, readTransform(target));
    });

    const resize = () => {
      const w = host.clientWidth || 1;
      const h = host.clientHeight || 1;
      renderer.setSize(w, h, false);
      camera.aspect = w / h;
      camera.updateProjectionMatrix();
    };
    resize();
    const observer = new ResizeObserver(resize);
    observer.observe(host);

    // Perte de contexte GPU (pilote réinitialisé, veille, trop de contextes) : le navigateur
    // l'annonce par un événement, et `render()` lèverait à la frame suivante. On arrête la boucle
    // proprement et on le dit — sinon l'exception traverse le `requestAnimationFrame`, où plus
    // aucune barrière React ne peut l'attraper, et la fenêtre blanchit.
    const onContextLost = (e: Event) => {
      e.preventDefault();
      cancelAnimationFrame(state.raf);
      state.raf = 0;
      setError("Contexte WebGL perdu (pilote graphique réinitialisé). Rechargez l'asset pour reprendre.");
    };
    const onContextRestored = () => {
      setError(null);
      if (state.raf === 0) tick();
    };
    renderer.domElement.addEventListener("webglcontextlost", onContextLost);
    renderer.domElement.addEventListener("webglcontextrestored", onContextRestored);

    const tick = () => {
      state.raf = requestAnimationFrame(tick);
      controls.update();
      try {
        renderer.render(scene, camera);
      } catch (e) {
        // Une frame qui lève ne doit pas relancer indéfiniment une boucle qui lèvera encore.
        cancelAnimationFrame(state.raf);
        state.raf = 0;
        setError(`Rendu interrompu : ${e instanceof Error ? e.message : String(e)}`);
      }
    };
    tick();

    return () => {
      cancelAnimationFrame(state.raf);
      renderer.domElement.removeEventListener("webglcontextlost", onContextLost);
      renderer.domElement.removeEventListener("webglcontextrestored", onContextRestored);
      observer.disconnect();
      for (const key of [...state.assets.keys()]) disposeAsset(state, key);
      if (state.selectionBox) disposeObject(state.selectionBox);
      disposeObject(grid);
      gizmo.detach();
      scene.remove(gizmoHelper);
      gizmoHelper.dispose();
      gizmo.dispose();
      controls.dispose();
      renderer.dispose();
      host.removeChild(renderer.domElement);
      gl.current = null;
    };
  }, []);

  // Synchronise la scène avec `assets` : retire ce qui a disparu, charge ce qui est nouveau, laisse
  // en place ce qui n'a pas bougé — recharger tout à chaque ajout ferait clignoter la scène et
  // perdrait les transformations de session.
  useEffect(() => {
    const state = gl.current;
    if (!state) return;

    const wanted = new Set(assetsRef.current.map((a) => a.key));
    for (const key of [...state.assets.keys()]) if (!wanted.has(key)) disposeAsset(state, key);

    const missing = assetsRef.current.filter((a) => !state.assets.has(a.key));
    if (missing.length === 0) {
      setLoading(false);
      rebuildOutline(false);
      return;
    }

    setError(null);
    setLoading(true);
    let cancelled = false;
    let pending = missing.length;
    const settle = () => {
      if (--pending > 0) return;
      setLoading(false);
      rebuildOutline(true);
    };

    for (const asset of missing) {
      // TOUT ce bloc est protégé : `GLTFLoader.parse` lève de façon SYNCHRONE sur un en-tête GLB
      // invalide (avant d'appeler le rappel d'erreur), `atob` lève sur du base64 tronqué, et
      // l'allocation du tampon lève quand la mémoire manque. Une exception ici traverse l'effet
      // et démonte l'arbre React entier — c'est-à-dire une fenêtre blanche pour un seul modèle
      // illisible. Le rappel d'erreur asynchrone ne suffit donc PAS.
      try {
        const bytes = b64ToBytes(asset.glbB64);
        // `slice()` : `parse` veut un ArrayBuffer, et celui d'un Uint8Array issu du décodage peut être
        // plus grand que la vue (offset non nul).
        const buffer = bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer;
        new GLTFLoader().parse(
          buffer,
          "",
          (gltf) => {
            if (cancelled || gl.current !== state) {
              disposeObject(gltf.scene);
              return;
            }
            try {
              gltf.scene.userData.nieAsset = asset.key;
              gl.current.scene.add(gltf.scene);
              gl.current.assets.set(asset.key, gltf.scene);
            } catch (e) {
              setError(`Ajout du modèle à la scène : ${e instanceof Error ? e.message : String(e)}`);
            }
            settle();
          },
          (e) => {
            if (cancelled) return;
            setError(`Chargement du modèle : ${e instanceof Error ? e.message : String(e)}`);
            settle();
          },
        );
      } catch (e) {
        if (cancelled) continue;
        setError(`Modèle « ${asset.key.split("/").pop()} » illisible : ${e instanceof Error ? e.message : String(e)}`);
        settle();
      }
    }

    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [assetsKey]);

  // Aplatit la hiérarchie de TOUS les assets pour l'outliner, indexe les objets par identifiant
  // stable et recalcule les statistiques.
  function rebuildOutline(refit: boolean) {
    try {
      rebuildOutlineInner(refit);
    } catch (e) {
      // Le parcours de scène touche des objets three.js et rappelle le parent : une exception ici
      // partirait dans un effet, donc dans la barrière — et emporterait l'éditeur pour un
      // outliner. La scène, elle, est déjà à l'écran et reste utilisable.
      setError(`Analyse de la scène : ${e instanceof Error ? e.message : String(e)}`);
    }
  }

  function rebuildOutlineInner(refit: boolean) {
    const state = gl.current;
    if (!state) return;

    const nodes: SceneNode[] = [];
    const stats: ViewportStats = { meshes: 0, triangles: 0, vertices: 0, materials: 0 };
    const materials = new Set<string>();
    state.byId.clear();

    for (const asset of assetsRef.current) {
      const group = state.assets.get(asset.key);
      if (!group) continue;
      let nextId = 1;

      const walk = (obj: THREE.Object3D, depth: number) => {
        const id = `${asset.key}#${nextId++}`;
        obj.userData.nieId = id;
        state.byId.set(id, obj);

        let triangles = 0;
        const mesh = obj as THREE.Mesh;
        if (mesh.isMesh && mesh.geometry) {
          const geom = mesh.geometry;
          const count = geom.index ? geom.index.count : geom.getAttribute("position")?.count ?? 0;
          triangles = Math.floor(count / 3);
          stats.meshes += 1;
          stats.triangles += triangles;
          stats.vertices += geom.getAttribute("position")?.count ?? 0;
          const mats = Array.isArray(mesh.material) ? mesh.material : [mesh.material];
          mats.forEach((m) => {
            if (!m) return;
            materials.add(m.uuid);
            if ("wireframe" in m) (m as THREE.MeshStandardMaterial).wireframe = wireframeRef.current;
          });
        }

        nodes.push({
          id,
          assetKey: asset.key,
          name: obj.name || (mesh.isMesh ? "Mesh" : obj.type),
          type: obj.type,
          depth,
          triangles,
        });
        obj.children.forEach((c) => walk(c, depth + 1));
      };
      group.children.forEach((c) => walk(c, 0));
    }
    stats.materials = materials.size;

    if (refit) frameObjects(state.assets.values(), state.camera, state.controls);
    onSceneLoadedRef.current?.(nodes, stats);
  }

  // Wireframe / grille — appliqués sans recharger les modèles.
  useEffect(() => {
    const state = gl.current;
    if (!state) return;
    for (const group of state.assets.values()) {
      group.traverse((o) => {
        const mesh = o as THREE.Mesh;
        if (!mesh.isMesh) return;
        const mats = Array.isArray(mesh.material) ? mesh.material : [mesh.material];
        mats.forEach((m) => {
          if (m && "wireframe" in m) (m as THREE.MeshStandardMaterial).wireframe = wireframe;
        });
      });
    }
  }, [wireframe, assetsKey]);

  useEffect(() => {
    if (gl.current) gl.current.grid.visible = showGrid;
  }, [showGrid]);

  // Surbrillance de la sélection : boîte englobante du noeud choisi dans l'outliner.
  useEffect(() => {
    const state = gl.current;
    if (!state) return;
    if (state.selectionBox) {
      state.scene.remove(state.selectionBox);
      state.selectionBox.geometry.dispose();
      disposeMaterial(state.selectionBox.material as THREE.Material);
      state.selectionBox = null;
    }
    if (selectedId == null) return;
    const target = state.byId.get(selectedId);
    if (!target) return;
    const helper = new THREE.BoxHelper(target, 0x3b82f6);
    helper.name = "__nie_selection__";
    state.scene.add(helper);
    state.selectionBox = helper;
    // L'inspecteur montre la transformation dès la sélection, sans attendre une manipulation.
    onTransformRef.current?.(selectedId, readTransform(target));
  }, [selectedId, assetsKey]);

  // Gizmo : attaché au noeud sélectionné, détaché en mode `none` ou sans sélection.
  useEffect(() => {
    const state = gl.current;
    if (!state) return;
    const target = selectedId == null ? null : state.byId.get(selectedId) ?? null;
    state.gizmoTarget = target;
    if (!target || gizmoMode === "none") {
      state.gizmo.detach();
      return;
    }
    state.gizmo.setMode(gizmoMode);
    state.gizmo.attach(target);
  }, [gizmoMode, selectedId, assetsKey]);

  // Clic dans le viewport → sélection par lancer de rayon, comme tout éditeur 3D.
  function onPointerDown(e: React.PointerEvent) {
    const state = gl.current;
    if (!state || state.assets.size === 0) return;
    // Le gizmo est prioritaire sur la sélection : sans cette garde, saisir une poignée lance un
    // raycast qui rate le maillage et déselectionne l'objet qu'on est en train de déplacer.
    if (state.gizmo.dragging || state.gizmo.axis !== null) return;
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const ndc = new THREE.Vector2(
      ((e.clientX - rect.left) / rect.width) * 2 - 1,
      -((e.clientY - rect.top) / rect.height) * 2 + 1,
    );
    const ray = new THREE.Raycaster();
    ray.setFromCamera(ndc, state.camera);
    const hit = ray.intersectObjects([...state.assets.values()], true)[0];
    onSelect?.(hit ? (hit.object.userData.nieId as string) ?? null : null);
  }

  function setCameraView(direction: "front" | "side" | "back") {
    const state = gl.current;
    if (!state || state.assets.size === 0) return;
    const box = new THREE.Box3();
    for (const object of state.assets.values()) box.union(new THREE.Box3().setFromObject(object));
    if (box.isEmpty()) return;
    const center = box.getCenter(new THREE.Vector3());
    const size = box.getSize(new THREE.Vector3());
    const halfHeight = size.y / 2;
    const halfWidth = (direction === "side" ? size.z : size.x) / 2;
    const halfDepth = (direction === "side" ? size.x : size.z) / 2;
    const tangent = Math.tan(THREE.MathUtils.degToRad(state.camera.fov / 2));
    const distance = Math.max(halfHeight / tangent, halfWidth / (tangent * state.camera.aspect), 0.01) * 1.15 + halfDepth;
    state.controls.target.copy(center);
    state.camera.position.copy(center).add(
      direction === "side" ? new THREE.Vector3(distance, 0, 0) : new THREE.Vector3(0, 0, direction === "back" ? -distance : distance),
    );
    state.controls.update();
  }

  return (
    <div className={className} style={{ position: "relative" }}>
      <div ref={hostRef} className="h-full w-full" onPointerDown={onPointerDown} />
      {assets.length > 0 && !loading && !error && (
        <div className="absolute bottom-2 left-2 flex gap-1" aria-label="Orientation de la caméra">
          {([["front", "Face"], ["side", "Côté"], ["back", "Dos"]] as const).map(([direction, label]) => (
            <button key={direction} type="button" onClick={() => setCameraView(direction)}
              className="rounded border border-app-line bg-app-box/90 px-2 py-1 text-xs text-ink hover:bg-app-selected">
              {label}
            </button>
          ))}
        </div>
      )}
      {loading && (
        <div className="pointer-events-none absolute left-3 top-3 rounded-md bg-app-box/80 px-2 py-1 text-tiny text-ink-dull">
          chargement du modèle…
        </div>
      )}
      {error && (
        <div className="absolute left-3 top-3 max-w-[80%] rounded-md bg-app-box/90 px-2 py-1 text-tiny text-status-error">
          {error}
        </div>
      )}
      {/* Un asset non assemblable ne doit pas se solder par un viewport muet : on dit lequel et
       * pourquoi, plutôt que de laisser l'échec dans un coin de la barre d'outils. */}
      {notice && assets.length === 0 && !loading && (
        <div className="pointer-events-none absolute inset-0 flex items-center justify-center p-6">
          <p className="max-w-md rounded-md border border-app-line bg-app-box/90 px-3 py-2 text-center text-tiny leading-relaxed text-ink-dull">
            {notice}
          </p>
        </div>
      )}
      {assets.length === 0 && !loading && !error && !notice && (
        <div className="pointer-events-none absolute inset-0 flex items-center justify-center text-xs text-ink-faint">
          Sélectionnez un modèle assemblable (.g4md) dans le navigateur de contenu.
        </div>
      )}
    </div>
  );
}
