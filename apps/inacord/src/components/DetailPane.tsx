import { lazy, Suspense, useEffect, useMemo, useState } from "react";
import { save, confirm } from "@tauri-apps/plugin-dialog";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { toast } from "sonner";
import { formatDescription, translateEffect } from "@rosegriffon/azalee/text";
import { api, type ExportFormat } from "@/lib/api";
import { useSettings, type Locale } from "@/lib/settings";
import { b64ToBytes, bytesToB64, hexLines, humanSize } from "@/lib/bytes";
import { modsDb, type ModRow } from "@/lib/modsDb";
import { stageReplacement, stageTextureReplacement } from "@/lib/modWorkspace";
import { codeOf } from "@/lib/vfsIndexDb";
import { useResolvedName } from "@/lib/nameResolve";
import { TextureSheet } from "@/components/TextureSheet";
import { AudioBankPanel } from "@/components/AudioBankPanel";
import { CameraTrackView } from "@/components/CameraTrackView";
import { NavmeshView } from "@/components/NavmeshView";
import { ModelPreview } from "@/components/ModelPreview";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

// Monaco et ses workers ne sont utiles qu'après le décodage explicite d'un `.cfg.bin`.
// Les garder hors du chunk de l'Explorateur évite de payer l'éditeur de texte pour chaque dossier.
const CfgbinViewer = lazy(() =>
  import("@/components/CfgbinViewer").then(({ CfgbinViewer }) => ({ default: CfgbinViewer })),
);

const BLENDER_EXTS = new Set(["g4md", "g4mg", "g4sk", "g4mt"]);

// `raw_cpk` : entrée d'un fichier `.cpk` ouvert hors VFS (cf. `ExplorerView` — navigation
// fusionnée dans `data/packs/*.cpk`, cf. demande utilisatrice « il faut fusionner le vfs viewer
// et raw packs cpk viewer »). Référencée par INDEX (pas par chemin, cf. `RawCpkEntryDto`).
export type DetailTarget =
  | { kind: "vfs"; path: string }
  | { kind: "disk"; path: string }
  | { kind: "raw_cpk"; path: string; entryIndex: number };

/** Champs dont le contenu est du texte de jeu — ceux qui portent des balises et de la furigana. */
const CHAMPS_TEXTE = new Set(["description", "desc", "text", "label", "effect", "name"]);

/**
 * Rend lisibles les textes du jeu contenus dans des données typées.
 *
 * Deux traitements distincts, dans cet ordre :
 *
 * 1. **Les balises du moteur** — `formatDescription` résout `<FST:…>`, `<FLC:…>`, `<FLA:…>`,
 *    `<FUL:…>` et `<MNT:…>` vers le nom réel **dans la langue demandée**, et retire la furigana
 *    `[守/まもる]` en gardant le kanji. C'est ce qui s'applique à *tout* texte du jeu.
 * 2. **Le glossaire d'effets** — `translateEffect` ne traduit que les libellés courts et
 *    normalisés (`AT +15%` → `ATT +15%`), et **uniquement vers le français** : le glossaire est
 *    unilingue. En anglais ou en japonais on laisse donc la valeur d'origine, qui est déjà dans
 *    la bonne langue, plutôt que de la franciser à contretemps.
 *
 * Les valeurs brutes ne sont jamais perdues : la vue générique en dessous les montre telles
 * quelles, et c'est elle qui reste éditable.
 */
function rendreLisible(valeur: unknown, locale: Locale): unknown {
  if (typeof valeur === "string") return formatDescription(valeur, locale);
  if (Array.isArray(valeur)) return valeur.map((v) => rendreLisible(v, locale));
  if (valeur && typeof valeur === "object") {
    return Object.fromEntries(
      Object.entries(valeur as Record<string, unknown>).map(([k, v]) => {
        if (typeof v !== "string" || !CHAMPS_TEXTE.has(k.toLowerCase())) {
          return [k, rendreLisible(v, locale)];
        }
        const texte = formatDescription(v, locale);
        return [k, locale === "fr" ? translateEffect(texte) : texte];
      }),
    );
  }
  return valeur;
}

export function DetailPane({ target }: { target: DetailTarget | null }) {
  const settings = useSettings();
  const [lines, setLines] = useState<string[]>([]);
  const [pngUrl, setPngUrl] = useState<string | null>(null);
  const [rawBytes, setRawBytes] = useState<Uint8Array | null>(null);
  const [editText, setEditText] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [mods, setMods] = useState<ModRow[]>([]);
  const [modChoice, setModChoice] = useState("");
  const [videoUrl, setVideoUrl] = useState<string | null>(null);
  const [videoError, setVideoError] = useState<string | null>(null);
  const [videoLoading, setVideoLoading] = useState(false);
  const [audioUrl, setAudioUrl] = useState<string | null>(null);
  const [audioError, setAudioError] = useState<string | null>(null);
  const [audioLoading, setAudioLoading] = useState(false);
  // "loose" = fichier physiquement sur disque (PAS empaqueté dans un CPK, `cpk === ""`) →
  // seul cas où une écriture EN PLACE est possible (`api.writeB64`) ; sinon toujours un export.
  const [isLoose, setIsLoose] = useState(false);
  // Décodeur générique `.cfg.bin` (RDBN/T2B → JSON "inagle") — couvre ~50 000 fichiers de
  // config du jeu, pas seulement les techniques (`GameDataView`), cf. `vfs_decode_cfgbin`.
  // ÉDITABLE + ré-encodable pour le T2B (`encode_t2b`, vérifié par round-trip réel sur des
  // centaines de vrais fichiers) — le RDBN reste lecture seule, aucun encodeur vérifié.
  const [configJson, setConfigJson] = useState<string | null>(null);
  /** Étiquette du parseur `nie-data` qui a répondu (`skill`, `formation`…), `null` si aucun. */
  const [configFamille, setConfigFamille] = useState<string | null>(null);
  /** Données typées, champs nommés — vides quand la famille n'est pas couverte. */
  const [configTypedJson, setConfigTypedJson] = useState<string>("");
  const [configFormat, setConfigFormat] = useState<"t2b" | "rdbn" | null>(null);
  // Formats d'export proposés pour le fichier courant (cf. `src-tauri/src/export.rs`) : dérivés
  // du nom côté Rust, donc rechargés à chaque sélection sans coût disque.
  const [exportFormats, setExportFormats] = useState<ExportFormat[]>([]);
  const [exportFormat, setExportFormat] = useState("");
  const [configLoading, setConfigLoading] = useState(false);
  const [configSaving, setConfigSaving] = useState(false);
  const [configError, setConfigError] = useState<string | null>(null);

  const ext = useMemo(() => (target ? target.path.split(".").pop()?.toLowerCase() ?? "" : ""), [target]);
  const name = useMemo(() => (target ? target.path.split(/[\\/]/).pop() ?? target.path : ""), [target]);
  const isCfgBin = useMemo(() => (target ? target.path.toLowerCase().endsWith(".cfg.bin") : false), [target]);
  // Nom réel (perso/technique/objet) lié à ce fichier — cf. demande utilisatrice « affiche le
  // nom... lié à un fichier au lieu de juste l'id ».
  const resolvedName = useResolvedName(settings.wikiDb, target ? codeOf(name) : null);

  useEffect(() => {
    setLines([]);
    setPngUrl(null);
    setRawBytes(null);
    setEditText("");
    setError(null);
    setVideoUrl(null);
    setVideoError(null);
    setAudioUrl(null);
    setAudioError(null);
    setIsLoose(false);
    setConfigJson(null);
    setConfigFamille(null);
    setConfigTypedJson("");
    setConfigFormat(null);
    setConfigError(null);
    if (!target) return;

    const describe =
      target.kind === "vfs"
        ? api.describe(target.path, settings.gameDir)
        : target.kind === "raw_cpk"
          ? api.rawCpkDescribe(target.entryIndex)
          : api.describeDiskFile(target.path);
    describe.then(setLines).catch((e) => setError(String(e)));

    if (target.kind === "vfs") {
      api
        .entryMeta(target.path, settings.gameDir)
        .then((meta) => setIsLoose(meta?.cpk === ""))
        .catch(() => setIsLoose(false));
    }

    if (ext === "g4tx" && target.kind === "vfs") {
      api
        .texturePngB64(target.path, settings.gameDir)
        .then((b64) => setPngUrl(`data:image/png;base64,${b64}`))
        .catch(() => {});
    }

    // Formats d'export du fichier courant. Le format proposé PAR DÉFAUT est la première
    // conversion disponible (PNG pour une texture, GLB pour un modèle, WAV pour un son…) plutôt
    // que le brut : l'utilisatrice qui ouvre ce menu veut convertir — « Extraire vers… », juste à
    // côté, sert déjà le brut.
    if (target.kind === "vfs") {
      api
        .exportFormats(target.path)
        .then((fs) => {
          setExportFormats(fs);
          setExportFormat(fs.find((f) => !f.brut)?.id ?? fs[0]?.id ?? "");
        })
        .catch(() => {
          setExportFormats([]);
          setExportFormat("");
        });
    } else {
      setExportFormats([]);
      setExportFormat("");
    }
  }, [target, ext, settings.gameDir]);

  useEffect(() => {
    modsDb.listMods().then(setMods).catch(() => {});
    if (target) modsDb.addRecent(target.path, target.kind === "vfs" ? "vfs" : "disk").catch(() => {});
  }, [target]);

  async function stageIntoMod() {
    // Le staging de mod (`stageReplacement`) cible un chemin VFS/disque réel, pas l'index d'une
    // entrée de `.cpk` brut ouvert hors VFS — non applicable ici (le bouton est déjà masqué pour
    // `raw_cpk`, cf. rendu ci-dessous ; ce garde est une seconde ligne de défense).
    if (!target || !modChoice || target.kind === "raw_cpk") return;
    setBusy(true);
    try {
      const ok = await stageReplacement(modChoice, target, settings.gameDir);
      if (ok) toast.success("Ajouté au mod");
    } catch (e) {
      toast.error(String(e));
    } finally {
      setBusy(false);
    }
  }

  /** Crée un mod à la volée puis enchaîne directement sur le choix du fichier de remplacement —
   * sans ça, le panneau était un cul-de-sac tant qu'aucun mod n'existait (il fallait deviner
   * qu'il fallait passer par l'onglet Mods d'abord). */
  async function createModAndStage() {
    if (!target || target.kind === "raw_cpk") return;
    setBusy(true);
    try {
      const id = await modsDb.createMod("Mon mod", "Créé depuis l'aperçu de fichier");
      setMods(await modsDb.listMods());
      setModChoice(id);
      const ok = await stageReplacement(id, target, settings.gameDir);
      if (ok) toast.success("Mod créé et fichier ajouté");
    } catch (e) {
      toast.error(String(e));
    } finally {
      setBusy(false);
    }
  }

  /** Remplace la texture d'un `.g4tx` VFS par un PNG (§2.2 roadmap) — encodage DDS/G4TX côté
   * Rust, cf. `stageTextureReplacement`. Distinct de `stageIntoMod` (copie brute générique) :
   * ici le contenu est RÉENCODÉ, pas simplement copié tel quel. */
  async function stageTexture() {
    if (!target || !modChoice || target.kind !== "vfs") return;
    setBusy(true);
    try {
      const ok = await stageTextureReplacement(modChoice, target.path, settings.gameDir);
      if (ok) toast.success("Texture réencodée et ajoutée au mod");
    } catch (e) {
      toast.error(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function loadRaw() {
    if (!target) return;
    setBusy(true);
    try {
      const b64 =
        target.kind === "vfs"
          ? await api.readB64(target.path, settings.gameDir)
          : target.kind === "raw_cpk"
            ? await api.rawCpkReadB64(target.entryIndex)
            : await api.readDiskFileB64(target.path);
      const bytes = b64ToBytes(b64);
      setRawBytes(bytes);
      setEditText(Array.from(bytes).map((b) => b.toString(16).padStart(2, "0")).join(" "));
    } catch (e) {
      toast.error(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function extract() {
    if (!target) return;
    const dest = await save({ defaultPath: name });
    if (!dest) return;
    setBusy(true);
    try {
      const written =
        target.kind === "vfs"
          ? await api.extractTo(target.path, dest, settings.gameDir)
          : target.kind === "raw_cpk"
            ? await api.rawCpkExtractTo(target.entryIndex, dest)
            : await api.saveBytesB64(dest, bytesToB64(await api.readDiskFileB64(target.path).then(b64ToBytes)));
      toast.success(`${humanSize(written)} écrits → ${dest}`);
    } catch (e) {
      toast.error(String(e));
    } finally {
      setBusy(false);
    }
  }

  /** Export CONVERTI : décode le fichier vers le format choisi et l'écrit (cf.
   * `src-tauri/src/export.rs`). Distinct de `extract`, qui écrit les octets du jeu tels quels —
   * un `.g4tx` extrait reste un `.g4tx`, illisible hors du jeu. */
  async function exportAs() {
    if (!target || target.kind !== "vfs" || !exportFormat) return;
    const dest = await save({ defaultPath: await api.exportDefaultName(target.path, exportFormat) });
    if (!dest) return;
    setBusy(true);
    try {
      const written = await api.exportAs(target.path, dest, exportFormat, settings.gameDir);
      toast.success(`${humanSize(written)} écrits → ${dest}`);
    } catch (e) {
      toast.error(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function saveEdited() {
    const hex = editText.replace(/[^0-9a-fA-F]/g, "");
    if (hex.length % 2 !== 0) {
      toast.error("Hex invalide (nombre impair de chiffres)");
      return;
    }
    const bytes = new Uint8Array(hex.length / 2);
    for (let i = 0; i < bytes.length; i++) bytes[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
    const dest = await save({ defaultPath: name });
    if (!dest) return;
    setBusy(true);
    try {
      const written = await api.saveBytesB64(dest, bytesToB64(bytes));
      toast.success(`Copie modifiée écrite (${humanSize(written)}) → ${dest}`);
    } catch (e) {
      toast.error(String(e));
    } finally {
      setBusy(false);
    }
  }

  /** Écrit la copie éditée EN PLACE (écrase le fichier loose réel sous le dossier du jeu) —
   * seul cas possible : `target.kind === "vfs" && isLoose` (vérifié aussi côté Rust, qui refuse
   * toute entrée empaquetée dans un CPK faute d'encodeur). Confirmation explicite avant
   * d'écrire dans le dossier du jeu, même motif que l'export de mod « overlay loose-file »
   * (EAC présent sur cette installation, comportement réel non confirmé par RE). */
  async function saveInPlace() {
    if (!target || target.kind !== "vfs" || !isLoose) return;
    const hex = editText.replace(/[^0-9a-fA-F]/g, "");
    if (hex.length % 2 !== 0) {
      toast.error("Hex invalide (nombre impair de chiffres)");
      return;
    }
    const ok = await confirm(
      `Ceci écrase directement « ${target.path} » dans le dossier du jeu.\n\n` +
        "⚠️ Easy Anti-Cheat est présent sur cette installation — une modification du dossier du jeu peut être détectée. Continuer ?",
      { title: "Écrire en place", kind: "warning" },
    );
    if (!ok) return;
    const bytes = new Uint8Array(hex.length / 2);
    for (let i = 0; i < bytes.length; i++) bytes[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
    setBusy(true);
    try {
      const written = await api.writeB64(target.path, bytesToB64(bytes), settings.gameDir);
      toast.success(`Écrit en place (${humanSize(written)}) → ${target.path}`);
    } catch (e) {
      toast.error(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function copyPath() {
    if (!target) return;
    await writeText(target.path);
    toast.success("Chemin copié");
  }

  /** Décodeur GÉNÉRIQUE `.cfg.bin` (RDBN/T2B → JSON) — couvre ~50 000 fichiers de config du
   * jeu (personnages, objets, techniques, auras, boutiques, quêtes…), vérifié réel sur un large
   * échantillon (cf. `game_data.rs`). ÉDITABLE dans les deux formats — `saveConfig` ré-encode via
   * `encode_cfgbin_config` (dispatch T2B/RDBN automatique côté Rust), les deux vérifiés par
   * round-trip réel sur le vrai jeu. */
  async function loadConfig() {
    if (!target || target.kind !== "vfs") return;
    setConfigLoading(true);
    setConfigError(null);
    try {
      // Décodage typé : la forme générique reste rendue à côté, donc on ne perd rien quand
      // aucune des 112 familles de `nie-data` ne couvre le fichier — ce qui est le cas de la
      // majorité des `.cfg.bin` du jeu (map, event, effect).
      const r = await api.vfsDecodeCfgbinTyped(target.path, settings.gameDir);
      const brut: unknown = JSON.parse(r.brut);
      setConfigJson(JSON.stringify(brut, null, 2));
      setConfigFormat(brut && typeof brut === "object" && "lists" in brut ? "rdbn" : "t2b");
      setConfigFamille(r.famille);
      setConfigTypedJson(
        r.famille
          ? JSON.stringify(rendreLisible(JSON.parse(r.json), settings.locale), null, 2)
          : "",
      );
    } catch (e) {
      setConfigError(String(e));
    } finally {
      setConfigLoading(false);
    }
  }

  /** Ré-encode `configJson` (édité) en T2B et l'écrit — en place si `isLoose`, sinon en
   * override loose (comportement réel du jeu NON CONFIRMÉ, même avertissement que
   * `saveInPlace`) après confirmation explicite. */
  async function saveConfig() {
    if (!target || target.kind !== "vfs" || !configJson) return;
    setConfigSaving(true);
    try {
      const b64 = await api.encodeCfgbinConfig(target.path, configJson, settings.gameDir);
      if (isLoose) {
        const written = await api.writeB64(target.path, b64, settings.gameDir);
        toast.success(`Config écrite en place (${humanSize(written)}) → ${target.path}`);
      } else {
        const ok = await confirm(
          `Ceci écrase directement « ${target.path} » dans le dossier du jeu (override loose d'un fichier normalement empaqueté).\n\n` +
            "⚠️ Comportement réel de nie.exe NON CONFIRMÉ par rétro-ingénierie (le jeu peut ignorer ce fichier) — Easy Anti-Cheat est présent sur cette installation. Continuer ?",
          { title: "Écrire en override loose", kind: "warning" },
        );
        if (!ok) return;
        const written = await api.writeLooseOverrideB64(target.path, b64, settings.gameDir);
        toast.success(`Override loose écrit (${humanSize(written)}) → ${target.path}`);
      }
    } catch (e) {
      toast.error(String(e));
    } finally {
      setConfigSaving(false);
    }
  }

  async function saveConfigAs() {
    if (!target || !configJson) return;
    const dest = await save({ defaultPath: name });
    if (!dest) return;
    setConfigSaving(true);
    try {
      const b64 = await api.encodeCfgbinConfig(target.path, configJson, settings.gameDir);
      const written = await api.saveBytesB64(dest, b64);
      toast.success(`${humanSize(written)} écrits → ${dest}`);
    } catch (e) {
      toast.error(String(e));
    } finally {
      setConfigSaving(false);
    }
  }

  async function loadAudio() {
    if (!target || target.kind !== "vfs") return;
    setAudioLoading(true);
    setAudioError(null);
    try {
      const b64 = await api.audioPreviewB64(target.path, settings.gameDir);
      setAudioUrl(`data:audio/wav;base64,${b64}`);
    } catch (e) {
      setAudioError(String(e));
    } finally {
      setAudioLoading(false);
    }
  }

  async function loadVideo() {
    if (!target || target.kind !== "vfs") return;
    setVideoLoading(true);
    setVideoError(null);
    try {
      const b64 = await api.videoPreviewB64(target.path, settings.gameDir);
      setVideoUrl(`data:video/mp4;base64,${b64}`);
    } catch (e) {
      setVideoError(String(e));
    } finally {
      setVideoLoading(false);
    }
  }

  async function openBlender() {
    if (!target || target.kind !== "vfs") return;
    setBusy(true);
    try {
      const msg = await api.openInBlender(target.path, settings.blenderExe, settings.gameDir);
      toast.success(msg);
    } catch (e) {
      toast.error(String(e));
    } finally {
      setBusy(false);
    }
  }

  if (!target) {
    return (
      <div className="flex h-full items-center justify-center type-body-medium text-on-surface-variant">
        Sélectionnez un fichier pour l'aperçu.
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col gap-3 p-4">
      <div>
        <div className="flex items-center gap-2">
          <h3 className="truncate type-title-small text-on-surface" title={target.path}>
            {resolvedName?.name ?? name}
          </h3>
          {resolvedName && (
            <Badge variant="secondary" className="capitalize">
              {resolvedName.kind === "chara" ? "personnage" : resolvedName.kind === "skill" ? "technique" : "objet"}
            </Badge>
          )}
          {target.kind === "disk" && <Badge variant="secondary">fichier externe</Badge>}
          {target.kind === "raw_cpk" && <Badge variant="secondary">CPK brut</Badge>}
          {isLoose && (
            <Badge variant="outline" title="Physiquement sur disque (pas dans un CPK) — modifiable en place">
              loose · modifiable en place
            </Badge>
          )}
        </div>
        {resolvedName && <p className="truncate type-body-small text-on-surface-variant">{resolvedName.extra ?? name}</p>}
        <p className="truncate type-body-small text-on-surface-variant" title={target.path}>
          {target.path}
        </p>
      </div>

      <div className="flex flex-wrap gap-2">
        <Button size="sm" variant="outline" onClick={extract} disabled={busy}>
          Extraire vers…
        </Button>
        {/* Export CONVERTI : « Extraire » écrit les octets du jeu tels quels (un `.g4tx` reste un
          * `.g4tx`) ; ici on choisit le format de sortie. La liste ne contient que ce qui marche
          * réellement pour ce fichier — elle est calculée côté Rust (`export.rs`), pas devinée
          * ici. */}
        {exportFormats.length > 1 && (
          <div className="flex items-center gap-1">
            <Select value={exportFormat} onValueChange={(v) => setExportFormat(v ?? "")}>
              <SelectTrigger size="sm" className="w-44">
                <SelectValue placeholder="Format…" />
              </SelectTrigger>
              <SelectContent>
                {exportFormats.map((f) => (
                  <SelectItem key={f.id} value={f.id}>
                    {f.brut ? "Brut" : f.ext.toUpperCase()}
                    {f.sans_perte ? "" : " (avec perte)"}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <Button
              size="sm"
              variant="outline"
              onClick={exportAs}
              disabled={busy || !exportFormat}
              title={exportFormats.find((f) => f.id === exportFormat)?.label}
            >
              Exporter sous…
            </Button>
          </div>
        )}
        <Button size="sm" variant="outline" onClick={copyPath}>
          Copier le chemin
        </Button>
        {target.kind === "vfs" && BLENDER_EXTS.has(ext) && (
          <Button size="sm" variant="outline" onClick={openBlender} disabled={busy}>
            🧊 Ouvrir dans Blender
          </Button>
        )}
        {target.kind === "vfs" && ext === "usm" && !videoUrl && (
          <Button size="sm" variant="outline" onClick={loadVideo} disabled={videoLoading}>
            {videoLoading ? "Remuxage ffmpeg…" : "▶️ Aperçu vidéo"}
          </Button>
        )}
        {target.kind === "vfs" && ["acb", "awb", "hca", "adx"].includes(ext) && !audioUrl && (
          <Button size="sm" variant="outline" onClick={loadAudio} disabled={audioLoading}>
            {audioLoading ? "Décodage HCA/ADX…" : "🔊 Aperçu audio"}
          </Button>
        )}
        {target.kind === "vfs" && isCfgBin && !configJson && (
          <Button size="sm" variant="outline" onClick={loadConfig} disabled={configLoading}>
            {configLoading ? "Décodage…" : "🗂️ Décoder (config)"}
          </Button>
        )}
      </div>

      {configError && <p className="type-body-small text-error">{configError}</p>}
      {configJson && (
        <div className="flex flex-col gap-1">
          <div className="flex items-center justify-between gap-2">
            <p className="type-label-small text-on-surface-variant">
              {configFormat === "t2b"
                ? "Décodé (T2B → JSON) — éditable, ré-encodable (encode_t2b, vérifié par round-trip réel)."
                : "Décodé (RDBN → JSON) — éditable (valeurs uniquement, pas de champs ajoutés/retirés), ré-encodable (encode_rdbn, vérifié par round-trip réel)."}
            </p>
            <Button
              size="sm"
              variant="ghost"
              onClick={async () => {
                await writeText(configJson);
                toast.success("JSON copié");
              }}
            >
              Copier
            </Button>
          </div>
          {/* Même chaîne JSON que l'éditeur texte, projetée en table/arbre — cf. `CfgbinViewer`.
           * L'édition RDBN suppose `target.kind === "vfs"` (`encode_cfgbin_config` relit
           * l'original comme gabarit), garanti par `loadConfig` et redit ici. */}
          {/* Famille reconnue : les champs portent leur nom au lieu d'un numéro de colonne.
            * La vue générique reste dessous — c'est elle qui est éditable et ré-encodable. */}
          {configFamille ? (
            <details className="rounded-lg border border-app-line bg-app-box/60" open>
              <summary className="cursor-pointer px-3 py-2 text-xs text-ink-dull">
                Données nommées — famille <span className="text-ink">{configFamille}</span>
              </summary>
              <pre className="max-h-72 overflow-auto px-3 pb-3 font-mono text-[11px] leading-relaxed">
                {configTypedJson}
              </pre>
            </details>
          ) : (
            <p className="text-xs text-ink-faint">
              Aucun parseur nommé pour cette famille — vue générique seule.
            </p>
          )}
          <Suspense fallback={<p className="p-3 text-xs text-ink-faint">Chargement de l’éditeur de configuration…</p>}>
            <CfgbinViewer
              value={configJson}
              onChange={setConfigJson}
              format={configFormat ?? "t2b"}
              readOnly={target.kind !== "vfs"}
            />
          </Suspense>
          <div className="flex flex-wrap gap-2">
            <Button size="sm" variant="outline" onClick={saveConfigAs} disabled={configSaving}>
              Enregistrer sous…
            </Button>
            {target.kind === "vfs" && (
              <Button size="sm" variant={isLoose ? "default" : "destructive"} onClick={saveConfig} disabled={configSaving}>
                {isLoose ? "Enregistrer en place" : "⚠️ Enregistrer en override loose"}
              </Button>
            )}
          </div>
        </div>
      )}

      {target.kind === "vfs" && (ext === "g4md" || ext === "g4mg") && (
        <ModelPreview key={`${settings.gameDir}:${target.path}`} path={target.path} gameDir={settings.gameDir} />
      )}

      {audioError && <p className="type-body-small text-error">{audioError}</p>}
      {audioUrl && (
        // eslint-disable-next-line jsx-a11y/media-has-caption
        <audio src={audioUrl} controls className="w-full" />
      )}

      {videoError && <p className="type-body-small text-error">{videoError}</p>}
      {videoUrl && (
        // eslint-disable-next-line jsx-a11y/media-has-caption
        <video src={videoUrl} controls className="max-h-72 w-full rounded-xl border border-app-line bg-black" />
      )}

      {target.kind !== "raw_cpk" && (
        <div className="flex flex-wrap items-center gap-2">
          {/* `Select` du design system (base-ui) — l'ancien `<select>` HTML brut était le seul
           * contrôle de l'app à ne pas suivre la palette ni le clavier des autres listes. */}
          <Select value={modChoice} onValueChange={(v) => setModChoice(v ?? "")}>
            <SelectTrigger size="sm" className="w-52">
              <SelectValue placeholder="Choisir un mod…" />
            </SelectTrigger>
            <SelectContent>
              {mods.map((m) => (
                <SelectItem key={m.id} value={m.id}>
                  {m.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          {/* Sans mod existant, l'ancien panneau ne montrait RIEN ici : impossible d'ajouter un
           * fichier à un mod tant qu'on n'était pas passé par l'onglet Mods. Le mod est créé à la
           * volée, comme le fait déjà Ctrl+V (`ExplorerView.doPaste`). */}
          {mods.length === 0 && (
            <Button size="sm" variant="outline" onClick={createModAndStage} disabled={busy}>
              Créer un mod et y ajouter ce fichier…
            </Button>
          )}
          <Button size="sm" variant="outline" onClick={stageIntoMod} disabled={!modChoice || busy}>
            Remplacer par un fichier…
          </Button>
          {ext === "g4tx" && (
            <Button
              size="sm"
              variant="outline"
              onClick={stageTexture}
              disabled={!modChoice || busy}
              title="Décode le PNG choisi et le réencode en DDS/G4TX — pris en charge uniquement pour les textures mono-image sans atlas (ex. les icônes/UI simples, pas les atlas multi-région)"
            >
              🖼️ Remplacer par un PNG…
            </Button>
          )}
        </div>
      )}

      <Separator />

      {error && <p className="type-body-small text-error">{error}</p>}

      {pngUrl && (
        <div className="max-h-64 overflow-auto rounded-lg border border-app-line bg-app-dark-box p-2">
          <img src={pngUrl} alt={name} className="max-w-full" />
        </div>
      )}

      {/* Un `.g4tx` n'est pas UNE image : la planche montre toutes ses textures nommées, là où
        * l'aperçu ci-dessus ne rend que celle que le nom de fichier désigne. */}
      {target.kind === "vfs" && ext === "g4tx" && <TextureSheet path={target.path} />}

      {/* Idem pour une banque : `audioPreviewB64` rend une piste, la banque en décrit jusqu'à 1 512. */}
      {target.kind === "vfs" && (ext === "acb" || ext === "awb") && <AudioBankPanel path={target.path} />}

      {/* Une caméra de cinématique est une collection de canaux échantillonnés (position, point
        * visé, champ de vision) : la tracer sur une échelle de frames commune rend une
        * trajectoire lisible, là où le JSON du décodeur demande de reconstituer mentalement
        * des milliers de nombres. */}
      {target.kind === "vfs" && ext === "g4cm" && <CameraTrackView path={target.path} />}

      {/* Un navmesh se lit en plan, vue de dessus : c'est la seule projection où la topologie
        * d'une zone marchable apparaît d'un coup d'œil. */}
      {target.kind === "vfs" && ext === "g4nv" && <NavmeshView path={target.path} />}

      <ScrollArea className="min-h-0 flex-1 rounded-lg border border-app-line bg-app-dark-box">
        <pre className="whitespace-pre-wrap p-3 font-mono text-xs leading-relaxed text-on-surface">{lines.join("\n") || "…"}</pre>
      </ScrollArea>

      <Tabs defaultValue="hex" className="shrink-0">
        <div className="flex items-center justify-between">
          <TabsList>
            <TabsTrigger value="hex" className="type-label-large state-layer" onClick={loadRaw}>
              Hex (lecture)
            </TabsTrigger>
            <TabsTrigger value="edit" className="type-label-large state-layer" onClick={loadRaw}>
              Éditer (copie)
            </TabsTrigger>
          </TabsList>
          {busy && <span className="type-label-small text-on-surface-variant">chargement…</span>}
        </div>
        <TabsContent value="hex">
          <ScrollArea className="h-40 rounded-lg border border-app-line bg-app-dark-box">
            <pre className="p-2 font-mono text-[11px] leading-relaxed text-on-surface">
              {rawBytes ? hexLines(rawBytes).join("\n") : "(cliquez pour charger)"}
            </pre>
          </ScrollArea>
        </TabsContent>
        <TabsContent value="edit" className="space-y-2">
          <textarea
            className="h-40 w-full resize-none rounded-lg border border-app-line bg-app-box p-2 font-mono text-[11px] leading-relaxed text-on-surface outline-none"
            spellCheck={false}
            value={editText}
            onChange={(e) => setEditText(e.target.value)}
            placeholder="(cliquez sur l'onglet pour charger les octets)"
          />
          <p className="type-body-small text-on-surface-variant">
            {isLoose
              ? "Fichier « loose » (physiquement sur disque, pas dans un CPK) : peut être écrasé EN PLACE."
              : "Empaqueté dans un CPK — aucun encodeur CPK dans nie-formats, « Enregistrer » exporte toujours un fichier externe."}
          </p>
          <div className="flex flex-wrap gap-2">
            <Button size="sm" onClick={saveEdited} disabled={!editText}>
              Enregistrer la copie modifiée sous…
            </Button>
            {target.kind === "vfs" && isLoose && (
              <Button size="sm" variant="destructive" onClick={saveInPlace} disabled={!editText || busy}>
                ⚠️ Enregistrer EN PLACE
              </Button>
            )}
          </div>
        </TabsContent>
      </Tabs>
    </div>
  );
}
