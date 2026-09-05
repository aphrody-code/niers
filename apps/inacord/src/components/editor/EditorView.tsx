// Mode ÉDITEUR — nie-explorer en logiciel type Unreal Engine.
//
// Disposition canonique d'un éditeur de moteur, chaque zone servie par ce que niers sait déjà
// faire :
//
//   ┌──────────────────────────── barre d'outils ────────────────────────────┐
//   │  viewport 3D temps réel (WebGL)              │  outliner (hiérarchie)  │
//   │  caméra libre, sélection au clic             │  détails (propriétés)   │
//   ├───────────────────────────────────────────────────────────────────────┤
//   │  navigateur de contenu (VFS, vignettes, filtres par type d'asset)      │
//   └───────────────────────────────────────────────────────────────────────┘
//
// Le backend renvoie le GLB assemblé (`vfs_glb_bytes_b64`) et le modèle vit dans le même moteur
// temps réel que les aperçus VFS et CPK brut : caméra orbitale, raycast de sélection, wireframe
// et statistiques de scène.
//
// Le panneau « Détails » est l'éditeur de propriétés déjà en place (`PropertyEditor`) : il relie
// l'objet sélectionné à ses fichiers, ses `.cfg.bin` éditables et les fonctions/adresses de
// `nie.exe` qui le manipulent. Sélectionner un modèle dans le navigateur de contenu ouvre donc à
// la fois sa géométrie dans le viewport et sa fiche complète à droite.
//
// MULTI-ASSETS : la scène porte plusieurs modèles à la fois (ctrl/cmd+clic dans le navigateur de
// contenu). `EditorViewState` ne connaît que l'asset PRINCIPAL — celui que l'Explorateur ouvre et
// que l'éditeur de propriétés décrit ; les assets ajoutés vivent ici, dans l'état local de la vue.
//
// GIZMO : il agit sur le NOEUD DE SCÈNE sélectionné et reste local à la session — aucun encodeur
// géométrique n'existe côté Rust, rien n'est écrit ni écrivable. C'est pourquoi position/rotation/
// échelle s'affichent dans la carte du noeud (référentiel « scène ») et jamais dans l'onglet
// Détails, qui décrit l'ASSET par son code (référentiel « données du jeu ») : mélanger les deux
// référentiels ferait passer une pose de session pour une propriété du jeu.
import { useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";

import { ContentBrowser } from "@/components/editor/ContentBrowser";
import { AvatarPipelinePanel } from "@/components/editor/AvatarPipelinePanel";
import { MenuPipelinePanel } from "@/components/editor/MenuPipelinePanel";
import { ErrorBoundary } from "@/components/ErrorBoundary";
import {
  Viewport3D,
  type GizmoMode,
  type NodeTransform,
  type SceneNode,
  type ViewportAsset,
  type ViewportStats,
} from "@/components/editor/Viewport3D";
import { PropertyEditor } from "@/components/PropertyEditor";
import { CircleButton } from "@/components/ui/circle-button";
import { Icon } from "@/components/ui/Icon";
import { SplitPane } from "@/components/ui/split-pane";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { api, type MotionClips } from "@/lib/api";
import { useSettings } from "@/lib/settings";
import { codeOf } from "@/lib/vfsIndexDb";
import { cn } from "@/lib/utils";

/** Extensions qui ouvrent réellement quelque chose dans le viewport (cf. `assemble_glb_for_preview`
 * : l'assemblage exige le G4MD **et** le G4MG de même nom, l'un ou l'autre servant de point
 * d'entrée). Le navigateur de contenu ne présente comme ouvrable que le .g4md dont le frère
 * existe ; ce jeu-ci reste plus large pour ne pas refuser une sélection venue d'ailleurs. */
const VIEWPORT_EXTS = new Set(["g4md", "g4mg"]);
const AVATAR_SCENE_KEY = "__avatar_assemble__";

/** Les modes du gizmo, dans l'ordre de la barre d'outils. */
const GIZMO_MODES: readonly (readonly [GizmoMode, string, string])[] = [
  ["none", "near_me", "Sélectionner"],
  ["translate", "open_with", "Déplacer"],
  ["rotate", "rotate_ccw", "Pivoter"],
  ["scale", "scale", "Redimensionner"],
];

/**
 * Racines réellement employées par les trois sous-systèmes à faire converger dans l'éditeur.
 *
 * - `20_EDIT` porte les mailles et les couches de visage que `nie-model-serve` recompose en
 *   avatar ;
 * - `21_icon_avatar` porte les planches de vignettes du même atelier ;
 * - `common/menu` contient les layouts et les scripts que `nie-lua::menu_host` pilote.
 *
 * Ces raccourcis n'inventent donc pas de second catalogue UI. Le rendu de menu reste 2D et
 * l'assemblage d'avatar reste côté Rust ; l'Éditeur devient leur point d'entrée commun pour
 * inspecter les assets, les textures et les modèles sources.
 */
const ESPACES_TRAVAIL: readonly { id: string; label: string; icon: string; prefix: string; title: string }[] = [
  {
    id: "avatar-modeles",
    label: "Avatar 3D",
    icon: "person",
    prefix: "data/common/chr/_face/20_EDIT",
    title: "Pièces 3D et couches de l'avatar",
  },
  {
    id: "avatar-ui",
    label: "Avatar UI",
    icon: "grid_view",
    prefix: "data/dx11/menu/200_icon/21_icon_avatar",
    title: "Vignettes et éléments de menu de l'atelier avatar",
  },
  {
    id: "menus",
    label: "Menus",
    icon: "menu",
    prefix: "data/common/menu",
    title: "Layouts et scripts des menus du jeu",
  },
];

function extOf(path: string): string {
  const name = path.split("/").pop() ?? path;
  return name.includes(".") ? name.split(".").pop()!.toLowerCase() : "";
}

/** Un triplet lisible : les valeurs brutes de three.js ont 17 décimales. */
function vec3(v: readonly [number, number, number], factor = 1): string {
  return v.map((c) => (Math.abs(c * factor) < 1e-4 ? "0" : (c * factor).toFixed(3))).join("  ");
}

/** Tranche de clips d'animation montée d'un coup dans le volet droit. */
const CLIP_PAGE = 200;

/** Les entiers d'un DTO specta arrivent en `number | null` (ils transitent en `f64`). */
function num(v: number | null): number {
  return v ?? 0;
}

export interface EditorViewState {
  /** Dossier courant du navigateur de contenu. */
  prefix: string;
  /** Asset sélectionné (chemin VFS). */
  selected: string | null;
}

export function EditorView({
  state,
  onStateChange,
  onOpenInExplorer,
}: {
  state: EditorViewState;
  onStateChange: (s: EditorViewState) => void;
  /** Renvoie l'asset courant vers l'Explorateur (aperçu/extraction/mods). */
  onOpenInExplorer?: (path: string) => void;
}) {
  const settings = useSettings();
  /** Asset modèle principal effectivement à l'écran — distinct de `state.selected`, qui peut être
   * une texture ou une config. */
  const [primary, setPrimary] = useState<string | null>(null);
  /** Assets ajoutés à la scène par ctrl/cmd+clic, hors asset principal. */
  const [extras, setExtras] = useState<string[]>([]);
  const [glbs, setGlbs] = useState<Record<string, string>>({});
  const [avatarGlb, setAvatarGlb] = useState<string | null>(null);
  const [glbErrors, setGlbErrors] = useState<Record<string, string>>({});
  const [glbLoading, setGlbLoading] = useState(false);
  const [nodes, setNodes] = useState<SceneNode[]>([]);
  const [stats, setStats] = useState<ViewportStats>({ meshes: 0, triangles: 0, vertices: 0, materials: 0 });
  const [selectedNode, setSelectedNode] = useState<string | null>(null);
  const [transforms, setTransforms] = useState<Record<string, NodeTransform>>({});
  const [gizmoMode, setGizmoMode] = useState<GizmoMode>("none");
  const [wireframe, setWireframe] = useState(false);
  const [showGrid, setShowGrid] = useState(true);
  const [rightTab, setRightTab] = useState<"outliner" | "details" | "anims">("outliner");
  const [clips, setClips] = useState<MotionClips | null>(null);
  const [clipsFor, setClipsFor] = useState<string | null>(null);
  const [clipsError, setClipsError] = useState<string | null>(null);
  const [clipsLoading, setClipsLoading] = useState(false);
  /** Un seul `.g4pk` de personnage déclare déjà 157 clips, et il y en a des dizaines : le volet
   * n'en monte qu'une tranche dans le DOM tant que l'utilisateur n'a pas demandé le reste. */
  const [clipLimit, setClipLimit] = useState(CLIP_PAGE);

  const selectedName = state.selected?.split("/").pop() ?? "";
  const selectedCode = state.selected ? codeOf(selectedName) : "";
  const canRender = state.selected ? VIEWPORT_EXTS.has(extOf(state.selected)) : false;

  // Les assets non-modèles (texture, son, config) ne vident PAS le viewport : dans un éditeur,
  // cliquer une texture ne doit pas faire disparaître le modèle qu'on est en train de regarder.
  useEffect(() => {
    if (state.selected && canRender) setPrimary(state.selected);
  }, [state.selected, canRender]);

  const scenePaths = useMemo(() => {
    const list = primary ? [primary] : [];
    for (const p of extras) if (!list.includes(p)) list.push(p);
    if (avatarGlb) list.unshift(AVATAR_SCENE_KEY);
    return list;
  }, [primary, extras, avatarGlb]);
  const scenePathsKey = scenePaths.join("\n");

  // Le GLB d'un chemin ne change pas : on ne charge que les entrées manquantes et on oublie celles
  // qui ont quitté la scène. Recharger l'ensemble à chaque ajout coûterait un assemblage complet
  // par asset déjà à l'écran.
  const glbsRef = useRef<Record<string, string>>({});
  glbsRef.current = glbs;
  useEffect(() => {
    const wanted = new Set(scenePaths);
    const loaded = glbsRef.current;
    if (Object.keys(loaded).some((p) => !wanted.has(p))) {
      setGlbs((prev) => Object.fromEntries(Object.entries(prev).filter(([p]) => wanted.has(p))));
    }
    const missing = scenePaths.filter((p) => p !== AVATAR_SCENE_KEY && !(p in loaded));
    if (missing.length === 0) return;

    let cancelled = false;
    setGlbLoading(true);
    Promise.all(
      missing.map((path) =>
        api
          .glbBytesB64(path, settings.gameDir)
          .then((b64) => ({ path, b64, error: null as string | null }))
          .catch((e) => ({ path, b64: null as string | null, error: String(e) })),
      ),
    ).then((results) => {
      if (cancelled) return;
      setGlbs((prev) => {
        const next = { ...prev };
        for (const r of results) if (r.b64) next[r.path] = r.b64;
        return next;
      });
      setGlbErrors((prev) => {
        const next = { ...prev };
        for (const r of results) {
          if (r.error) next[r.path] = r.error;
          else delete next[r.path];
        }
        return next;
      });
      setGlbLoading(false);
    });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [scenePathsKey, settings.gameDir]);

  // Le GLB déjà assemblé par `nie-model-serve` suit le même chemin de rendu que les GLB VFS,
  // mais ne doit évidemment pas être redemandé au VFS local.
  useEffect(() => {
    setGlbs((prev) => avatarGlb ? { ...prev, [AVATAR_SCENE_KEY]: avatarGlb } : Object.fromEntries(Object.entries(prev).filter(([key]) => key !== AVATAR_SCENE_KEY)));
  }, [avatarGlb]);

  const assets = useMemo<ViewportAsset[]>(
    () => scenePaths.filter((p) => glbs[p]).map((p) => ({ key: p, glbB64: glbs[p]! })),
    [scenePaths, glbs],
  );

  // Lister les clips coûte la lecture de TOUTES les archives .g4pk du radical (des dizaines de Mo
  // décompressés) : on ne le déclenche qu'à l'ouverture de l'onglet, et une seule fois par asset.
  useEffect(() => {
    if (rightTab !== "anims" || !state.selected || clipsFor === state.selected) return;
    const path = state.selected;
    let cancelled = false;
    setClipsLoading(true);
    api
      .motionClips(path, settings.gameDir)
      .then((r) => {
        if (cancelled) return;
        setClips(r);
        setClipsError(null);
        setClipLimit(CLIP_PAGE);
      })
      .catch((e) => {
        if (cancelled) return;
        setClips(null);
        setClipsError(String(e));
      })
      .finally(() => {
        if (cancelled) return;
        setClipsFor(path);
        setClipsLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [rightTab, state.selected, clipsFor, settings.gameDir]);

  // Un nouvel asset principal = nouvelle scène : sélection de noeud et poses de session périmées.
  useEffect(() => {
    setSelectedNode(null);
    setTransforms({});
  }, [primary]);

  // Un noeud dont l'asset a été retiré de la scène ne doit pas rester sélectionné : le gizmo
  // resterait accroché à un objet libéré.
  useEffect(() => {
    if (selectedNode && !nodes.some((n) => n.id === selectedNode)) setSelectedNode(null);
  }, [nodes, selectedNode]);

  const selectedNodeInfo = useMemo(() => nodes.find((n) => n.id === selectedNode) ?? null, [nodes, selectedNode]);
  const selectedTransform = selectedNode ? transforms[selectedNode] : undefined;

  // Un asset non assemblable ne doit pas se solder par un viewport muet : on nomme le fichier et
  // la raison exacte remontée par le backend.
  const primaryError = primary ? glbErrors[primary] : undefined;
  const notice = primaryError
    ? `${primary!.split("/").pop()} n'est pas assemblable : ${primaryError}. Le viewport exige le couple .g4md + .g4mg de même nom dans le même dossier.`
    : null;

  /** Clic dans le navigateur de contenu. `additive` (ctrl/cmd) ajoute à la scène sans toucher à
   * l'asset principal — c'est lui que décrivent l'Explorateur et l'éditeur de propriétés. */
  function handleSelect(path: string, additive: boolean) {
    if (additive) {
      setExtras((prev) => (prev.includes(path) || path === primary ? prev : [...prev, path]));
      return;
    }
    setExtras([]);
    onStateChange({ ...state, selected: path });
  }

  /** Change de corpus sans conserver une scène ou une sélection de l'espace précédent. */
  function ouvrirEspace(prefix: string) {
    setPrimary(null);
    setExtras([]);
    setNodes([]);
    setStats({ meshes: 0, triangles: 0, vertices: 0, materials: 0 });
    setSelectedNode(null);
    setTransforms({});
    setGizmoMode("none");
    onStateChange({ prefix, selected: null });
  }

  return (
    <div className="flex h-full min-h-0 flex-col">
      {/* Barre d'outils */}
      <div className="flex shrink-0 items-center gap-2 border-b border-app-line px-2 py-1.5">
        <span className="min-w-0 flex-1 truncate text-xs font-medium text-ink" title={state.selected ?? undefined}>
          {state.selected ? selectedName : "Aucun asset sélectionné"}
          {extras.length > 0 && <span className="ml-2 text-tiny text-ink-faint">+{extras.length} dans la scène</span>}
          {glbLoading && <span className="ml-2 text-tiny text-ink-faint">chargement…</span>}
        </span>

        {/* Gizmo de transformation — inactif tant qu'aucun noeud n'est sélectionné, la manipulation
         * n'ayant alors aucune cible. */}
        <div className="flex shrink-0 items-center gap-1.5 border-r border-app-line pr-3">
          {GIZMO_MODES.map(([mode, icon, label]) => (
            <CircleButton
              key={mode}
              icon={icon}
              size="sm"
              variant={gizmoMode === mode ? "accent" : "default"}
              title={label}
              aria-label={label}
              disabled={mode !== "none" && !selectedNode}
              onClick={() => setGizmoMode(mode)}
            />
          ))}
        </div>

        {/* Les raccourcis sont des CORPUS de travail, pas des modes graphiques concurrents :
            chaque sélection conserve le viewport, le navigateur et l'inspecteur de l'Éditeur. */}
        <div className="flex shrink-0 items-center gap-1 border-r border-app-line pr-3" aria-label="Espaces de travail">
          {ESPACES_TRAVAIL.map((espace) => (
            <button
              key={espace.id}
              type="button"
              className={cn(
                "flex items-center gap-1 rounded px-1.5 py-1 text-tiny transition-colors",
                state.prefix === espace.prefix
                  ? "bg-accent text-white"
                  : "text-ink-dull hover:bg-app-hover hover:text-ink",
              )}
              title={espace.title}
              aria-label={espace.label}
              onClick={() => ouvrirEspace(espace.prefix)}
            >
              <Icon name={espace.icon} size={13} />
              <span>{espace.label}</span>
            </button>
          ))}
        </div>

        <div className="flex shrink-0 items-center gap-1.5">
          <CircleButton
            icon="grid_view"
            size="sm"
            variant={showGrid ? "accent" : "default"}
            title="Afficher la grille"
            aria-label="Afficher la grille"
            onClick={() => setShowGrid((v) => !v)}
          />
          <CircleButton
            icon="deployed_code"
            size="sm"
            variant={wireframe ? "accent" : "default"}
            title="Mode fil de fer"
            aria-label="Mode fil de fer"
            onClick={() => setWireframe((v) => !v)}
          />
          {/* Éditeur de scène NATIF (nie-editor) : éditeur Fyrox complet — graphe de scène,
           * inspecteur réflexif, gizmos de transformation, undo/redo — en rendu OpenGL, dans sa
           * propre fenêtre. Le viewport ci-dessous reste l'aperçu intégré ; celui-ci est
           * l'atelier. */}
          <CircleButton
            icon="wand"
            size="sm"
            title="Ouvrir dans l'éditeur de scène natif (Fyrox)"
            aria-label="Ouvrir dans l'éditeur de scène natif"
            onClick={() =>
              api
                .openInSceneEditor(state.selected, settings.gameDir)
                .then((m) => toast.success(m))
                .catch((e) => toast.error(String(e)))
            }
          />
          <CircleButton
            icon="open_in_new"
            size="sm"
            title="Ouvrir dans l'Explorateur"
            aria-label="Ouvrir dans l'Explorateur"
            disabled={!state.selected}
            onClick={() => state.selected && onOpenInExplorer?.(state.selected)}
          />
        </div>

        {/* Statistiques de scène — ce qu'affiche le coin d'un viewport d'éditeur. */}
        <div className="flex shrink-0 gap-3 border-l border-app-line pl-3 font-mono text-tiny text-ink-faint">
          <span>{stats.meshes} mesh</span>
          <span>{stats.triangles.toLocaleString("fr-FR")} tris</span>
          <span>{stats.vertices.toLocaleString("fr-FR")} verts</span>
          <span>{stats.materials} mat</span>
        </div>
      </div>
      {state.prefix === "data/common/chr/_face/20_EDIT" && (
        <AvatarPipelinePanel baseUrl={settings.modelServiceUrl} onGlb={(glb) => { setAvatarGlb(glb); setSelectedNode(null); }} />
      )}
      {state.prefix === "data/common/menu" && <MenuPipelinePanel baseUrl={settings.modelServiceUrl} />}

      {/* Corps : (viewport | panneaux droits) au-dessus du navigateur de contenu */}
      <SplitPane
        axis="y"
        side="end"
        defaultSize={220}
        min={100}
        max={600}
        storageKey="editor-content-browser"
        className="min-h-0 flex-1"
        panel={
          <ContentBrowser
            prefix={state.prefix}
            onNavigate={(prefix) => onStateChange({ ...state, prefix })}
            selected={state.selected}
            onSelect={handleSelect}
            className="h-full border-t border-app-line"
          />
        }
      >
        <SplitPane
          axis="x"
          side="end"
          defaultSize={320}
          min={240}
          max={640}
          storageKey="editor-inspector"
          className="h-full"
          panel={
            <div className="flex h-full min-h-0 flex-col border-l border-app-line bg-app-dark-box">
              <Tabs
                value={rightTab}
                onValueChange={(v) => v && setRightTab(v as "outliner" | "details" | "anims")}
              >
                <TabsList variant="line" className="px-2 pt-1.5">
                  <TabsTrigger value="outliner" className="text-xs">
                    Hiérarchie
                  </TabsTrigger>
                  <TabsTrigger value="details" className="text-xs" disabled={!selectedCode}>
                    Détails
                  </TabsTrigger>
                  <TabsTrigger value="anims" className="text-xs" disabled={!state.selected}>
                    Animations
                  </TabsTrigger>
                </TabsList>
              </Tabs>

              {rightTab === "outliner" ? (
                <div className="no-scrollbar min-h-0 flex-1 overflow-y-auto p-1.5">
                  {assets.length === 0 ? (
                    <p className="p-2 text-tiny text-ink-faint">
                      Aucune scène chargée. Sélectionnez un <code>.g4md</code> assemblable ;
                      ctrl/cmd+clic ajoute un second modèle à la scène.
                    </p>
                  ) : (
                    // Un en-tête par asset : sans lui, deux modèles aux mêmes noms de noeuds
                    // donnent une hiérarchie illisible.
                    assets.map((a) => (
                      <div key={a.key} className="mb-1.5">
                        <div className="flex items-center gap-1 rounded bg-app-box/60 px-1 py-0.5 text-tiny font-semibold text-ink">
                          <Icon name="view_in_ar" size={12} className="shrink-0 text-accent" />
                          <span className="min-w-0 flex-1 truncate" title={a.key}>
                            {a.key.split("/").pop()}
                          </span>
                          {a.key !== primary && (
                            <button
                              type="button"
                              className="shrink-0 rounded p-0.5 text-ink-faint transition-colors hover:bg-app-hover hover:text-ink"
                              title="Retirer de la scène"
                              aria-label="Retirer de la scène"
                              onClick={() => setExtras((prev) => prev.filter((p) => p !== a.key))}
                            >
                              <Icon name="close" size={11} />
                            </button>
                          )}
                        </div>
                        {nodes
                          .filter((n) => n.assetKey === a.key)
                          .map((n) => (
                            <button
                              key={n.id}
                              type="button"
                              onClick={() => setSelectedNode(n.id === selectedNode ? null : n.id)}
                              className={cn(
                                "flex w-full items-center gap-1.5 rounded px-1 py-0.5 text-left text-tiny transition-colors",
                                n.id === selectedNode
                                  ? "bg-accent text-white"
                                  : "text-ink-dull hover:bg-app-hover hover:text-ink",
                              )}
                              style={{ paddingLeft: 4 + n.depth * 12 }}
                              title={`${n.type}${n.triangles ? ` · ${n.triangles} triangles` : ""}`}
                            >
                              <Icon name={n.triangles > 0 ? "view_in_ar" : "account_tree"} size={12} />
                              <span className="min-w-0 flex-1 truncate">{n.name}</span>
                              {n.triangles > 0 && (
                                <span className="shrink-0 font-mono opacity-60">{n.triangles}</span>
                              )}
                            </button>
                          ))}
                      </div>
                    ))
                  )}

                  {/* Carte du NOEUD DE SCÈNE : sa transformation appartient à la session, pas aux
                   * données du jeu — l'onglet Détails, lui, décrit l'asset par son code. */}
                  {selectedNodeInfo && (
                    <div className="mt-2 rounded border border-app-line bg-app-box p-2 text-tiny text-ink-dull">
                      <p className="font-semibold text-ink">{selectedNodeInfo.name}</p>
                      <p>type : {selectedNodeInfo.type}</p>
                      {selectedNodeInfo.triangles > 0 && (
                        <p>{selectedNodeInfo.triangles.toLocaleString("fr-FR")} triangles</p>
                      )}
                      {selectedTransform && (
                        <dl className="mt-1.5 space-y-0.5 border-t border-app-line pt-1.5 font-mono">
                          <div className="flex gap-2">
                            <dt className="w-14 shrink-0 text-ink-faint">position</dt>
                            <dd className="min-w-0 truncate">{vec3(selectedTransform.position)}</dd>
                          </div>
                          <div className="flex gap-2">
                            <dt className="w-14 shrink-0 text-ink-faint">rotation</dt>
                            <dd className="min-w-0 truncate">{vec3(selectedTransform.rotation, 180 / Math.PI)}°</dd>
                          </div>
                          <div className="flex gap-2">
                            <dt className="w-14 shrink-0 text-ink-faint">échelle</dt>
                            <dd className="min-w-0 truncate">{vec3(selectedTransform.scale)}</dd>
                          </div>
                        </dl>
                      )}
                      <p className="mt-1.5 text-ink-faint">
                        Transformation locale à la session : le dépôt ne sait pas réécrire une
                        géométrie G4MG/G4MD, rien n'est enregistré.
                      </p>
                    </div>
                  )}
                </div>
              ) : rightTab === "anims" ? (
                /* LECTURE SEULE, délibérément : aucun bouton lecture/pause, aucune barre de
                 * transport. Le GLB servi au viewport n'a ni `skins` ni `animations` (cf.
                 * `nie_formats::assemble`), donc rien ici ne peut être rejoué — offrir les
                 * commandes d'un lecteur promettrait une fonction inexistante. */
                <div className="no-scrollbar min-h-0 flex-1 overflow-y-auto p-2 text-tiny">
                  <p className="mb-2 text-ink-faint">
                    Clips <span className="font-semibold text-ink-dull">déclarés</span> par les
                    fichiers de mouvement <code>.g4mt</code> contenus dans les archives{" "}
                    <code>.g4pk</code> de même radical. Liste seule : le modèle affiché ne porte ni
                    squelette ni animation, aucune lecture n&apos;est possible ici.
                  </p>

                  {clipsLoading && <p className="text-ink-faint">lecture des archives…</p>}
                  {clipsError && <p className="text-error">{clipsError}</p>}

                  {!clipsLoading && clips && (
                    <>
                      <p className="mb-1.5 font-mono text-ink-faint">
                        {clips.clips.length} clip(s) · {clips.archives.length} archive(s)
                      </p>
                      {clips.notice && (
                        <p className="mb-1.5 rounded border border-app-line bg-app-box p-1.5 text-ink-dull">
                          {clips.notice}
                        </p>
                      )}
                      <ul className="space-y-1">
                        {clips.clips.slice(0, clipLimit).map((c, i) => (
                          <li
                            key={`${c.archive}#${c.motion_file}#${i}`}
                            className="rounded border border-app-line bg-app-box px-1.5 py-1"
                          >
                            <div className="flex items-center gap-1.5">
                              <Icon name="animation" size={12} className="shrink-0 text-accent" />
                              <span
                                className="min-w-0 flex-1 truncate font-semibold text-ink"
                                title={c.name}
                              >
                                {c.name || "(clip sans nom)"}
                              </span>
                              {c.additive && (
                                <span
                                  className="shrink-0 rounded bg-app-hover px-1 text-ink-faint"
                                  title="Clip additif : superposé à une pose de base"
                                >
                                  additif
                                </span>
                              )}
                            </div>
                            <div className="font-mono text-ink-faint">
                              frames {num(c.start_frame)}→{num(c.end_frame)} ({num(c.frame_count)}){" "}
                              · {num(c.fps)} fps · {num(c.target_count)} cibles
                            </div>
                            <div
                              className="truncate text-ink-faint"
                              title={`${c.archive} → ${c.motion_file}`}
                            >
                              {c.motion_file}
                            </div>
                          </li>
                        ))}
                      </ul>
                      {clips.clips.length > clipLimit && (
                        <button
                          type="button"
                          className="mt-1.5 w-full rounded border border-app-line px-1.5 py-1 text-ink-dull transition-colors hover:bg-app-hover hover:text-ink"
                          onClick={() => setClipLimit((n) => n + CLIP_PAGE)}
                        >
                          Afficher {Math.min(CLIP_PAGE, clips.clips.length - clipLimit)} clips de
                          plus ({clips.clips.length - clipLimit} restants)
                        </button>
                      )}
                    </>
                  )}
                </div>
              ) : (
                selectedCode && (
                  <PropertyEditor
                    code={selectedCode}
                    className="min-h-0 flex-1 p-2"
                    onOpenFile={(p) => onStateChange({ ...state, selected: p })}
                  />
                )
              )}
            </div>
          }
        >
          {/* Le viewport est la zone la plus exposée de l'application : trois moteurs (WebGL,
            * three.js, le GLB du jeu) dont aucun n'est sous notre contrôle. Sa barrière propre
            * garde la panne DANS le viewport — hiérarchie, détails et navigateur de contenu
            * continuent de servir. `resetKeys` : changer d'asset réarme tout seul. */}
          <ErrorBoundary zone="Aperçu 3D" resetKeys={[scenePathsKey]}>
            <Viewport3D
              assets={assets}
              selectedId={selectedNode}
              onSelect={setSelectedNode}
              onSceneLoaded={(n, s) => {
                setNodes(n);
                setStats(s);
              }}
              onTransform={(id, trs) => setTransforms((prev) => ({ ...prev, [id]: trs }))}
              gizmoMode={gizmoMode}
              notice={notice}
              wireframe={wireframe}
              showGrid={showGrid}
              className="h-full w-full bg-app-darker-box"
            />
          </ErrorBoundary>
        </SplitPane>
      </SplitPane>
    </div>
  );
}
