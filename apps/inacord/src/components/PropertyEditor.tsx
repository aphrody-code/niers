// Éditeur de propriétés — l'inspecteur « façon IDE » de niers.
//
// Une entité du jeu (un joueur, une technique, un objet, un mode de jeu) n'existe pas dans UN
// fichier : elle est éclatée entre son modèle 3D, ses textures, ses sons, ses lignes de config
// `.cfg.bin`, ses scripts, et le code machine de `nie.exe` qui la manipule. Jusqu'ici il fallait
// aller chercher chacun de ces morceaux à la main, dans quatre écrans différents, sans que rien
// ne dise qu'ils parlent du même personnage.
//
// Ce panneau prend un CODE INTERNE (`c01000010` = Mark Evans, `whs00340` = une technique, …) et
// rassemble tout ce qui s'y rattache, chaque aspect restant ÉDITABLE là où il l'est déjà :
//   • Identité  — nom localisé résolu par le miroir wiki (`nameResolve`/`wikiDb`).
//   • Fichiers  — tous les fichiers VFS du même code, groupés par nature (modèle, texture, son,
//                 config, script), chacun ouvrable dans l'éditeur adéquat.
//   • Données   — les `.cfg.bin` liés, décodés en JSON et RÉÉCRITS par les encodeurs vérifiés
//                 (`encode_cfgbin_config`, T2B et RDBN) — c'est le même chemin d'édition que
//                 `DetailPane`, pas une seconde implémentation.
//   • Moteur    — fonctions labellisées, classes RTTI et adresses statiques de `niers.sqlite`
//                 (`nie-re`) dont le nom mentionne le code : le pont entre la donnée et le code
//                 machine qui la lit, avec l'adresse à ouvrir dans r2/Ghidra ou dans l'onglet
//                 Live (lecture mémoire du process en cours).
//
// Rien n'est inventé ici : chaque source est un module déjà en place (`vfsIndexDb`, `api.related`,
// `reDb`, `api.vfsDecodeCfgbin`/`encodeCfgbinConfig`). Ce composant est le point de jonction qui
// manquait.
import { lazy, Suspense, useEffect, useMemo, useState } from "react";
import { confirm } from "@tauri-apps/plugin-dialog";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/Icon";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { api } from "@/lib/api";
import { humanSize } from "@/lib/bytes";
import { useResolvedName } from "@/lib/nameResolve";
import { defaultReDbPath, reDb, toStaticHex, type FunctionRow, type RttiClassRow } from "@/lib/reDb";
import { useSettings } from "@/lib/settings";
import { codeOf, vfsIndexDb } from "@/lib/vfsIndexDb";
import { cn } from "@/lib/utils";

// Même séparation que l'inspecteur : un panneau de propriétés ne charge Monaco qu'au moment où
// l'utilisateur édite réellement une configuration décodée.
const CfgbinViewer = lazy(() =>
  import("@/components/CfgbinViewer").then(({ CfgbinViewer }) => ({ default: CfgbinViewer })),
);

type Aspect = "files" | "data" | "engine";

const ASPECT_LABELS: Record<Aspect, string> = {
  files: "Fichiers",
  data: "Données",
  engine: "Moteur",
};

/** Classement d'un fichier lié par nature — pilote le regroupement et l'icône. */
type FileKind = "model" | "texture" | "audio" | "video" | "config" | "script" | "other";

const KIND_OF_EXT: Record<string, FileKind> = {
  g4md: "model",
  g4mg: "model",
  g4pkm: "model",
  g4sk: "model",
  g4mt: "model",
  g4ma: "model",
  g4tx: "texture",
  png: "texture",
  dds: "texture",
  acb: "audio",
  awb: "audio",
  hca: "audio",
  adx: "audio",
  usm: "video",
  lua: "script",
  luac: "script",
  lc: "script",
};

const KIND_LABELS: Record<FileKind, string> = {
  model: "Modèles 3D",
  texture: "Textures",
  audio: "Audio",
  video: "Vidéo",
  config: "Configuration",
  script: "Scripts",
  other: "Autres",
};

const KIND_ICONS: Record<FileKind, string> = {
  model: "view_in_ar",
  texture: "image",
  audio: "volume_up",
  video: "movie",
  config: "database",
  script: "edit_note",
  other: "description",
};

function kindOf(path: string): FileKind {
  const lower = path.toLowerCase();
  if (lower.endsWith(".cfg.bin")) return "config";
  const ext = lower.split(".").pop() ?? "";
  return KIND_OF_EXT[ext] ?? "other";
}

interface RelatedFile {
  path: string;
  name: string;
  size: number;
  kind: FileKind;
}

/** Onglet « Données » : décode un `.cfg.bin` lié, laisse l'éditer, le réécrit par le même chemin
 * vérifié que `DetailPane` (`encode_cfgbin_config` → T2B ou RDBN selon la forme du JSON). */
function ConfigEditor({ path, gameDir }: { path: string; gameDir?: string }) {
  const [json, setJson] = useState<string | null>(null);
  const [format, setFormat] = useState<"t2b" | "rdbn" | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setLoading(true);
    setError(null);
    setJson(null);
    api
      .vfsDecodeCfgbin(path, gameDir)
      .then((value) => {
        setJson(JSON.stringify(value, null, 2));
        setFormat(value && typeof value === "object" && "lists" in value ? "rdbn" : "t2b");
      })
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  }, [path, gameDir]);

  async function save() {
    if (!json) return;
    const ok = await confirm(
      `Ceci réécrit « ${path} » dans le dossier du jeu.\n\n` +
        "⚠️ Easy Anti-Cheat est présent sur cette installation — une modification du dossier du jeu peut être détectée. Continuer ?",
      { title: "Enregistrer la configuration", kind: "warning" },
    );
    if (!ok) return;
    setSaving(true);
    try {
      const b64 = await api.encodeCfgbinConfig(path, json, gameDir);
      const written = await api.writeLooseOverrideB64(path, b64, gameDir);
      toast.success(`${humanSize(written)} écrits → ${path}`);
    } catch (e) {
      toast.error(String(e));
    } finally {
      setSaving(false);
    }
  }

  if (loading) return <p className="p-3 text-xs text-ink-faint">décodage…</p>;
  if (error) return <p className="p-3 text-xs text-status-error">{error}</p>;
  if (!json) return null;

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-2">
      <div className="flex items-center justify-between gap-2">
        <span className="truncate text-tiny text-ink-faint" title={path}>
          {path} · {format?.toUpperCase()}
        </span>
        <div className="flex shrink-0 gap-1">
          <Button
            size="xs"
            variant="ghost"
            onClick={async () => {
              await writeText(json);
              toast.success("JSON copié");
            }}
          >
            Copier
          </Button>
          <Button size="xs" variant="destructive" onClick={save} disabled={saving}>
            Enregistrer
          </Button>
        </div>
      </div>
      {/* Même visionneuse que `DetailPane` : sans ça, les deux points d'édition d'un `.cfg.bin`
       * divergeraient dès la première évolution de l'un des deux. */}
      <Suspense fallback={<p className="p-3 text-xs text-ink-faint">Chargement de l’éditeur de configuration…</p>}>
        <CfgbinViewer value={json} onChange={setJson} format={format ?? "t2b"} className="min-h-0 flex-1" />
      </Suspense>
    </div>
  );
}

export function PropertyEditor({
  code,
  onOpenFile,
  className,
}: {
  /** Code interne de l'entité (`c01000010`, `whs00340`, …). */
  code: string;
  /** Ouvre un fichier lié dans l'Explorateur (aperçu/édition). */
  onOpenFile?: (path: string) => void;
  className?: string;
}) {
  const settings = useSettings();
  const [aspect, setAspect] = useState<Aspect>("files");
  const [files, setFiles] = useState<RelatedFile[]>([]);
  const [filesLoading, setFilesLoading] = useState(true);
  const [filesError, setFilesError] = useState<string | null>(null);
  const [selectedConfig, setSelectedConfig] = useState<string | null>(null);

  const [reDbPath, setReDbPath] = useState<string | null>(null);
  const [functions, setFunctions] = useState<FunctionRow[]>([]);
  const [classes, setClasses] = useState<RttiClassRow[]>([]);
  const [engineLoading, setEngineLoading] = useState(false);
  const [engineError, setEngineError] = useState<string | null>(null);

  const resolved = useResolvedName(settings.wikiDb, code);

  // Fichiers liés — index SQL précis (`vfs_files.code`, si la réindexation a été faite), sinon
  // repli substring en mémoire côté Rust (`vfs_related`). Même stratégie que `SearchView`.
  useEffect(() => {
    let cancelled = false;
    setFilesLoading(true);
    setFilesError(null);
    (async () => {
      let rows: RelatedFile[] = [];
      try {
        const indexed = await vfsIndexDb.related(code, 400);
        rows = indexed.map((r) => ({ path: r.path, name: r.name, size: r.size, kind: kindOf(r.path) }));
      } catch {
        rows = [];
      }
      if (rows.length === 0) {
        const hits = await api.related(code, 400, settings.gameDir);
        rows = hits.map((h) => ({ path: h.path, name: h.name, size: h.size, kind: kindOf(h.path) }));
      }
      if (!cancelled) setFiles(rows);
    })()
      .catch((e) => !cancelled && setFilesError(String(e)))
      .finally(() => !cancelled && setFilesLoading(false));
    return () => {
      cancelled = true;
    };
  }, [code, settings.gameDir]);

  // Base RE — chemin auto-détecté une fois, puis recherche par code (les labels de `nie-re`
  // contiennent souvent le code interne, ex. `chara_c01000010_*`).
  useEffect(() => {
    const dir = settings.gameDir;
    if (!dir.trim()) return;
    defaultReDbPath(dir)
      .then(setReDbPath)
      .catch(() => setReDbPath(null));
  }, [settings.gameDir]);

  useEffect(() => {
    if (aspect !== "engine" || !reDbPath) return;
    let cancelled = false;
    setEngineLoading(true);
    setEngineError(null);
    Promise.all([reDb.searchFunctions(reDbPath, code, 50), reDb.searchRttiClasses(reDbPath, code, 25)])
      .then(([fns, cls]) => {
        if (cancelled) return;
        setFunctions(fns);
        setClasses(cls);
      })
      .catch((e) => !cancelled && setEngineError(String(e)))
      .finally(() => !cancelled && setEngineLoading(false));
    return () => {
      cancelled = true;
    };
  }, [aspect, reDbPath, code]);

  const grouped = useMemo(() => {
    const out = new Map<FileKind, RelatedFile[]>();
    for (const f of files) {
      const list = out.get(f.kind) ?? [];
      list.push(f);
      out.set(f.kind, list);
    }
    for (const list of out.values()) list.sort((a, b) => a.name.localeCompare(b.name));
    return out;
  }, [files]);

  const configs = grouped.get("config") ?? [];

  // Premier `.cfg.bin` sélectionné d'office : sans ça, l'onglet Données s'ouvre vide alors qu'il a
  // presque toujours quelque chose à montrer.
  useEffect(() => {
    setSelectedConfig(configs[0]?.path ?? null);
  }, [code, configs.length]);

  const ORDER: FileKind[] = ["model", "texture", "audio", "video", "config", "script", "other"];

  return (
    <div className={cn("flex min-h-0 flex-col gap-2", className)}>
      {/* En-tête d'identité */}
      <div className="flex flex-col gap-1 border-b border-app-line pb-2">
        <div className="flex items-center gap-2">
          <h3 className="min-w-0 flex-1 truncate text-sm font-semibold text-ink" title={code}>
            {resolved?.name ?? code}
          </h3>
          {resolved && (
            <Badge variant="secondary">
              {resolved.kind === "chara" ? "personnage" : resolved.kind === "skill" ? "technique" : "objet"}
            </Badge>
          )}
        </div>
        <span className="truncate text-tiny text-ink-faint">
          {code}
          {resolved?.extra ? ` · ${resolved.extra}` : ""}
          {` · ${files.length} fichier(s) lié(s)`}
        </span>
      </div>

      <Tabs value={aspect} onValueChange={(v) => v && setAspect(v as Aspect)}>
        <TabsList variant="line">
          {(Object.keys(ASPECT_LABELS) as Aspect[]).map((a) => (
            <TabsTrigger key={a} value={a} className="text-xs">
              {ASPECT_LABELS[a]}
            </TabsTrigger>
          ))}
        </TabsList>
      </Tabs>

      {aspect === "files" && (
        <ScrollArea className="min-h-0 flex-1">
          {filesError && <p className="p-2 text-xs text-status-error">{filesError}</p>}
          {filesLoading && <p className="p-2 text-xs text-ink-faint">recherche des fichiers liés…</p>}
          <div className="flex flex-col gap-3 pb-2">
            {ORDER.filter((k) => grouped.has(k)).map((k) => (
              <div key={k}>
                <p className="px-1 pb-1 text-tiny font-semibold uppercase tracking-wide text-ink-faint">
                  {KIND_LABELS[k]} · {grouped.get(k)!.length}
                </p>
                <div className="flex flex-col">
                  {grouped.get(k)!.map((f) => (
                    <button
                      key={f.path}
                      type="button"
                      className="flex items-center gap-2 rounded-md px-1.5 py-1 text-left text-xs text-ink-dull transition-colors hover:bg-app-hover hover:text-ink"
                      onClick={() => onOpenFile?.(f.path)}
                      title={f.path}
                    >
                      <Icon name={KIND_ICONS[k]} size={14} />
                      <span className="min-w-0 flex-1 truncate">{f.name}</span>
                      <span className="shrink-0 text-tiny text-ink-faint">{humanSize(f.size)}</span>
                    </button>
                  ))}
                </div>
              </div>
            ))}
            {!filesLoading && files.length === 0 && (
              <p className="p-2 text-xs text-ink-faint">
                Aucun fichier lié trouvé pour ce code. Réindexer le VFS (Paramètres) rend cette
                recherche exacte au lieu d'un repli par sous-chaîne.
              </p>
            )}
          </div>
        </ScrollArea>
      )}

      {aspect === "data" && (
        <div className="flex min-h-0 flex-1 flex-col gap-2">
          {configs.length === 0 ? (
            <p className="p-2 text-xs text-ink-faint">Aucun fichier de configuration lié à ce code.</p>
          ) : (
            <>
              <div className="flex flex-wrap gap-1">
                {configs.map((c) => (
                  <button
                    key={c.path}
                    type="button"
                    className={cn(
                      "rounded-md px-2 py-0.5 text-tiny transition-colors",
                      selectedConfig === c.path
                        ? "bg-accent text-white"
                        : "bg-app-box text-ink-dull hover:bg-app-hover hover:text-ink",
                    )}
                    onClick={() => setSelectedConfig(c.path)}
                    title={c.path}
                  >
                    {c.name}
                  </button>
                ))}
              </div>
              {selectedConfig && <ConfigEditor path={selectedConfig} gameDir={settings.gameDir} />}
            </>
          )}
        </div>
      )}

      {aspect === "engine" && (
        <ScrollArea className="min-h-0 flex-1">
          {!reDbPath && (
            <p className="p-2 text-xs text-ink-faint">
              Base RE introuvable (<code>var/niers.sqlite</code>) — l'onglet Moteur relie une entité
              au code machine de <code>nie.exe</code> qui la manipule ; sans cette base, rien à
              relier.
            </p>
          )}
          {engineError && <p className="p-2 text-xs text-status-error">{engineError}</p>}
          {engineLoading && <p className="p-2 text-xs text-ink-faint">recherche dans niers.sqlite…</p>}
          {reDbPath && !engineLoading && (
            <div className="flex flex-col gap-3 pb-2">
              <div>
                <p className="px-1 pb-1 text-tiny font-semibold uppercase tracking-wide text-ink-faint">
                  Fonctions · {functions.length}
                </p>
                {functions.length === 0 ? (
                  <p className="px-1 text-xs text-ink-faint">Aucune fonction labellisée ne mentionne ce code.</p>
                ) : (
                  functions.map((f) => (
                    <button
                      key={f.id}
                      type="button"
                      className="flex w-full items-center gap-2 rounded-md px-1.5 py-1 text-left font-mono text-tiny text-ink-dull transition-colors hover:bg-app-hover hover:text-ink"
                      title="Copier l'adresse statique"
                      onClick={async () => {
                        await writeText(toStaticHex(f.vaddr));
                        toast.success(`Adresse copiée : ${toStaticHex(f.vaddr)}`);
                      }}
                    >
                      <span className="shrink-0 text-accent">{toStaticHex(f.vaddr)}</span>
                      <span className="min-w-0 flex-1 truncate">{f.name ?? "(sans nom)"}</span>
                      {f.subsystem && <span className="shrink-0 text-ink-faint">{f.subsystem}</span>}
                    </button>
                  ))
                )}
              </div>
              <div>
                <p className="px-1 pb-1 text-tiny font-semibold uppercase tracking-wide text-ink-faint">
                  Classes RTTI · {classes.length}
                </p>
                {classes.length === 0 ? (
                  <p className="px-1 text-xs text-ink-faint">Aucune classe RTTI ne mentionne ce code.</p>
                ) : (
                  classes.map((c) => (
                    <div key={c.id} className="flex items-center gap-2 px-1.5 py-1 font-mono text-tiny text-ink-dull">
                      {c.vtable_vaddr != null && <span className="shrink-0 text-accent">{toStaticHex(c.vtable_vaddr)}</span>}
                      <span className="min-w-0 flex-1 truncate">{c.name}</span>
                    </div>
                  ))
                )}
              </div>
            </div>
          )}
        </ScrollArea>
      )}
    </div>
  );
}

/** Code interne déductible d'un chemin VFS (basename sans extension) — réexporté pour les vues
 * qui décident d'afficher ou non l'éditeur de propriétés. */
export { codeOf };
