import { useEffect, useMemo, useRef, useState } from "react";
import { writeText, readText } from "@tauri-apps/plugin-clipboard-manager";
import { confirm } from "@tauri-apps/plugin-dialog";
import { toast } from "sonner";
import { api, type FolderRole, type RawCpkEntry, type VfsDir } from "@/lib/api";
import { useSettings } from "@/lib/settings";
import { humanSize } from "@/lib/bytes";
import { recordVisit, togglePin, usePinnedPlaces } from "@/lib/places";
import { codeOf } from "@/lib/vfsIndexDb";
import { useThumbnail } from "@/lib/thumbs";
import { useResolvedNames } from "@/lib/nameResolve";
import {
  showExportSelectionMenu,
  showRawCpkFileContextMenu,
  showVfsFileContextMenu,
  showVfsFolderContextMenu,
} from "@/lib/contextMenu";
import { registerFileOps } from "@/lib/editBus";
import { SplitPane } from "@/components/ui/split-pane";
import type { ExplorerTab, ExplorerTabPatch } from "@/lib/explorerTabs";
import { modsDb } from "@/lib/modsDb";
import { stageReplacement, stageReplacementFromPath } from "@/lib/modWorkspace";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Icon } from "@/components/ui/Icon";
import { CircleButton } from "@/components/ui/circle-button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { Slider } from "@/components/ui/slider";
import { useT } from "@/lib/i18n";
import { DetailPane, type DetailTarget } from "@/components/DetailPane";
import { PropertyEditor } from "@/components/PropertyEditor";
import { SelectionBar } from "@/components/SelectionBar";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";

type SortKey = "name" | "size";

/** Ligne de fichier affichée — VFS normal, ou entrée d'un `.cpk` ouvert hors VFS (`entryIndex`
 * présent) quand la navigation est descendue dans `data/packs/*.cpk` (vue fusionnée, cf. demande
 * utilisatrice « il faut fusionner le vfs viewer et raw packs cpk viewer »). */
interface Row {
  path: string;
  name: string;
  size: number;
  entryIndex?: number;
}

/** Détecte si `prefix` est descendu à l'intérieur d'un `.cpk` (tout segment se terminant par
 * `.cpk`) — au-delà de ce segment, la navigation ne porte plus sur le VFS mais sur les entrées
 * RÉELLES du fichier `.cpk` physique, lues directement via `CpkReader` (cf. `open_raw_cpk`). */
function detectCpkBoundary(prefix: string): { cpkVfsPrefix: string; inner: string } | null {
  const segs = prefix.split("/").filter(Boolean);
  const idx = segs.findIndex((s) => s.toLowerCase().endsWith(".cpk"));
  if (idx === -1) return null;
  return { cpkVfsPrefix: segs.slice(0, idx + 1).join("/"), inner: segs.slice(idx + 1).join("/") };
}

/**
 * `.cpk` actuellement ouvert CÔTÉ BACKEND. Volontairement module-level, donc PARTAGÉ par toutes
 * les instances d'`ExplorerView` : le backend ne garde qu'un seul lecteur `.cpk` à la fois
 * (`RawCpkState` dans `src-tauri/src/lib.rs`, écrasé par chaque `open_raw_cpk`) et toutes les
 * commandes `raw_cpk_*` ne prennent qu'un INDEX d'entrée, sans dire de quel fichier. Avec un
 * témoin par onglet, deux onglets descendus dans deux `.cpk` différents extrairaient chacun
 * l'entrée n° i du `.cpk` de l'autre.
 *
 * Limite assumée, non contournable côté UI : la table des matières reste mise en cache par onglet
 * (affichage), mais la POIGNÉE backend est unique — un onglet redevenu actif dans un autre `.cpk`
 * doit donc réémettre `open_raw_cpk` avant toute extraction. Une vraie correction demanderait un
 * lecteur `.cpk` par handle côté Rust, hors périmètre ici.
 */
let openedCpkPrefix: string | null = null;

/** Fichiers montés d'un coup dans la liste/grille (cf. `visibles`). */
const PAGE_FICHIERS = 300;

/** Correspondances ramenées par page de recherche — le TOTAL est rendu à part (`FindPage.total`). */
const PAGE_RECHERCHE = 500;

/** Vignette lazy de la vue grille — cf. demande utilisatrice « compare l'UI de nie-explorer et
 * azalee cpk explorer et fusionne le meilleur des deux » : la vue grille + vignettes est la
 * différenciation la plus marquante d'azalee `/cpk` (`CpkModelThumb`), portée ici pour les
 * `.g4tx`. Les vignettes 3D (`.g4md` : assemblage GLB + rendu) restent volontairement hors
 * portée — ouvrir le fichier reste le chemin de l'aperçu 3D, pas de faux raccourci.
 *
 * Chargement différé, cache borné, file de décodage et **résolution réduite** vivent dans
 * `lib/thumbs` : la grille de l'éditeur s'en sert à l'identique, et c'est là qu'est documenté
 * pourquoi la pleine résolution saturait la mémoire du processus de rendu. */
function FileThumbnail({ path, ext, gameDir }: { path: string; ext: string; gameDir?: string }) {
  const { ref, src, supporte } = useThumbnail(path, ext, gameDir);

  if (!supporte) {
    return <Icon name="description" size={28} className="text-on-surface-variant" />;
  }
  return (
    <div
      ref={ref}
      className="flex h-full w-full items-center justify-center overflow-hidden rounded-lg bg-surface-container-highest"
    >
      {src ? (
        // biome-ignore lint: aperçu local, pas d'optimisation next/image (app Tauri, pas Next)
        <img src={src} alt="" className="h-full w-full object-contain" />
      ) : (
        <Icon name="image" size={24} className="text-on-surface-variant/50" />
      )}
    </div>
  );
}

// La barre latérale « emplacements » (épingles curées + épinglés utilisatrice + récents) vivait
// ICI, en plus de la rangée d'onglets globale : deux navigations concurrentes à l'écran. Elle est
// désormais servie par la barre latérale UNIQUE de l'app (`components/Sidebar.tsx`, alimentée par
// `App.tsx`), exactement comme `SpacesSidebar` de spacedrive qui porte à la fois les vues et les
// emplacements. `lib/places.ts` est inchangé — seul l'endroit du rendu a bougé.

export function ExplorerView({
  state,
  onStateChange,
  active,
  onOpenInNewTab,
  onBack,
  onForward,
  canGoBack = false,
  canGoForward = false,
}: {
  state: ExplorerTab;
  /** Applique un PATCH à l'onglet — `id`/historique restent la propriété du store. */
  onStateChange: (patch: ExplorerTabPatch) => void;
  /** Vrai pour l'unique instance visible. Plusieurs `ExplorerView` sont montées en permanence
   * (une par onglet) : tout ce qui est GLOBAL au process — `editBus`, raccourcis `window` — doit
   * rester derrière cette garde, sinon N instances s'enregistrent et se marchent dessus. */
  active: boolean;
  onOpenInNewTab?: (prefix: string) => void;
  onBack?: () => void;
  onForward?: () => void;
  canGoBack?: boolean;
  canGoForward?: boolean;
}) {
  const settings = useSettings();
  const t = useT();
  // Un sous-dossier porte désormais SON COMPTE de fichiers (`vfs_ls` le rend gratuitement, du
  // même balayage) : l'interface distingue un dossier de 12 560 textures d'un dossier vide sans
  // avoir à y descendre.
  const [dirs, setDirs] = useState<VfsDir[]>([]);
  const [files, setFiles] = useState<Row[]>([]);
  /** Correspondances TOTALES de la recherche courante, avant troncature à `PAGE_RECHERCHE`. */
  const [searchTotal, setSearchTotal] = useState(0);
  const [role, setRole] = useState<FolderRole | null>(null);
  // Cache du `.cpk` brut actuellement ouvert (vue fusionnée VFS/CPK) — évite de relire tout le
  // fichier à chaque sous-dossier visité À L'INTÉRIEUR du même `.cpk` (`open_raw_cpk` relit le
  // fichier entier + reparse la table des matières à chaque appel côté Rust).
  const [cpkEntries, setCpkEntries] = useState<RawCpkEntry[]>([]);
  /** `.cpk` dont CET onglet détient la table des matières en cache — distinct du témoin backend
   * `openedCpkPrefix` (module-level), qui ne dit que ce que le lecteur Rust a ouvert en DERNIER. */
  const cpkCachedPrefix = useRef<string | null>(null);
  const cpkBoundary = useMemo(() => detectCpkBoundary(state.prefix), [state.prefix]);
  const [query, setQuery] = useState(state.query ?? "");

  // Requête poussée depuis l'extérieur (palette de commandes Ctrl+K) — l'instance de cet onglet
  // reste montée en permanence (`keepMounted` sur le panneau + `display:none` sur les onglets
  // inactifs), donc un simple état initial ne suffit pas : il faut resynchroniser à chaque
  // nouvelle valeur de `state.query`.
  useEffect(() => {
    if (state.query !== undefined) setQuery(state.query);
  }, [state.query]);
  const [ext, setExt] = useState(state.ext ?? "");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [sortKey, setSortKey] = useState<SortKey>(state.sortKey ?? "name");
  // Vue liste (défaut, dense — navigation clavier/multi-sélection) ou grille (vignettes, façon
  // azalee `/cpk` — cf. demande utilisatrice de fusion des deux UI). Le choix appartient à
  // l'ONGLET (remonté au store, donc restauré au prochain lancement), pas à l'application : la
  // vue grille sert un contexte précis (« ce dossier-là est un dossier de textures »).
  const [viewMode, setViewMode] = useState<"list" | "grid">(state.viewMode ?? "list");
  // Taille des vignettes en vue grille (px, réglable via le popover « Options d'affichage » —
  // pattern porté de spacedrive : un « View Options » à côté du sélecteur liste/grille, pas un
  // réglage caché dans les Paramètres généraux).
  const [gridSize, setGridSize] = useState(state.gridSize ?? 96);
  const pins = usePinnedPlaces();
  // Multi-sélection RÉELLE (Ctrl/Shift-clic, comme l'explorateur Windows) — cf. demande
  // utilisatrice « editer doit vraiment copier coller et tout select les fichiers dossiers pas
  // du texte » : les `PredefinedMenuItem` Copy/SelectAll de Tauri sont des commandes texte de
  // l'OS, sans effet sur une liste HTML custom. `state.selected` reste la cible d'aperçu
  // (`DetailPane`, le dernier élément touché) ; `multiSelected` porte le surlignage + les
  // opérations groupées (Ctrl+A/Ctrl+C, menu Édition natif).
  const [multiSelected, setMultiSelected] = useState<Set<string>>(new Set());
  // Ancre de la sélection par plage (Shift-clic) sur les DOSSIERS, séparée de `state.selected` :
  // contrairement aux fichiers, un clic simple sur un dossier NAVIGUE (`goto`) plutôt que de le
  // "sélectionner" pour l'aperçu — piggy-backer sur `state.selected` ferait croire à `DetailPane`
  // qu'un dossier est un fichier VFS ciblé (échec d'aperçu silencieux mais trompeur).
  const [folderAnchor, setFolderAnchor] = useState<string | null>(null);
  /** Onglet de l'inspecteur de droite — aperçu du fichier, ou éditeur de propriétés de l'entité. */
  const [inspectorTab, setInspectorTab] = useState<"preview" | "properties">("preview");
  /** Jeton de la dernière requête de listage/recherche lancée — cf. `fresh()` dans l'effet de
   * chargement (anti-race, §2.7 roadmap). `useRef` et pas `useState` : le changer ne doit PAS
   * provoquer de rendu, et sa valeur doit être lisible immédiatement dans le même tour. */
  const requestSeq = useRef(0);

  // Recherche VFS globale non disponible À L'INTÉRIEUR d'un `.cpk` ouvert (portée volontairement
  // limitée pour cette fusion — la recherche continue de fonctionner normalement partout ailleurs).
  const searching = query.trim().length > 0 && !cpkBoundary;

  useEffect(() => {
    setLoading(true);
    setError(null);
    // Garde anti-réponse-périmée (§2.7 roadmap : « recherche avec anti-race explicite », relevé
    // comme écart réel non fermé). Sans elle, une frappe rapide peut faire arriver la réponse de
    // "c010" APRÈS celle de "c0100" et réafficher la liste précédente : le champ affiche une
    // requête, la liste en montre une autre. Chaque exécution incrémente le jeton ; toute réponse
    // dont le jeton n'est plus le courant est jetée.
    const seq = ++requestSeq.current;
    const fresh = () => seq === requestSeq.current;

    if (cpkBoundary) {
      // Vue fusionnée : à l'intérieur d'un `.cpk`, on liste ses VRAIES entrées (pas le VFS) —
      // cf. demande utilisatrice « il faut fusionner le vfs viewer et raw packs cpk viewer ».
      (async () => {
        let entries = cpkEntries;
        if (cpkCachedPrefix.current !== cpkBoundary.cpkVfsPrefix) {
          // Revendiqué AVANT le premier `await` : l'effet de resynchronisation ci-dessous tourne
          // dans le même commit et ne doit pas rouvrir le même fichier une seconde fois.
          cpkCachedPrefix.current = cpkBoundary.cpkVfsPrefix;
          openedCpkPrefix = cpkBoundary.cpkVfsPrefix;
          const gameDir = settings.gameDir || (await api.defaultGameDir());
          const absPath = `${gameDir.replace(/[\\/]+$/, "")}/${cpkBoundary.cpkVfsPrefix}`;
          entries = await api.rawCpkOpen(absPath);
          setCpkEntries(entries);
        }
        const inner = cpkBoundary.inner;
        // Compte par sous-dossier, comme le fait `vfs_ls` côté VFS : la table des matières du CPK
        // est déjà parcourue en entier ici, le compte ne coûte rien de plus.
        const dirCounts = new Map<string, number>();
        const fileRows: Row[] = [];
        for (const e of entries) {
          let rest: string;
          if (inner === "") rest = e.path;
          else if (e.path === inner) continue;
          else if (e.path.startsWith(`${inner}/`)) rest = e.path.slice(inner.length + 1);
          else continue;
          const slash = rest.indexOf("/");
          if (slash === -1) {
            fileRows.push({ path: `${state.prefix}/${rest}`, name: rest, size: e.size, entryIndex: e.index });
          } else {
            const seg = rest.slice(0, slash);
            dirCounts.set(seg, (dirCounts.get(seg) ?? 0) + 1);
          }
        }
        if (!fresh()) return;
        setDirs([...dirCounts].map(([name, count]) => ({ name, count })).sort((a, b) => a.name.localeCompare(b.name)));
        setFiles(fileRows);
        setRole(null);
      })()
        .catch((e) => {
          // L'ouverture a échoué : la revendication faite plus haut serait un mensonge (ni cache
          // local, ni lecteur backend valide) — on la relâche pour qu'un nouvel essai reparte.
          cpkCachedPrefix.current = null;
          if (openedCpkPrefix === cpkBoundary.cpkVfsPrefix) openedCpkPrefix = null;
          if (fresh()) setError(String(e));
        })
        .finally(() => fresh() && setLoading(false));
      return;
    }

    if (state.prefix === "data/packs") {
      // `data/packs` : le VFS n'expose JAMAIS les conteneurs `.cpk` eux-mêmes comme entrées
      // navigables (seuls les chemins internes du jeu le sont) — pont vers les VRAIS fichiers
      // physiques, chacun cliquable comme un dossier pour descendre dedans (cf. `cpkBoundary`
      // ci-dessus). Cf. demande utilisatrice « data\packs n'est pas préchargé ».
      api
        .listPacksDir(settings.gameDir)
        .then((packs) => {
          if (!fresh()) return;
          // `data/packs` est une vue de fichiers PHYSIQUES, pas un dossier du VFS : aucun compte
          // d'entrées internes n'est connu ici sans ouvrir chaque `.cpk`. `0` = « non compté »,
          // et l'affichage ne montre alors rien plutôt qu'un « 0 fichier » faux.
          setDirs(packs.map((p) => ({ name: p.name, count: 0 })));
          setFiles([]);
          setRole(null);
        })
        .catch((e) => fresh() && setError(String(e)))
        .finally(() => fresh() && setLoading(false));
      return;
    }

    const req = searching
      ? // Paginée : la page de 500 s'accompagne enfin de son dénominateur, sinon « 500 trouvés »
        // et « 500 existants » s'écrivent pareil et l'utilisatrice ne sait pas qu'elle en rate.
        api.findPaged(query.trim(), ext.trim() || undefined, PAGE_RECHERCHE, 0, settings.gameDir).then((page) => {
          if (!fresh()) return;
          setDirs([]);
          setFiles(page.files);
          setSearchTotal(page.total);
          setRole(null);
        })
      : api.ls(state.prefix, settings.gameDir).then((r) => {
          if (!fresh()) return;
          setDirs(r.dirs);
          setFiles(r.files);
          setSearchTotal(0);
          setRole(r.role);
        });
    req.catch((e) => fresh() && setError(String(e))).finally(() => fresh() && setLoading(false));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [state.prefix, query, ext, settings.gameDir, cpkBoundary]);

  // Resynchronisation du lecteur `.cpk` backend quand CET onglet (re)devient actif : le témoin
  // `openedCpkPrefix` est partagé, un autre onglet a pu ouvrir un autre `.cpk` entre-temps et les
  // extractions par index viseraient alors le mauvais fichier. Séparé de l'effet de listage :
  // redevenir actif ne doit PAS relister tout le dossier.
  useEffect(() => {
    if (!active || !cpkBoundary) return;
    const wanted = cpkBoundary.cpkVfsPrefix;
    if (openedCpkPrefix === wanted) return;
    openedCpkPrefix = wanted; // revendiqué avant l'await, cf. effet de listage
    (async () => {
      const gameDir = settings.gameDir || (await api.defaultGameDir());
      await api.rawCpkOpen(`${gameDir.replace(/[\\/]+$/, "")}/${wanted}`);
    })().catch((e) => {
      if (openedCpkPrefix === wanted) openedCpkPrefix = null;
      setError(String(e));
    });
  }, [active, cpkBoundary, settings.gameDir]);

  const sortedDirs = useMemo(() => [...dirs].sort((a, b) => a.name.localeCompare(b.name)), [dirs]);
  // Chemins pleins des dossiers, dans l'ordre affiché — sert à la fois à Ctrl+A (`doSelectAll`)
  // et à la sélection par plage Shift-clic sur les dossiers (même mécanique que `sortedFiles`).
  const dirPaths = useMemo(
    () => (searching ? [] : sortedDirs.map((d) => (state.prefix ? `${state.prefix}/${d.name}` : d.name))),
    [searching, sortedDirs, state.prefix],
  );
  const sortedFiles = useMemo(() => {
    const arr = [...files];
    arr.sort((a, b) => (sortKey === "size" ? b.size - a.size : a.name.localeCompare(b.name)));
    return arr;
  }, [files, sortKey]);

  // Le VFS a des dossiers de plus de 12 000 fichiers (`.../10_icon_chr/uniform` : 12 560 `.g4tx`).
  // Chaque entrée est un bouton riche — et, pour une texture, un observateur d'intersection : les
  // monter tous d'un coup coûte cher avant même qu'une seule vignette ne soit décodée. On en monte
  // une tranche, le reste à la demande.
  //
  // La SÉLECTION, elle, continue de porter sur `sortedFiles` en entier : Ctrl+A sélectionne tout
  // le dossier, pas seulement ce qui est monté — un plafond d'affichage ne doit pas devenir un
  // plafond d'action.
  const [visibles, setVisibles] = useState(PAGE_FICHIERS);
  useEffect(() => setVisibles(PAGE_FICHIERS), [state.prefix, files, sortKey]);

  /** Entrée sur laquelle porte le clavier, DISTINCTE de `state.selected`.
   *
   * `state.selected` pilote l'inspecteur et ne vaut que pour un fichier. Les flèches s'appuyaient
   * sur lui seul et n'assignaient que des fichiers : le curseur ne pouvait donc pas franchir les
   * dossiers, qui sont listés en tête — dans un dossier qui en contient, la navigation au clavier
   * ne démarrait même pas. Le curseur, lui, passe partout ; il ne met à jour l'aperçu que
   * lorsqu'il tombe sur un fichier. */
  const [curseur, setCurseur] = useState<string | null>(null);
  useEffect(() => setCurseur(null), [state.prefix]);
  /** Conteneur de la liste — sert à ramener le curseur dans la zone visible. Une recherche par
   * `document.querySelector` serait fausse : plusieurs onglets restent montés (`keepMounted`) et
   * portent les mêmes chemins. */
  const listeRef = useRef<HTMLDivElement | null>(null);

  /** Ramène l'entrée dans la zone visible, sans la centrer — comportement d'un explorateur de
   * fichiers, où la liste ne bouge que si le curseur en sort. */
  function faireDefilerVers(path: string) {
    requestAnimationFrame(() => {
      listeRef.current
        ?.querySelector(`[data-path="${CSS.escape(path)}"]`)
        ?.scrollIntoView({ block: "nearest" });
    });
  }
  const affiches = useMemo(() => sortedFiles.slice(0, visibles), [sortedFiles, visibles]);

  // Nom réel (perso/technique/objet) lié à chaque fichier, résolu par lot via le miroir wiki
  // local — cf. demande utilisatrice « affiche le nom... lié à un fichier au lieu de juste l'id ».
  const fileCodes = useMemo(() => sortedFiles.map((f) => codeOf(f.name)), [sortedFiles]);

  // Taille totale de la sélection courante (fichiers uniquement — un dossier VFS n'a pas de
  // taille propre) — affichée dans la barre de statut, cf. rendu plus bas.
  const selectedTotalSize = useMemo(
    () => sortedFiles.reduce((sum, f) => (multiSelected.has(f.path) ? sum + f.size : sum), 0),
    [sortedFiles, multiSelected],
  );
  const resolved = useResolvedNames(settings.wikiDb, fileCodes);

  // Liste plate dossiers+fichiers pour la navigation clavier (haut/bas/entrée/retour, à la yazi).
  const flatEntries = useMemo(
    () => [
      ...sortedDirs.map((d) => ({ kind: "dir" as const, path: state.prefix ? `${state.prefix}/${d.name}` : d.name })),
      ...sortedFiles.map((f) => ({ kind: "file" as const, path: f.path })),
    ],
    [sortedDirs, sortedFiles, state.prefix],
  );

  const segments = state.prefix ? state.prefix.split("/") : [];

  /** Clic sur un dossier — extrait pour être partagé entre la vue liste et la vue grille (même
   * sémantique Ctrl/Shift-clic dans les deux, cf. `viewMode`). */
  function handleDirClick(path: string, e: React.MouseEvent) {
    if (e.ctrlKey || e.metaKey) {
      setMultiSelected((prev) => {
        const next = new Set(prev);
        if (next.has(path)) next.delete(path);
        else next.add(path);
        return next;
      });
      setFolderAnchor(path);
    } else if (e.shiftKey && folderAnchor) {
      const idxA = dirPaths.indexOf(folderAnchor);
      const idxB = dirPaths.indexOf(path);
      if (idxA !== -1 && idxB !== -1) {
        const [lo, hi] = idxA < idxB ? [idxA, idxB] : [idxB, idxA];
        setMultiSelected(new Set(dirPaths.slice(lo, hi + 1)));
      } else {
        setMultiSelected(new Set([path]));
      }
      setFolderAnchor(path);
    } else {
      goto(path);
    }
  }

  /** Clic MILIEU sur un dossier = « ouvrir dans un nouvel onglet », convention de navigateur.
   * Ctrl+clic est déjà pris (multi-sélection), et React ne remonte pas le bouton du milieu dans
   * `onClick` : sans `onAuxClick`, ce geste n'existerait tout simplement pas. */
  function handleDirAuxClick(path: string, e: React.MouseEvent) {
    if (e.button !== 1 || !onOpenInNewTab) return;
    e.preventDefault();
    onOpenInNewTab(path);
  }

  /** Menu contextuel d'un dossier — partagé par la vue liste et la vue grille. */
  function showFolderMenu(path: string, e: React.MouseEvent) {
    e.preventDefault();
    showVfsFolderContextMenu({
      path,
      onOpen: () => goto(path),
      ...(onOpenInNewTab ? { onOpenInNewTab: () => onOpenInNewTab(path) } : {}),
    });
  }

  /** Clic sur un fichier — extrait pour être partagé entre la vue liste et la vue grille. */
  function handleFileClick(f: Row, e: React.MouseEvent) {
    if (e.ctrlKey || e.metaKey) {
      setMultiSelected((prev) => {
        const next = new Set(prev);
        if (next.has(f.path)) next.delete(f.path);
        else next.add(f.path);
        return next;
      });
      onStateChange({ selected: f.path });
    } else if (e.shiftKey && state.selected) {
      const idxA = sortedFiles.findIndex((x) => x.path === state.selected);
      const idxB = sortedFiles.findIndex((x) => x.path === f.path);
      if (idxA !== -1 && idxB !== -1) {
        const [lo, hi] = idxA < idxB ? [idxA, idxB] : [idxB, idxA];
        setMultiSelected(new Set(sortedFiles.slice(lo, hi + 1).map((x) => x.path)));
      }
      onStateChange({ selected: f.path });
    } else {
      setMultiSelected(new Set());
      onStateChange({ selected: f.path });
    }
  }

  /** « Ajouter à un mod… » du menu contextuel. `showVfsFileContextMenu` sait afficher cette entrée
   * depuis toujours (`onStageIntoMod`), mais l'Explorateur ne la lui passait JAMAIS : le menu
   * contextuel n'a donc jamais montré l'action, alors que c'est le geste principal de l'app. Le
   * mod est créé à la volée s'il n'en existe aucun (même règle que Ctrl+V, cf. `doPaste`). */
  async function stageIntoMod(path: string) {
    try {
      const mods = await modsDb.listMods();
      const modId = mods[0]?.id ?? (await modsDb.createMod("Mon mod", "Créé depuis le menu contextuel"));
      const modName = mods[0]?.name ?? "Mon mod";
      const ok = await stageReplacement(modId, { kind: "vfs", path }, settings.gameDir);
      if (ok) toast.success(`Ajouté au mod « ${modName} »`, { description: path });
    } catch (e) {
      toast.error(String(e));
    }
  }

  /** « Ajouter à un mod… » en LOT, depuis la barre de multi-sélection. Les dossiers cochés sont
   * ignorés : le VFS n'a pas de remplacement récursif, seul un fichier se substitue à un fichier.
   *
   * `stageReplacement` demande le fichier de remplacement par un sélecteur NATIF, un par cible :
   * enchaîner N dialogues sans prévenir serait une embuscade, d'où la confirmation préalable et
   * l'arrêt net au premier dialogue annulé. */
  async function stageSelectionIntoMod() {
    const paths = [...multiSelected].filter((p) => sortedFiles.some((f) => f.path === p));
    if (paths.length === 0) {
      toast.error("Aucun fichier dans la sélection — un dossier ne se remplace pas");
      return;
    }
    if (paths.length > 1) {
      const go = await confirm(
        `Un sélecteur de fichier s'ouvrira pour chacun des ${paths.length} fichiers sélectionnés. Annuler l'un d'eux arrête l'opération.`,
        { title: "Ajouter la sélection à un mod", kind: "warning" },
      );
      if (!go) return;
    }
    try {
      const mods = await modsDb.listMods();
      const modId = mods[0]?.id ?? (await modsDb.createMod("Mon mod", "Créé depuis la barre de sélection"));
      const modName = mods[0]?.name ?? "Mon mod";
      let staged = 0;
      for (const path of paths) {
        if (!(await stageReplacement(modId, { kind: "vfs", path }, settings.gameDir))) break;
        staged += 1;
      }
      if (staged > 0) toast.success(`${staged} fichier(s) ajouté(s) au mod « ${modName} »`);
    } catch (e) {
      toast.error(String(e));
    }
  }

  /** Menu contextuel d'un fichier — même dispatch VFS/CPK-brut que la vue liste. */
  function handleFileContextMenu(f: Row, e: React.MouseEvent) {
    e.preventDefault();
    if (f.entryIndex !== undefined) {
      showRawCpkFileContextMenu({
        path: f.path,
        name: f.name,
        size: f.size,
        entryIndex: f.entryIndex,
        onOpen: () => onStateChange({ selected: f.path }),
      });
    } else {
      showVfsFileContextMenu({
        path: f.path,
        name: f.name,
        size: f.size,
        gameDir: settings.gameDir,
        blenderExe: settings.blenderExe,
        onOpen: () => onStateChange({ selected: f.path }),
        onStageIntoMod: () => void stageIntoMod(f.path),
      });
    }
  }

  function goto(prefix: string) {
    recordVisit(prefix);
    setMultiSelected(new Set());
    setFolderAnchor(null);
    onStateChange({ prefix, selected: null });
  }

  /** Ctrl+A — sélectionne TOUT le dossier/résultat courant (dossiers + fichiers), comme
   * l'explorateur Windows, pas juste le texte visible. */
  function doSelectAll() {
    const filePaths = sortedFiles.map((f) => f.path);
    const all = [...dirPaths, ...filePaths];
    if (all.length === 0) return;
    setMultiSelected(new Set(all));
    toast.success(`${all.length.toLocaleString("fr-FR")} élément(s) sélectionné(s)`);
  }

  /** Ctrl+C — copie les chemins de la sélection (multi ou simple) dans le presse-papiers, en
   * TEXTE — délibérément PAS `api.clipboardWriteFileList` (CF_HDROP réel, cf. `doPaste`) : un
   * chemin VFS (`data/common/chr/...`) est VIRTUEL, à l'intérieur d'un CPK — aucun fichier
   * n'existe à cet emplacement sur le vrai disque, donc le poser en CF_HDROP tromperait
   * l'Explorateur Windows (il tenterait d'ouvrir un chemin qui n'existe nulle part) plutôt que de
   * l'aider. Le texte reste la représentation correcte ici. */
  /** Export en lot de la sélection, au format choisi dans un menu natif (cf.
   * `showExportSelectionMenu`). Seuls les FICHIERS sont concernés : un dossier VFS n'a pas
   * d'octets à convertir. */
  async function exportSelection() {
    const paths = [...multiSelected].filter((p) => sortedFiles.some((f) => f.path === p));
    if (paths.length === 0) {
      toast.error("Aucun fichier dans la sélection — un dossier ne s'exporte pas");
      return;
    }
    await showExportSelectionMenu(paths, settings.gameDir);
  }

  function doCopySelection() {
    const paths = multiSelected.size > 0 ? [...multiSelected] : state.selected ? [state.selected] : [];
    if (paths.length === 0) {
      toast.error("Rien à copier — sélectionnez d'abord un fichier ou un dossier");
      return;
    }
    writeText(paths.join("\n")).then(() => toast.success(`${paths.length} chemin(s) copié(s)`));
  }

  /** Ctrl+V — VRAI collage (cf. demande utilisatrice « editer doit vraiment copier coller »),
   * pas un simple message : le VFS est en lecture seule (aucun encodeur CPK), donc "coller" veut
   * dire "proposer le fichier du presse-papiers comme remplacement" — le même mécanisme que
   * « Ajouter à un mod… », juste sans repasser par le sélecteur natif puisqu'on a déjà un chemin.
   *
   * Deux sources, la VRAIE (CF_HDROP) tentée en premier (recherche 2026-08-08 « lis vraiment le
   * code de cosmic… les interactions os et le filesystem ») : un Ctrl+C dans l'Explorateur
   * Windows écrit CF_HDROP (liste de fichiers native), PAS forcément de texte lisible — l'ancien
   * `readText()` seul pouvait donc rater un copier-coller pourtant parfaitement légitime depuis
   * l'Explorateur. `api.clipboardReadFileList()` (CF_HDROP réel, `clipboard-win`) est tenté
   * d'abord ; repli sur l'ancien texte-si-chemin-réel pour les autres sources (un chemin copié
   * comme texte depuis ailleurs, notre propre `doCopySelection` qui écrit des chemins VFS en
   * texte — PAS des fichiers réels, cf. son commentaire).
   */
  async function doPaste() {
    if (!state.selected) {
      toast.error("Sélectionnez d'abord un fichier VFS à remplacer");
      return;
    }
    let clip = (await api.clipboardReadFileList().catch(() => null))?.[0]?.trim();
    if (!clip || !(await api.diskFileExists(clip).catch(() => false))) {
      clip = (await readText().catch(() => null))?.trim();
      if (!clip || clip.includes("\n") || !(await api.diskFileExists(clip).catch(() => false))) {
        toast.message("Rien à coller.", {
          description: "Le presse-papiers doit contenir un fichier réel (copié depuis l'Explorateur Windows) ou le chemin d'UN fichier existant sur disque.",
        });
        return;
      }
    }
    try {
      const mods = await modsDb.listMods();
      const modId = mods[0]?.id ?? (await modsDb.createMod("Presse-papiers", "Créé automatiquement par Ctrl+V"));
      const modName = mods[0]?.name ?? "Presse-papiers";
      const ok = await stageReplacementFromPath(modId, { kind: "vfs", path: state.selected }, clip, settings.gameDir);
      if (ok) toast.success(`Collé dans le mod « ${modName} »`, { description: state.selected });
    } catch (e) {
      toast.error(String(e));
    }
  }

  // `editBus` est un SINGLETON : une seule vue au monde y détient les opérations Édition. Toutes
  // les instances d'onglet sont montées en même temps — sans la garde `active`, chacune
  // s'enregistrerait (la dernière montée gagnerait, pas celle qu'on regarde) et le nettoyage d'un
  // onglet fermé effacerait l'enregistrement d'un onglet bien vivant.
  useEffect(() => {
    if (!active) return;
    registerFileOps({ selectAll: doSelectAll, copySelection: doCopySelection, paste: doPaste });
    return () => registerFileOps(null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [active, multiSelected, state.selected, sortedDirs, sortedFiles, state.prefix, searching]);

  // Ctrl+D « Add to sidebar » — raccourci réel de cosmic-files (confirmé sur capture du menu
  // File), adapté ici pour épingler le dossier COURANT (pas une sélection multi-fichiers).
  // Écouteur `window`, donc global : posé UNIQUEMENT par l'onglet actif, sinon N onglets
  // basculeraient l'épingle N fois (soit un aller-retour, soit rien du tout selon la parité).
  useEffect(() => {
    if (!active) return;
    function onKeyDown(e: KeyboardEvent) {
      const tag = (e.target as HTMLElement | null)?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA") return;
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "d") {
        e.preventDefault();
        togglePin(state.prefix);
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [active, state.prefix]);

  // Ctrl+A/Ctrl+C : gérés par l'accélérateur RÉEL du menu natif « Édition » (`App.tsx`, via
  // `editBus`), pas ici — un accélérateur de menu natif est global (fonctionne même si la liste
  // n'a pas le focus DOM), donc un doublon local ferait potentiellement doubler l'action.
  function onListKeyDown(e: React.KeyboardEvent) {
    if (searching || flatEntries.length === 0) return;
    // Le curseur prime sur la sélection : c'est lui que les flèches déplacent, et il peut être
    // posé sur un dossier, que `state.selected` n'accepte pas.
    const idx = flatEntries.findIndex((en) => en.path === (curseur ?? state.selected));
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      // Sans curseur ni sélection (`idx === -1`), la première flèche prend la première entrée,
      // quel que soit son sens : il faut bien entrer dans la liste par un bout.
      const next =
        idx === -1
          ? 0
          : e.key === "ArrowDown"
            ? Math.min(idx + 1, flatEntries.length - 1)
            : Math.max(idx - 1, 0);
      const entry = flatEntries[next];
      if (!entry) return;
      setCurseur(entry.path);
      if (entry.kind === "file") onStateChange({ selected: entry.path });
      faireDefilerVers(entry.path);
    } else if (e.key === "Enter" && idx >= 0) {
      const entry = flatEntries[idx];
      if (entry.kind === "dir") goto(entry.path);
      else onStateChange({ selected: entry.path });
    } else if (e.key === "Backspace") {
      goto(segments.slice(0, -1).join("/"));
    }
  }

  // Résout le type de cible depuis la ligne réellement sélectionnée : `entryIndex` présent =
  // entrée d'un `.cpk` brut (vue fusionnée), sinon un chemin VFS normal.
  const selectedRow = sortedFiles.find((f) => f.path === state.selected) ?? null;
  /** Code interne de l'entité à laquelle appartient le fichier sélectionné (`c01000010.g4tx` →
   * `c01000010`) — clé de l'éditeur de propriétés. Vide pour un dossier ou un nom sans code. */
  const selectedCode = state.selected ? codeOf(state.selected.split("/").pop() ?? "") : "";
  const target: DetailTarget | null = !state.selected
    ? null
    : selectedRow?.entryIndex !== undefined
      ? { kind: "raw_cpk", path: state.selected, entryIndex: selectedRow.entryIndex }
      : { kind: "vfs", path: state.selected };

  return (
    // Deux colonnes : contenu + inspecteur à droite, **redimensionnable**. Sa largeur était
    // figée à 320 px — trop étroit pour un aperçu d'image large ou une table de config à dix
    // colonnes, trop large quand on veut voir l'arborescence. `SplitPane` persiste le choix
    // (`nie-explorer:split:explorer-inspector`), comme il le fait déjà dans l'Éditeur et Lua.
    <SplitPane
      axis="x"
      side="end"
      defaultSize={320}
      min={240}
      max={880}
      storageKey="explorer-inspector"
      className="h-full min-h-0"
      panel={
        <>
          {/* Inspecteur : aperçu du fichier, ET éditeur de propriétés de l'ENTITÉ à laquelle il
           * appartient (modèle/texture/config du même code interne, données éditables, fonctions et
           * adresses de `nie.exe` qui le manipulent). Sélectionner la texture d'un joueur ouvre donc
           * la fiche du joueur, pas seulement un aperçu d'image. */}
          <div className="flex h-full min-w-0 flex-col gap-2 p-2 pl-0">
            <Tabs value={inspectorTab} onValueChange={(v) => v && setInspectorTab(v as "preview" | "properties")}>
              <TabsList variant="line">
                <TabsTrigger value="preview" className="text-xs">
                  Aperçu
                </TabsTrigger>
                <TabsTrigger value="properties" className="text-xs" disabled={!selectedCode}>
                  Propriétés
                </TabsTrigger>
              </TabsList>
            </Tabs>
            <div className="min-h-0 flex-1 overflow-hidden rounded-lg border border-app-line bg-app-box/60">
              {inspectorTab === "properties" && selectedCode ? (
                <PropertyEditor
                  code={selectedCode}
                  className="h-full p-3"
                  onOpenFile={(p) => onStateChange({ selected: p })}
                />
              ) : (
                <DetailPane target={target} />
              )}
            </div>
          </div>
        </>
      }
    >
      {/* `relative` : ancre de la barre flottante de sélection (`SelectionBar`, en `absolute`). */}
        <div className="relative flex min-h-0 flex-1 flex-col gap-2 p-2">
          {/* Barre d'outils — mise en forme de la `TopBar` de l'explorer spacedrive : boutons ronds
           * (`CircleButton`) sur fond transparent et fil d'Ariane en texte, plutôt qu'une pilule
           * pleine largeur. */}
          <div className="flex items-center gap-1.5">
            {/* Arrière/Avant parcourent l'HISTORIQUE de cet onglet (là où l'on est déjà passé) ;
              * « remonter » suit la HIÉRARCHIE (le dossier parent). Deux gestes différents : après
              * un saut depuis la barre latérale, « arrière » revient au dossier précédent alors que
              * « remonter » descend d'un cran dans l'arborescence du nouvel emplacement. */}
            <CircleButton
              icon="arrow_back"
              size="sm"
              title="Précédent"
              aria-label="Précédent"
              disabled={!canGoBack}
              onClick={() => onBack?.()}
            />
            <CircleButton
              icon="arrow_forward"
              size="sm"
              title="Suivant"
              aria-label="Suivant"
              disabled={!canGoForward}
              onClick={() => onForward?.()}
            />
            <CircleButton
              icon="home"
              size="sm"
              title={t("explorer.root")}
              aria-label={t("explorer.root")}
              onClick={() => goto("")}
            />
            <CircleButton
              icon="expand_less"
              size="sm"
              title={t("explorer.parent")}
              aria-label={t("explorer.parent")}
              disabled={segments.length === 0}
              onClick={() => goto(segments.slice(0, -1).join("/"))}
            />
            <nav className="flex min-w-0 flex-1 flex-wrap items-center gap-0.5 text-xs">
              {segments.map((seg, i) => (
                <span key={i} className="flex items-center gap-0.5">
                  <button
                    type="button"
                    className="rounded-md px-1.5 py-0.5 text-ink-dull transition-colors hover:bg-app-hover hover:text-ink"
                    onClick={() => goto(segments.slice(0, i + 1).join("/"))}
                  >
                    {seg}
                  </button>
                  <span className="text-ink-faint">/</span>
                </span>
              ))}
            </nav>
            <CircleButton
              icon="stars"
              size="sm"
              variant={pins.includes(state.prefix) ? "accent" : "default"}
              title="Épingler à la barre latérale (Ctrl+D)"
              aria-label="Épingler à la barre latérale"
              onClick={() => togglePin(state.prefix)}
            />
            <CircleButton
              icon={sortKey === "name" ? "sort_by_alpha" : "table_rows"}
              size="sm"
              title={sortKey === "name" ? t("explorer.sort_size") : t("explorer.sort_name")}
              aria-label={sortKey === "name" ? t("explorer.sort_size") : t("explorer.sort_name")}
              onClick={() => {
                const next = sortKey === "name" ? "size" : "name";
                setSortKey(next);
                onStateChange({ sortKey: next });
              }}
            />
            <Popover>
              <PopoverTrigger
                render={
                  <CircleButton
                    icon="tune"
                    size="sm"
                    title="Options d'affichage"
                    aria-label="Options d'affichage"
                  />
                }
              />
              <PopoverContent className="w-56">
                <p className="px-1 pb-1 type-label-small text-on-surface-variant">Affichage</p>
                {/* Vue Liste/Grille — ToggleGroup porté de `spaceui/primitives/ToggleGroup.tsx`
                 * (spacedrive), cf. components/ui/toggle-group.tsx. */}
                <ToggleGroup
                  value={[viewMode]}
                  onValueChange={(v) => {
                    const next = v[0] as "list" | "grid" | undefined;
                    if (!next) return;
                    setViewMode(next);
                    onStateChange({ viewMode: next });
                  }}
                  className="w-full"
                >
                  <ToggleGroupItem value="list" className="flex-1 justify-center">
                    <Icon name="view_list" size={14} />
                    Liste
                  </ToggleGroupItem>
                  <ToggleGroupItem value="grid" className="flex-1 justify-center">
                    <Icon name="grid_view" size={14} />
                    Grille
                  </ToggleGroupItem>
                </ToggleGroup>
                {viewMode === "grid" && (
                  <div className="px-1 pt-2">
                    <p className="pb-1 type-label-small text-on-surface-variant">Taille des vignettes</p>
                    <Slider
                      value={[gridSize]}
                      onValueChange={(v) => {
                        const next = (Array.isArray(v) ? v[0] : v) ?? gridSize;
                        setGridSize(next);
                        onStateChange({ gridSize: next });
                      }}
                      min={72}
                      max={192}
                      step={8}
                    />
                  </div>
                )}
              </PopoverContent>
            </Popover>
          </div>

          <div className="flex gap-2">
            <Input
              placeholder={t("explorer.search_placeholder")}
              value={query}
              onChange={(e) => {
                setQuery(e.target.value);
                onStateChange({ query: e.target.value });
              }}
            />
            <Input
              placeholder={t("explorer.ext_placeholder")}
              className="w-20"
              value={ext}
              onChange={(e) => {
                setExt(e.target.value);
                onStateChange({ ext: e.target.value });
              }}
            />
          </div>

          {error && <p className="type-body-small text-error">{error}</p>}

          {!searching && role && (
            <div className="rounded-lg border border-app-line bg-app-box p-3 text-ink-dull">
              <p className="text-xs leading-relaxed">{role.role}</p>
              <Badge variant="outline" className="mt-1.5">
                {role.status}
              </Badge>
            </div>
          )}

          <ScrollArea
            className="min-h-0 flex-1 rounded-2xl border border-app-line bg-app-dark-box"
            tabIndex={0}
            onKeyDown={onListKeyDown}
          >
            <div
              ref={listeRef}
              className={viewMode === "grid" ? "grid gap-2 p-2" : "divide-y divide-app-line py-1"}
              style={viewMode === "grid" ? { gridTemplateColumns: `repeat(auto-fill,minmax(${gridSize}px,1fr))` } : undefined}
            >
              {!searching &&
                sortedDirs.map((d) => {
                  const path = state.prefix ? `${state.prefix}/${d.name}` : d.name;
                  const isMultiSelected = multiSelected.has(path);
                  // `0` = compte inconnu (vue `data/packs`), pas « dossier vide » : on n'affiche
                  // alors rien plutôt qu'un chiffre faux.
                  const compte = d.count > 0 ? d.count.toLocaleString() : "";
                  const infobulle = compte ? `${d.name} — ${compte} fichiers` : d.name;
                  if (viewMode === "grid") {
                    return (
                      <button
                        key={d.name}
                        data-path={path}
                        className={`state-layer flex flex-col items-center gap-1 rounded-xl p-2 text-center ${
                          isMultiSelected ? "bg-primary-container/40" : ""
                        } ${curseur === path ? "ring-1 ring-inset ring-accent" : ""}`}
                        onClick={(e) => handleDirClick(path, e)}
                        onAuxClick={(e) => handleDirAuxClick(path, e)}
                        onContextMenu={(e) => showFolderMenu(path, e)}
                        title={infobulle}
                      >
                        <Icon name="folder" size={40} className="shrink-0 text-primary" />
                        <span className="w-full truncate type-label-small text-on-surface">{d.name}</span>
                        {compte && <span className="type-label-small text-on-surface-variant">{compte}</span>}
                      </button>
                    );
                  }
                  return (
                    <button
                      key={d.name}
                      data-path={path}
                      className={`state-layer flex w-full items-center gap-2 px-3 py-2 text-left type-body-medium ${
                        isMultiSelected ? "bg-primary-container/40 text-on-surface" : "text-on-surface"
                      } ${curseur === path ? "ring-1 ring-inset ring-accent" : ""}`}
                      onClick={(e) => handleDirClick(path, e)}
                      onAuxClick={(e) => handleDirAuxClick(path, e)}
                      onContextMenu={(e) => showFolderMenu(path, e)}
                      title={infobulle}
                    >
                      <Icon name="folder" size={16} className="shrink-0 text-primary" />
                      <span className="truncate">{d.name}</span>
                      {compte && <span className="ml-auto shrink-0 tabular-nums type-label-small text-on-surface-variant">{compte}</span>}
                    </button>
                  );
                })}
              {viewMode === "grid" &&
                affiches.map((f) => {
                  const ext = f.name.includes(".") ? f.name.split(".").pop()!.toLowerCase() : "";
                  const isMultiSelected = multiSelected.has(f.path);
                  return (
                    <button
                      key={f.path}
                      data-path={f.path}
                      className={`state-layer flex flex-col items-center gap-1 rounded-xl p-2 text-center ${
                        curseur === f.path ? "ring-1 ring-inset ring-accent " : ""
                      }${
                        state.selected === f.path
                          ? "bg-secondary-container"
                          : isMultiSelected
                            ? "bg-primary-container/40"
                            : ""
                      }`}
                      onClick={(e) => handleFileClick(f, e)}
                      onContextMenu={(e) => handleFileContextMenu(f, e)}
                      title={f.path}
                    >
                      <div className="h-16 w-16 shrink-0">
                        <FileThumbnail path={f.path} ext={ext} gameDir={settings.gameDir} />
                      </div>
                      <span className="w-full truncate type-label-small text-on-surface">
                        {resolved.get(codeOf(f.name))?.name ?? f.name}
                      </span>
                    </button>
                  );
                })}
              {viewMode === "list" &&
                affiches.map((f) => {
                const name = resolved.get(codeOf(f.name));
                const isMultiSelected = multiSelected.has(f.path);
                return (
                  <button
                    key={f.path}
                    data-path={f.path}
                    className={`state-layer flex w-full items-center justify-between gap-2 px-3 py-2 text-left type-body-medium ${
                      state.selected === f.path
                        ? "bg-secondary-container text-on-secondary-container"
                        : isMultiSelected
                          ? "bg-primary-container/40 text-on-surface"
                          : "text-on-surface"
                    } ${curseur === f.path ? "ring-1 ring-inset ring-accent" : ""}`}
                    onClick={(e) => handleFileClick(f, e)}
                    onContextMenu={(e) => handleFileContextMenu(f, e)}
                    title={f.path}
                  >
                    <span className="flex min-w-0 items-center gap-2">
                      <Icon name="description" size={16} className="shrink-0 text-on-surface-variant" />
                      <span className="flex min-w-0 flex-col">
                        <span className="truncate">{name?.name ?? (searching ? f.path : f.name)}</span>
                        {name && (
                          <span className="truncate type-label-small text-on-surface-variant">
                            {searching ? f.path : f.name}
                            {name.extra ? ` · ${name.extra}` : ""}
                          </span>
                        )}
                      </span>
                    </span>
                    <span className="shrink-0 type-label-small text-on-surface-variant">{humanSize(f.size)}</span>
                  </button>
                );
              })}
              {sortedFiles.length > visibles && (
                <button
                  type="button"
                  className="state-layer m-2 rounded-lg border border-outline-variant px-3 py-2 type-label-large text-on-surface-variant"
                  onClick={() => setVisibles((n) => n + PAGE_FICHIERS)}
                >
                  Afficher {Math.min(PAGE_FICHIERS, sortedFiles.length - visibles).toLocaleString("fr-FR")} fichiers
                  de plus ({(sortedFiles.length - visibles).toLocaleString("fr-FR")} restants)
                </button>
              )}
              {!loading && dirs.length === 0 && files.length === 0 && (
                <p className="p-4 type-body-small text-on-surface-variant">{t("explorer.empty")}</p>
              )}
            </div>
          </ScrollArea>
          {/* Barre de statut — pattern porté de l'Explorer spacedrive (compteur à gauche, résumé de
           * la sélection courante à droite dès qu'elle est non vide). */}
          <div className="flex items-center justify-between gap-2 type-label-small text-on-surface-variant">
            <span>
              {loading
                ? t("explorer.loading")
                : searching
                  ? // Le total n'est affiché QUE s'il dépasse la page : « 500 sur 12 480 » dit ce
                    // qui manque, « 42 sur 42 » n'apprend rien.
                    searchTotal > files.length
                    ? `${t("explorer.results", { n: files.length })} · sur ${searchTotal.toLocaleString("fr-FR")}`
                    : t("explorer.results", { n: files.length })
                  : t("explorer.count", { dirs: dirs.length, files: files.length })}
            </span>
            {multiSelected.size > 0 && (
              <span>
                {multiSelected.size.toLocaleString("fr-FR")} sélectionné(s)
                {selectedTotalSize > 0 && ` · ${humanSize(selectedTotalSize)}`}
              </span>
            )}
          </div>

          <SelectionBar
            count={multiSelected.size}
            totalSize={selectedTotalSize}
            onClear={() => {
              setMultiSelected(new Set());
              setFolderAnchor(null);
            }}
            onCopyPaths={doCopySelection}
            onStageIntoMod={() => void stageSelectionIntoMod()}
            onExport={() => void exportSelection()}
          />
        </div>
    </SplitPane>
  );
}
