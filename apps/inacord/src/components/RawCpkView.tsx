// Exploration d'un CPK RÉEL, PHYSIQUEMENT présent sur disque, hors du VFS du jeu monté — cf.
// demande utilisatrice « puisse explorer aussi les cpk réels hors du vfs présent physiquement en
// les ouvrant ». Même lecteur `nie_formats::cpk::CpkReader` que le VFS (`crates/engine/nie-formats/src/
// cpk.rs`), juste sans passer par l'indirection `cpk_list.cfg.bin`/`Vfs` : un mod téléchargé, un
// DLC séparé ou une copie de sauvegarde d'un pack peuvent être ouverts directement.
import { useCallback, useState } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { toast } from "sonner";
import { api, type RawCpkEntry } from "@/lib/api";
import { useSettings } from "@/lib/settings";
import { hexLines, humanSize, b64ToBytes } from "@/lib/bytes";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Icon } from "@/components/ui/Icon";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { ModelPreview } from "@/components/ModelPreview";

/** Extension en minuscule d'une entrée CPK — pilote l'affichage conditionnel des boutons audio/vidéo. */
function entryExt(entry: RawCpkEntry): string {
  return entry.path.split(".").pop()?.toLowerCase() ?? "";
}

export function RawCpkView() {
  const settings = useSettings();
  const [cpkPath, setCpkPath] = useState<string | null>(null);
  const [entries, setEntries] = useState<RawCpkEntry[]>([]);
  const [query, setQuery] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [selected, setSelected] = useState<RawCpkEntry | null>(null);
  const [lines, setLines] = useState<string[]>([]);
  const [rawBytes, setRawBytes] = useState<Uint8Array | null>(null);
  const [busy, setBusy] = useState(false);
  const [extractingAll, setExtractingAll] = useState(false);

  // Parité d'outils avec DetailPane (roadmap §6) : audio/vidéo/3D câblés — un `.hca`/`.adx`/`.usm`
  // est autonome (pas de fichier frère référencé), et l'aperçu 3D (GLB) résout désormais ses frères
  // g4mg/g4tx DANS le CPK ouvert (`assemble_glb_from_cpk_entries` côté Rust, PAS le VFS — le gap
  // « résolveur de frères scopé au seul CPK courant » est fermé, 2026-08-08). Blender reste hors de
  // portée : `open_in_blender` dépend du VFS pour l'addon `plugins/niers-blender` + `NIE_GAME_DIR`.
  const [audioUrl, setAudioUrl] = useState<string | null>(null);
  const [audioLoading, setAudioLoading] = useState(false);
  const [videoUrl, setVideoUrl] = useState<string | null>(null);
  const [videoLoading, setVideoLoading] = useState(false);

  async function openCpk() {
    // Ouvre directement dans `<jeu>/data/packs` — c'est là que vivent RÉELLEMENT les `.cpk` du
    // jeu (cf. demande utilisatrice « ouvrir un cpk doit ouvrir un cpk dans leur dossier … data\
    // packs direct »), pas un dossier arbitraire choisi par l'OS par défaut.
    const gameDir = settings.gameDir || (await api.defaultGameDir().catch(() => ""));
    const defaultPath = gameDir ? `${gameDir.replace(/[\\/]+$/, "")}/data/packs` : undefined;
    const picked = await open({ title: "Ouvrir un fichier .cpk", defaultPath, filters: [{ name: "CPK", extensions: ["cpk"] }] });
    if (typeof picked !== "string") return;
    setLoading(true);
    setError(null);
    setSelected(null);
    setLines([]);
    setRawBytes(null);
    try {
      const list = await api.rawCpkOpen(picked);
      setCpkPath(picked);
      setEntries(list);
      toast.success(`${list.length.toLocaleString("fr-FR")} entrée(s) — ${picked}`);
    } catch (e) {
      setError(String(e));
      toast.error(String(e));
    } finally {
      setLoading(false);
    }
  }

  async function selectEntry(entry: RawCpkEntry) {
    setSelected(entry);
    setLines([]);
    setRawBytes(null);
    setAudioUrl(null);
    setVideoUrl(null);
    try {
      setLines(await api.rawCpkDescribe(entry.index));
    } catch (e) {
      setLines([String(e)]);
    }
  }

  async function loadAudio() {
    if (!selected) return;
    setAudioLoading(true);
    try {
      const b64 = await api.rawCpkAudioPreviewB64(selected.index);
      setAudioUrl(`data:audio/wav;base64,${b64}`);
    } catch (e) {
      toast.error(String(e));
    } finally {
      setAudioLoading(false);
    }
  }

  async function loadVideo() {
    if (!selected) return;
    setVideoLoading(true);
    try {
      const b64 = await api.rawCpkVideoPreviewB64(selected.index);
      setVideoUrl(`data:video/mp4;base64,${b64}`);
    } catch (e) {
      toast.error(String(e));
    } finally {
      setVideoLoading(false);
    }
  }

  const loadSelectedGlb = useCallback(
    () => selected ? api.rawCpkGlbBytesB64(selected.index) : Promise.reject(new Error("aucune entrée sélectionnée")),
    [selected],
  );

  async function loadRaw() {
    if (!selected) return;
    setBusy(true);
    try {
      const b64 = await api.rawCpkReadB64(selected.index);
      setRawBytes(b64ToBytes(b64));
    } catch (e) {
      toast.error(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function extractSelected() {
    if (!selected) return;
    const dest = await save({ defaultPath: selected.path.split("/").pop() });
    if (!dest) return;
    setBusy(true);
    try {
      const written = await api.rawCpkExtractTo(selected.index, dest);
      toast.success(`${humanSize(written)} écrits → ${dest}`);
    } catch (e) {
      toast.error(String(e));
    } finally {
      setBusy(false);
    }
  }

  /** Extrait TOUTES les entrées du CPK ouvert vers un dossier choisi (arborescence préservée) —
   * évite d'extraire un gros CPK une entrée à la fois depuis l'UI (§1.3 roadmap). */
  async function extractAll() {
    if (!cpkPath) return;
    const destDir = await open({ title: "Dossier de destination", directory: true });
    if (typeof destDir !== "string") return;
    setExtractingAll(true);
    try {
      const [ok, err] = await api.rawCpkExtractAll(destDir);
      if (err > 0) {
        toast.warning(`${ok} extraites, ${err} échec(s) → ${destDir}`);
      } else {
        toast.success(`${ok.toLocaleString("fr-FR")} entrées extraites → ${destDir}`);
      }
    } catch (e) {
      toast.error(String(e));
    } finally {
      setExtractingAll(false);
    }
  }

  const filtered = query.trim() ? entries.filter((e) => e.path.toLowerCase().includes(query.toLowerCase())) : entries;

  return (
    <div className="grid h-full grid-cols-[minmax(300px,1fr)_1.4fr] gap-3 p-3">
      <div className="flex min-h-0 flex-col gap-2">
        <div className="flex gap-2">
          <Button onClick={openCpk} disabled={loading} className="shrink-0">
            {loading ? "Ouverture…" : "📦 Ouvrir un .cpk…"}
          </Button>
          {cpkPath && (
            <Button variant="outline" onClick={extractAll} disabled={extractingAll} className="shrink-0">
              {extractingAll ? "Extraction…" : "Tout extraire…"}
            </Button>
          )}
          <Input placeholder="Filtrer les entrées…" value={query} onChange={(e) => setQuery(e.target.value)} />
        </div>

        {!cpkPath && !error && (
          <Alert>
            <AlertTitle>Aucun CPK ouvert</AlertTitle>
            <AlertDescription>
              Ouvre n'importe quel fichier <code>.cpk</code> du disque — mod téléchargé, DLC séparé, copie de
              sauvegarde d'un pack — sans dépendre du <code>cpk_list.cfg.bin</code> du jeu monté (onglet Explorateur).
            </AlertDescription>
          </Alert>
        )}
        {error && (
          <Alert variant="destructive">
            <AlertTitle>Échec de l'ouverture</AlertTitle>
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        )}
        {cpkPath && (
          <p className="truncate type-body-small text-on-surface-variant" title={cpkPath}>
            {cpkPath} — {entries.length.toLocaleString("fr-FR")} entrée(s)
          </p>
        )}

        <ScrollArea className="min-h-0 flex-1 rounded-2xl border border-app-line bg-app-dark-box">
          <div className="divide-y divide-app-line py-1">
            {filtered.map((e) => (
              <button
                key={e.index}
                className={`state-layer flex w-full items-center justify-between gap-2 px-3 py-2 text-left type-body-medium ${
                  selected?.index === e.index ? "bg-secondary-container text-on-secondary-container" : "text-on-surface"
                }`}
                onClick={() => selectEntry(e)}
                title={e.path}
              >
                <span className="flex min-w-0 items-center gap-2">
                  <Icon name="description" size={16} className="shrink-0 text-on-surface-variant" />
                  <span className="truncate">{e.path}</span>
                  {e.is_compressed && (
                    <Badge variant="outline" className="shrink-0">
                      compressé
                    </Badge>
                  )}
                </span>
                <span className="shrink-0 type-label-small text-on-surface-variant">{humanSize(e.size)}</span>
              </button>
            ))}
            {cpkPath && filtered.length === 0 && (
              <p className="p-4 type-body-small text-on-surface-variant">Aucune entrée ne correspond.</p>
            )}
          </div>
        </ScrollArea>
      </div>

      <div className="min-h-0 overflow-hidden rounded-2xl border border-app-line bg-app-dark-box">
        {!selected ? (
          <div className="flex h-full items-center justify-center type-body-medium text-on-surface-variant">
            Sélectionnez une entrée pour l'aperçu.
          </div>
        ) : (
          <div className="flex h-full flex-col gap-3 p-4">
            <div>
              <h3 className="truncate type-title-small text-on-surface" title={selected.path}>
                {selected.path.split("/").pop()}
              </h3>
              <p className="truncate type-body-small text-on-surface-variant" title={selected.path}>
                {selected.path}
              </p>
            </div>
            <div className="flex flex-wrap gap-2">
              <Button size="sm" variant="outline" onClick={extractSelected} disabled={busy}>
                Extraire vers…
              </Button>
              <Button size="sm" variant="outline" onClick={loadRaw} disabled={busy}>
                {rawBytes ? "Recharger l'hex" : "Charger l'hex"}
              </Button>
              {["acb", "awb", "hca", "adx"].includes(entryExt(selected)) && !audioUrl && (
                <Button size="sm" variant="outline" onClick={loadAudio} disabled={audioLoading}>
                  {audioLoading ? "Décodage HCA/ADX…" : "🔊 Aperçu audio"}
                </Button>
              )}
              {entryExt(selected) === "usm" && !videoUrl && (
                <Button size="sm" variant="outline" onClick={loadVideo} disabled={videoLoading}>
                  {videoLoading ? "Remuxage ffmpeg…" : "▶️ Aperçu vidéo"}
                </Button>
              )}
            </div>
            {audioUrl && (
              // eslint-disable-next-line jsx-a11y/media-has-caption
              <audio src={audioUrl} controls className="w-full" />
            )}
            {videoUrl && (
              // eslint-disable-next-line jsx-a11y/media-has-caption
              <video src={videoUrl} controls className="max-h-72 w-full rounded-xl border border-app-line bg-black" />
            )}
            {entryExt(selected) === "g4md" && (
              <ModelPreview key={selected.index} path={selected.path} loadGlb={loadSelectedGlb} />
            )}
            <ScrollArea className="min-h-0 flex-1 rounded-lg border border-app-line bg-app-dark-box">
              <pre className="whitespace-pre-wrap p-3 font-mono text-xs leading-relaxed text-on-surface">
                {lines.join("\n") || "…"}
              </pre>
            </ScrollArea>
            {rawBytes && (
              <ScrollArea className="h-40 rounded-lg border border-app-line bg-app-dark-box">
                <pre className="p-2 font-mono text-[11px] leading-relaxed text-on-surface">{hexLines(rawBytes).join("\n")}</pre>
              </ScrollArea>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
