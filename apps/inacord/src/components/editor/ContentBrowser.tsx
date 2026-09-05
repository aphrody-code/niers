// Navigateur de contenu — le bandeau bas d'un éditeur type Unreal : arborescence à gauche, grille
// de vignettes à droite, filtre par type d'asset.
//
// Il navigue le MÊME VFS que l'Explorateur (`api.ls`, ~255 800 fichiers) : ce n'est pas une
// seconde base d'assets, juste une présentation orientée « choisir quelque chose à ouvrir dans le
// viewport » plutôt que « inspecter un fichier ». Les vignettes de textures réutilisent
// `api.texturePngB64` (décodage déjà instantané) avec un cache module-level, exactement comme la
// vue grille de l'Explorateur.
//
// Un asset « ouvrable » n'est pas un asset de la famille modèle : c'est un .g4md dont le .g4mg
// frère existe (cf. `openableStems`). Ctrl/cmd+clic sur un asset ouvrable l'AJOUTE à la scène
// courante au lieu de la remplacer.
import { useEffect, useMemo, useRef, useState } from "react";

import { Icon } from "@/components/ui/Icon";
import { Input } from "@/components/ui/input";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { api, type VfsDir } from "@/lib/api";
import { humanSize } from "@/lib/bytes";
import { showVfsFileContextMenu, showVfsFolderContextMenu } from "@/lib/contextMenu";
import { useSettings } from "@/lib/settings";
import { useThumbnail } from "@/lib/thumbs";
import { cn } from "@/lib/utils";

/** Familles d'assets — le filtre qu'un éditeur propose (Unreal : Static Mesh / Texture / Audio…). */
type AssetFilter = "all" | "models" | "textures" | "audio" | "configs";

const FILTER_LABELS: Record<AssetFilter, string> = {
  all: "Tout",
  models: "Modèles",
  textures: "Textures",
  audio: "Audio",
  configs: "Configs",
};

/** Fichiers montés d'un coup dans la grille (cf. `limite`). */
const PAGE = 300;

const MODEL_EXTS = new Set(["g4md", "g4mg", "g4pkm", "g4sk", "g4mt", "g4ma"]);
const TEXTURE_EXTS = new Set(["g4tx", "png", "dds"]);
const AUDIO_EXTS = new Set(["acb", "awb", "hca", "adx"]);

function extOf(name: string): string {
  return name.includes(".") ? name.split(".").pop()!.toLowerCase() : "";
}

function stemOf(name: string): string {
  return name.includes(".") ? name.slice(0, name.lastIndexOf(".")) : name;
}

function matchesFilter(name: string, filter: AssetFilter): boolean {
  if (filter === "all") return true;
  const ext = extOf(name);
  if (filter === "models") return MODEL_EXTS.has(ext);
  if (filter === "textures") return TEXTURE_EXTS.has(ext);
  if (filter === "audio") return AUDIO_EXTS.has(ext);
  return name.toLowerCase().endsWith(".cfg.bin");
}

function iconFor(name: string): string {
  const ext = extOf(name);
  if (MODEL_EXTS.has(ext)) return "view_in_ar";
  if (TEXTURE_EXTS.has(ext)) return "image";
  if (AUDIO_EXTS.has(ext)) return "volume_up";
  if (name.toLowerCase().endsWith(".cfg.bin")) return "database";
  if (ext === "usm") return "movie";
  return "description";
}

/** Vignette : cache borné, file de décodage et résolution réduite vivent dans `lib/thumbs`
 * (source unique, partagée avec la vue grille de l'Explorateur). */
function Thumb({ path, name, gameDir }: { path: string; name: string; gameDir?: string }) {
  const { ref, src } = useThumbnail(path, extOf(name), gameDir);

  return (
    <div ref={ref} className="flex h-14 w-full items-center justify-center overflow-hidden rounded bg-app-darker-box">
      {src ? (
        <img src={src} alt="" className="h-full w-full object-contain" />
      ) : (
        <Icon name={iconFor(name)} size={22} className="text-ink-faint" />
      )}
    </div>
  );
}

export interface ContentBrowserProps {
  prefix: string;
  onNavigate: (prefix: string) => void;
  selected: string | null;
  /** `additive` (ctrl/cmd+clic sur un asset ouvrable) : ajouter à la scène au lieu de la remplacer. */
  onSelect: (path: string, additive: boolean) => void;
  className?: string;
}

export function ContentBrowser({ prefix, onNavigate, selected, onSelect, className }: ContentBrowserProps) {
  const settings = useSettings();
  const [dirs, setDirs] = useState<VfsDir[]>([]);
  const [files, setFiles] = useState<{ path: string; name: string; size: number }[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [filter, setFilter] = useState<AssetFilter>("all");
  const [query, setQuery] = useState("");
  const seq = useRef(0);

  useEffect(() => {
    const mine = ++seq.current;
    setLoading(true);
    setError(null);
    api
      .ls(prefix, settings.gameDir)
      .then((r) => {
        if (mine !== seq.current) return;
        setDirs(r.dirs);
        setFiles(r.files);
      })
      .catch((e) => mine === seq.current && setError(String(e)))
      .finally(() => mine === seq.current && setLoading(false));
  }, [prefix, settings.gameDir]);

  const segments = prefix ? prefix.split("/") : [];
  const shown = useMemo(() => {
    const q = query.trim().toLowerCase();
    return files.filter((f) => matchesFilter(f.name, filter) && (!q || f.name.toLowerCase().includes(q)));
  }, [files, filter, query]);

  // Le VFS a des dossiers de plus de 12 000 fichiers (`.../10_icon_chr/uniform` : 12 560 `.g4tx`).
  // Les monter tous d'un coup, c'est autant de noeuds DOM et d'observateurs d'intersection : la
  // grille rame avant même d'avoir décodé la moindre vignette. On en monte une tranche, le reste
  // à la demande — même remède que la liste de clips d'animation de l'éditeur.
  const [limite, setLimite] = useState(PAGE);
  useEffect(() => setLimite(PAGE), [prefix, filter, query]);
  const affiches = shown.slice(0, limite);

  // `assemble_glb_for_preview` exige le G4MD **et** le G4MG de même nom dans le même dossier : un
  // dossier `chr` qui n'a que des .g4sk/.g4pk (cas de `data/common/chr/c000101`) ne produit qu'un
  // « G4MD introuvable ». On ne présente donc comme ouvrable que le .g4md dont le frère existe —
  // le .g4mg reste visible, mais comme composant, pas comme point d'entrée.
  const openableStems = useMemo(() => {
    const byStem = new Map<string, Set<string>>();
    for (const f of files) {
      const ext = extOf(f.name);
      if (ext !== "g4md" && ext !== "g4mg") continue;
      const stem = stemOf(f.name);
      let exts = byStem.get(stem);
      if (!exts) byStem.set(stem, (exts = new Set()));
      exts.add(ext);
    }
    const ok = new Set<string>();
    for (const [stem, exts] of byStem) if (exts.has("g4md") && exts.has("g4mg")) ok.add(stem);
    return ok;
  }, [files]);

  return (
    <div className={cn("flex min-h-0 flex-col bg-app-dark-box", className)}>
      {/* Barre d'outils du navigateur */}
      <div className="flex shrink-0 items-center gap-2 border-b border-app-line px-2 py-1.5">
        <button
          type="button"
          className="rounded p-1 text-ink-dull transition-colors hover:bg-app-hover hover:text-ink disabled:opacity-40"
          disabled={segments.length === 0}
          onClick={() => onNavigate(segments.slice(0, -1).join("/"))}
          title="Dossier parent"
          aria-label="Dossier parent"
        >
          <Icon name="arrow_back" size={14} />
        </button>
        <nav className="flex min-w-0 flex-1 items-center gap-0.5 overflow-hidden text-tiny">
          <button
            type="button"
            className="rounded px-1 py-0.5 text-ink-faint transition-colors hover:bg-app-hover hover:text-ink"
            onClick={() => onNavigate("")}
          >
            /
          </button>
          {segments.map((seg, i) => (
            <span key={i} className="flex shrink-0 items-center gap-0.5">
              <button
                type="button"
                className="rounded px-1 py-0.5 text-ink-dull transition-colors hover:bg-app-hover hover:text-ink"
                onClick={() => onNavigate(segments.slice(0, i + 1).join("/"))}
              >
                {seg}
              </button>
              <span className="text-ink-faint">/</span>
            </span>
          ))}
        </nav>
        <Input
          placeholder="Filtrer…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          className="h-6 w-40 shrink-0 text-xs"
        />
        <ToggleGroup
          value={[filter]}
          onValueChange={(v) => v[0] && setFilter(v[0] as AssetFilter)}
          className="shrink-0"
        >
          {(Object.keys(FILTER_LABELS) as AssetFilter[]).map((f) => (
            <ToggleGroupItem key={f} value={f} className="px-2 text-tiny">
              {FILTER_LABELS[f]}
            </ToggleGroupItem>
          ))}
        </ToggleGroup>
        <span className="shrink-0 text-tiny text-ink-faint">
          {loading ? "…" : `${shown.length}/${files.length}`}
        </span>
      </div>

      {error && <p className="px-2 py-1 text-tiny text-status-error">{error}</p>}

      <div className="no-scrollbar min-h-0 flex-1 overflow-y-auto p-2">
        {/* Dossiers d'abord, comme tout navigateur d'assets */}
        {dirs.length > 0 && (
          <div className="mb-2 flex flex-wrap gap-1">
            {dirs.map((d) => {
              const path = prefix ? `${prefix}/${d.name}` : d.name;
              return (
                <button
                  key={d.name}
                  type="button"
                  className="flex items-center gap-1.5 rounded border border-app-line bg-app-box px-2 py-1 text-tiny text-ink-dull transition-colors hover:bg-app-hover hover:text-ink"
                  onClick={() => onNavigate(path)}
                  onContextMenu={(e) => {
                    e.preventDefault();
                    showVfsFolderContextMenu({ path, onOpen: () => onNavigate(path) });
                  }}
                  title={d.count > 0 ? `${d.name} — ${d.count.toLocaleString()} fichiers` : d.name}
                >
                  <Icon name="folder" size={13} className="text-accent" />
                  {d.name}
                  {d.count > 0 && <span className="tabular-nums text-ink-faint">{d.count.toLocaleString()}</span>}
                </button>
              );
            })}
          </div>
        )}

        <div className="grid gap-2" style={{ gridTemplateColumns: "repeat(auto-fill,minmax(88px,1fr))" }}>
          {affiches.map((f) => {
            const ext = extOf(f.name);
            const openable = ext === "g4md" && openableStems.has(stemOf(f.name));
            const dead = MODEL_EXTS.has(ext) && !openable;
            const hint = openable
              ? "ouvrable dans le viewport — ctrl/cmd+clic : ajouter à la scène"
              : dead
                ? "non assemblable seul : le viewport a besoin du couple .g4md + .g4mg de même nom"
                : null;
            return (
              <button
                key={f.path}
                type="button"
                className={cn(
                  "relative flex flex-col gap-1 rounded-md border p-1.5 text-left transition-colors",
                  selected === f.path
                    ? "border-accent bg-accent/15"
                    : "border-transparent hover:border-app-line hover:bg-app-hover",
                  dead && "opacity-60",
                )}
                onClick={(e) => onSelect(f.path, openable && (e.ctrlKey || e.metaKey))}
                onContextMenu={(e) => {
                  e.preventDefault();
                  showVfsFileContextMenu({
                    path: f.path,
                    name: f.name,
                    size: f.size,
                    gameDir: settings.gameDir,
                    blenderExe: settings.blenderExe,
                    onOpen: () => onSelect(f.path, false),
                  });
                }}
                title={`${f.path}\n${humanSize(f.size)}${hint ? `\n${hint}` : ""}`}
              >
                <Thumb path={f.path} name={f.name} gameDir={settings.gameDir} />
                {openable && (
                  <span className="absolute right-2 top-2 h-1.5 w-1.5 rounded-full bg-accent" aria-hidden="true" />
                )}
                <span className="w-full truncate text-tiny text-ink-dull">{f.name}</span>
              </button>
            );
          })}
        </div>

        {shown.length > limite && (
          <button
            type="button"
            className="mt-2 w-full rounded border border-app-line px-2 py-1 text-tiny text-ink-dull transition-colors hover:bg-app-hover hover:text-ink"
            onClick={() => setLimite((n) => n + PAGE)}
          >
            Afficher {Math.min(PAGE, shown.length - limite)} fichiers de plus ({shown.length - limite}{" "}
            restants)
          </button>
        )}

        {!loading && shown.length === 0 && dirs.length === 0 && (
          <p className="p-3 text-tiny text-ink-faint">Dossier vide (ou tout filtré).</p>
        )}
      </div>
    </div>
  );
}
