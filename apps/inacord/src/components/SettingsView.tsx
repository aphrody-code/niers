import { useCallback, useEffect, useState } from "react";
import { useTheme } from "next-themes";
import { MemoireCard } from "@/components/MemoireCard";
import { open } from "@tauri-apps/plugin-dialog";
import { check as checkUpdate, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { toast } from "sonner";
import { api, type BlenderSceneResult, type McpStatus, type McpTarget, type VfsStats } from "@/lib/api";
import { vfsIndexDb, type VfsIndexMeta } from "@/lib/vfsIndexDb";
import { jobsDb } from "@/lib/jobsDb";
import {
  ACCENT_THEMES,
  getSettings,
  setSettings,
  useSettings,
  type AccentTheme,
  type Locale,
} from "@/lib/settings";
import { useT, LOCALE_LABELS } from "@/lib/i18n";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Slider } from "@/components/ui/slider";
import { Switch } from "@/components/ui/switch";
import { Progress } from "@/components/ui/progress";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

/** Libellés des variantes de palette — mêmes noms que les fichiers de
 * `var/spaceui/packages/tokens/src/css/themes/`. */
const ACCENT_THEME_LABELS: Record<AccentTheme, string> = {
  spacedrive: "Spacedrive (défaut)",
  midnight: "Midnight",
  noir: "Noir",
  slate: "Slate",
  nord: "Nord",
  mocha: "Mocha",
};

function Field({
  label,
  hint,
  value,
  placeholder,
  onChange,
  onBrowse,
}: {
  label: string;
  hint: string;
  value: string;
  placeholder: string;
  onChange: (v: string) => void;
  onBrowse: () => void;
}) {
  return (
    <div className="space-y-1.5">
      <Label>{label}</Label>
      <p className="type-body-small text-on-surface-variant">{hint}</p>
      <div className="flex gap-2">
        <Input value={value} placeholder={placeholder} onChange={(e) => onChange(e.target.value)} />
        <Button variant="outline" onClick={onBrowse}>
          Parcourir…
        </Button>
      </div>
    </div>
  );
}

export function SettingsView() {
  const settings = useSettings();
  const t = useT();
  const { theme, setTheme, resolvedTheme } = useTheme();
  const [autoGameDir, setAutoGameDir] = useState("");
  const [gameDirOk, setGameDirOk] = useState<boolean | null>(null);
  const [stats, setStats] = useState<VfsStats | null>(null);
  const [statsError, setStatsError] = useState<string | null>(null);
  const [indexMeta, setIndexMeta] = useState<VfsIndexMeta | null>(null);
  const [reindexing, setReindexing] = useState(false);
  const [reindexProgress, setReindexProgress] = useState<{ done: number; total: number } | null>(null);
  // Job `nie-tasks` en cours (cf. `vfsIndexDb.reindex`) — permet un VRAI bouton Annuler
  // (`vfsIndexDb.cancelReindex`), pas seulement un indicateur de chargement.
  const [reindexTaskId, setReindexTaskId] = useState<string | null>(null);
  const [installingBlenderAddon, setInstallingBlenderAddon] = useState(false);
  const [blenderImportBusy, setBlenderImportBusy] = useState(false);
  const [blenderImportPreview, setBlenderImportPreview] = useState<{ path: string; pngB64: string } | null>(null);
  const [sceneChara, setSceneChara] = useState("");
  const [sceneSkill, setSceneSkill] = useState("");
  const [sceneBusy, setSceneBusy] = useState(false);
  const [sceneResult, setSceneResult] = useState<BlenderSceneResult | null>(null);
  const [checkingUpdate, setCheckingUpdate] = useState(false);
  const [pendingUpdate, setPendingUpdate] = useState<Update | null>(null);
  const [installingUpdate, setInstallingUpdate] = useState(false);

  async function checkForUpdate() {
    setCheckingUpdate(true);
    try {
      const update = await checkUpdate();
      if (update?.available) {
        setPendingUpdate(update);
        toast.success(`Mise à jour ${update.version} disponible`);
      } else {
        setPendingUpdate(null);
        toast.success("niers est à jour");
      }
    } catch (e) {
      toast.error(String(e));
    } finally {
      setCheckingUpdate(false);
    }
  }

  async function installUpdate() {
    if (!pendingUpdate) return;
    setInstallingUpdate(true);
    try {
      await pendingUpdate.downloadAndInstall();
      await relaunch();
    } catch (e) {
      toast.error(String(e));
      setInstallingUpdate(false);
    }
  }

  async function installBlenderAddon() {
    setInstallingBlenderAddon(true);
    try {
      const msg = await api.installNiersBlenderAddon(settings.blenderExe, settings.gameDir);
      toast.success(msg);
    } catch (e) {
      toast.error(String(e));
    } finally {
      setInstallingBlenderAddon(false);
    }
  }

  async function importBlendFile() {
    const f = await open({ filters: [{ name: "Blender", extensions: ["blend"] }] });
    if (typeof f !== "string") return;
    setBlenderImportBusy(true);
    setBlenderImportPreview(null);
    try {
      const pngB64 = await api.blenderPreviewPngB64(f, settings.blenderExe);
      setBlenderImportPreview({ path: f, pngB64 });
    } catch (e) {
      toast.error(String(e));
    } finally {
      setBlenderImportBusy(false);
    }
  }

  async function buildSkillScene() {
    if (!sceneChara.trim() || !sceneSkill.trim()) {
      toast.error("Personnage et technique requis");
      return;
    }
    setSceneBusy(true);
    setSceneResult(null);
    try {
      // Résolution du nom libre → code interne via le GraphQL azalee (même source que
      // SearchView) — la technique, elle, est résolue SERVEUR (game_data::find_skill, local,
      // pas de round-trip réseau requis) directement par blenderBuildSkillScene.
      const r = await api.remoteSearchChara(settings.azaleeUrl, sceneChara.trim());
      const code = r.characters?.[0]?.internalCode;
      if (!code) {
        toast.error(`Aucun personnage trouvé pour « ${sceneChara} »`);
        return;
      }
      const result = await api.blenderBuildSkillScene(code, sceneSkill.trim(), settings.blenderExe, settings.gameDir);
      setSceneResult(result);
      for (const w of result.warnings) toast.warning(w);
      toast.success(`Scène construite : ${result.blend_path}`);
    } catch (e) {
      toast.error(String(e));
    } finally {
      setSceneBusy(false);
    }
  }

  function refreshIndexMeta() {
    vfsIndexDb.meta().then(setIndexMeta).catch(() => setIndexMeta(null));
  }

  useEffect(() => {
    api.defaultGameDir().then(setAutoGameDir);
    refreshIndexMeta();
  }, []);

  async function reindex() {
    setReindexing(true);
    setReindexProgress(null);
    // Journal DURABLE (§8 roadmap) : la progression ne vit plus seulement dans ce `useState` —
    // elle est écrite dans la table `jobs`, donc consultable depuis le gestionnaire d'opérations
    // (pied de barre latérale) même après avoir quitté cet écran, et un job coupé net réapparaît
    // en « interrompu » au prochain démarrage au lieu de disparaître sans laisser de trace.
    const jobId = await jobsDb.create("vfs-reindex", "Réindexation du VFS");
    // Écritures SQL limitées à ~1/s : le callback est appelé des centaines de fois (un lot de
    // 8 000 entrées côté scan, puis 400 côté insertion) — persister CHAQUE tick ferait plus de
    // travail disque que l'indexation elle-même.
    let lastWrite = 0;
    try {
      const meta = await vfsIndexDb.reindex(
        settings.gameDir,
        (done, total) => {
          setReindexProgress({ done, total });
          const now = Date.now();
          if (now - lastWrite > 1000) {
            lastWrite = now;
            void jobsDb.progress(jobId, done, total);
          }
        },
        setReindexTaskId,
      );
      setIndexMeta(meta);
      await jobsDb.progress(jobId, meta.total, meta.total);
      await jobsDb.finish(jobId, "done");
      toast.success(`Index VFS reconstruit : ${meta.total.toLocaleString("fr-FR")} fichiers`);
    } catch (e) {
      const msg = String(e);
      await jobsDb.finish(jobId, msg.includes("annul") ? "canceled" : "error", msg);
      toast.error(msg);
    } finally {
      setReindexing(false);
      setReindexProgress(null);
      setReindexTaskId(null);
    }
  }

  async function cancelReindex() {
    if (reindexTaskId) await vfsIndexDb.cancelReindex(reindexTaskId);
  }

  useEffect(() => {
    const dir = settings.gameDir || autoGameDir;
    if (!dir) return;
    api.checkGameDir(dir).then(setGameDirOk);
    api
      .stats(settings.gameDir)
      .then((s) => {
        setStats(s);
        setStatsError(null);
      })
      .catch((e) => setStatsError(String(e)));
  }, [settings.gameDir, autoGameDir]);

  return (
    <div className="mx-auto flex max-w-2xl flex-col gap-4 p-4">
      <Card>
        <CardHeader>
          <CardTitle>{t("settings.appearance")}</CardTitle>
          <CardDescription>Langue, thème, taille de police et zoom de l'interface.</CardDescription>
        </CardHeader>
        <CardContent className="space-y-5">
          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-1.5">
              <Label>{t("settings.language")}</Label>
              <Select value={settings.locale} onValueChange={(v) => setSettings({ locale: v as Locale })}>
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {(Object.keys(LOCALE_LABELS) as Locale[]).map((l) => (
                    <SelectItem key={l} value={l}>
                      {LOCALE_LABELS[l]}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>

            <div className="space-y-1.5">
              <Label>{t("settings.theme")}</Label>
              <Select value={theme ?? "dark"} onValueChange={(v) => v && setTheme(v)}>
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="light">{t("settings.theme.light")}</SelectItem>
                  <SelectItem value="dark">{t("settings.theme.dark")}</SelectItem>
                  <SelectItem value="system">{t("settings.theme.system")}</SelectItem>
                </SelectContent>
              </Select>
            </div>

            <div className="space-y-1.5">
              <Label>Palette</Label>
              {/* Variantes de `var/spaceui/packages/tokens/src/css/themes/*.css`, portées telles
               * quelles (cf. styles.css). Toutes sombres : sans effet en thème clair, où spaceui
               * ne fournit qu'une seule palette (`themes/light.css`). */}
              <Select
                value={settings.accentTheme}
                onValueChange={(v) => v && setSettings({ accentTheme: v as AccentTheme })}
              >
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {ACCENT_THEMES.map((a) => (
                    <SelectItem key={a} value={a}>
                      {ACCENT_THEME_LABELS[a]}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              {resolvedTheme === "light" && (
                <p className="type-body-small text-on-surface-variant">
                  Sans effet en thème clair — spaceui ne fournit qu'une palette claire.
                </p>
              )}
            </div>
          </div>

          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <Label>{t("settings.font_scale")}</Label>
              <span className="type-label-small text-on-surface-variant">
                {Math.round(settings.fontScale * 100)}%
              </span>
            </div>
            <Slider
              value={[settings.fontScale]}
              min={0.8}
              max={1.4}
              step={0.05}
              onValueChange={(v) => setSettings({ fontScale: Array.isArray(v) ? v[0] : v })}
            />
          </div>

          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <Label>{t("settings.ui_zoom")}</Label>
              <span className="type-label-small text-on-surface-variant">
                {Math.round(settings.uiZoom * 100)}%
              </span>
            </div>
            <Slider
              value={[settings.uiZoom]}
              min={0.7}
              max={1.5}
              step={0.05}
              onValueChange={(v) => setSettings({ uiZoom: Array.isArray(v) ? v[0] : v })}
            />
          </div>

          <Button
            variant="outline"
            size="sm"
            onClick={() => setSettings({ fontScale: 1, uiZoom: 1 })}
          >
            {t("settings.reset")}
          </Button>

          {/* Les quatre outils de spécialiste occupaient en permanence la barre latérale d'une
              application qui s'ouvre sur une médiathèque. Ils y reviennent d'un clic, et restent
              de toute façon atteignables par Ctrl+K même quand ce réglage est éteint. */}
          <div className="flex items-center justify-between border-t border-app-line pt-4">
            <div className="space-y-0.5">
              <Label htmlFor="outils-avances">Outils avancés</Label>
              <p className="text-xs text-on-surface-variant">
                Affiche Reverse-engineering, Archives Criware, Mémoire du jeu, Scripts Lua et
                Archives CPK dans la barre latérale.
              </p>
            </div>
            <Switch
              id="outils-avances"
              checked={settings.outilsAvances}
              onCheckedChange={(v) => setSettings({ outilsAvances: v })}
            />
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Répertoire du jeu</CardTitle>
          <CardDescription>
            Doit contenir <code>data/cpk_list.cfg.bin</code>.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-3">
          <Field
            label="Chemin"
            hint="Vide = auto-détection (NIE_GAME_DIR, dossier courant, puis VRAIE détection Steam — registre + bibliothèques + appmanifest_2799860.acf)."
            value={settings.gameDir}
            placeholder={autoGameDir}
            onChange={(v) => setSettings({ gameDir: v })}
            onBrowse={async () => {
              const dir = await open({ directory: true });
              if (typeof dir === "string") setSettings({ gameDir: dir });
            }}
          />
          {gameDirOk !== null && (
            <Badge variant={gameDirOk ? "default" : "destructive"}>
              {gameDirOk ? "✓ cpk_list.cfg.bin trouvé" : "✗ cpk_list.cfg.bin introuvable à cet emplacement"}
            </Badge>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Miroir wiki (recherche chara/waza)</CardTitle>
          <CardDescription>
            Fichier <code>supabase-*.sqlite</code> (miroir <code>nie-wiki</code>).
          </CardDescription>
        </CardHeader>
        <CardContent>
          <Field
            label="Base SQLite"
            hint="Vide = résolution automatique (NIE_WIKI_DB / SQLITE_DB_PATH, sinon le supabase-*.sqlite le plus récent sous <jeu>/var/wiki-mirror) — utilisée pour afficher les noms réels (perso/technique/objet) dans l'Explorateur."
            value={settings.wikiDb}
            placeholder="(auto-détecté)"
            onChange={(v) => setSettings({ wikiDb: v })}
            onBrowse={async () => {
              const f = await open({ filters: [{ name: "SQLite", extensions: ["sqlite", "db"] }] });
              if (typeof f === "string") setSettings({ wikiDb: f });
            }}
          />
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Blender (plugins/niers-blender)</CardTitle>
          <CardDescription>
            Pour « Ouvrir dans Blender » sur les modèles G4MD/G4MG/G4SK/G4MT. L'extension
            (<code>plugins/niers-blender</code>) est incluse dans ce dépôt ; clonée automatiquement si absente
            en dernier recours (installation distribuée pointée sur un simple jeu Steam).
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-3">
          <Field
            label="blender.exe"
            hint={String.raw`Vide = auto-détection (Blender Foundation\Blender 5.2\4.2\4.1\4.0).`}
            value={settings.blenderExe}
            placeholder={String.raw`C:\Program Files\Blender Foundation\Blender 5.2\blender.exe`}
            onChange={(v) => setSettings({ blenderExe: v })}
            onBrowse={async () => {
              const f = await open({ filters: [{ name: "Blender", extensions: ["exe"] }] });
              if (typeof f === "string") setSettings({ blenderExe: f });
            }}
          />
          <div className="space-y-1.5">
            <Button size="sm" variant="outline" onClick={installBlenderAddon} disabled={installingBlenderAddon}>
              {installingBlenderAddon ? "Installation…" : "🧩 Installer l'extension Blender niers"}
            </Button>
            <p className="type-body-small text-on-surface-variant">
              Installe/active <strong>vraiment</strong> l'extension dans le dossier d'addons de Blender
              (Préférences → Add-ons) — persiste au-delà d'un seul lancement, et lie sa préférence
              « Raw Data Root » au vrai dossier <code>data/</code> du jeu : un Blender ouvert
              indépendamment de nie-explorer retrouve alors squelettes partagés et pièces de personnage
              sans configuration manuelle.
            </p>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Pont Blender ↔ niers</CardTitle>
          <CardDescription>
            Importer un <code>.blend</code> existant (aperçu instantané, sans ouvrir Blender) ou construire
            une VRAIE scène — personnage + cut-in de technique, uniquement des assets réels du VFS local
            (jamais de géométrie fabriquée ; si un asset manque, c'est signalé, pas masqué).
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-5">
          <div className="space-y-1.5">
            <Label>Importer un .blend</Label>
            <div className="flex gap-2">
              <Button size="sm" variant="outline" onClick={importBlendFile} disabled={blenderImportBusy}>
                {blenderImportBusy ? "Rendu…" : "📂 Choisir un .blend"}
              </Button>
              {blenderImportPreview && (
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => api.blenderOpenScene(blenderImportPreview.path, settings.blenderExe)}
                >
                  Ouvrir dans Blender
                </Button>
              )}
            </div>
            {blenderImportPreview && (
              <img
                src={`data:image/png;base64,${blenderImportPreview.pngB64}`}
                alt={blenderImportPreview.path}
                className="max-w-full rounded-lg border border-app-line"
              />
            )}
          </div>

          <div className="space-y-1.5 border-t border-app-line pt-4">
            <Label>Construire une scène (personnage + technique)</Label>
            <div className="flex flex-wrap gap-2">
              <Input
                value={sceneChara}
                placeholder="Personnage (ex. Byron Love)"
                onChange={(e) => setSceneChara(e.target.value)}
                className="max-w-56"
              />
              <Input
                value={sceneSkill}
                placeholder="Technique (ex. Savoir suprême)"
                onChange={(e) => setSceneSkill(e.target.value)}
                className="max-w-56"
              />
              <Button size="sm" onClick={buildSkillScene} disabled={sceneBusy}>
                {sceneBusy ? "Construction…" : "🎬 Construire la scène"}
              </Button>
            </div>
            {sceneResult && (
              <div className="space-y-1.5">
                <p className="type-body-medium text-on-surface">
                  <strong>{sceneResult.skill_name}</strong> ({sceneResult.event_id_name}) →{" "}
                  <code className="type-body-small">{sceneResult.blend_path}</code>
                </p>
                {sceneResult.warnings.map((w, i) => (
                  <p key={i} className="type-body-small text-tertiary whitespace-pre-wrap">
                    ⚠ {w}
                  </p>
                ))}
                {sceneResult.preview_png_b64 && (
                  <img
                    src={`data:image/png;base64,${sceneResult.preview_png_b64}`}
                    alt={sceneResult.skill_name}
                    className="max-w-full rounded-lg border border-app-line"
                  />
                )}
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() => api.blenderOpenScene(sceneResult.blend_path, settings.blenderExe)}
                >
                  Ouvrir dans Blender
                </Button>
              </div>
            )}
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Résolveur distant azalee</CardTitle>
          <CardDescription>
            GraphQL <code>/api/graphql</code> (sans authentification) + REST <code>/api/cpk</code> /{" "}
            <code>/api/save/resolve-roster</code> — contrat réel confirmé depuis les sources du service.
            Utilisé en <strong>bonus</strong> de l'index local (personnages/techniques/roster de save) ;
            les fichiers du jeu restent toujours résolus en local d'abord. Vide = azalee.rosegriffon.fr.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <div className="space-y-1.5">
            <Label>URL de base</Label>
            <Input
              value={settings.azaleeUrl}
              placeholder="https://azalee.rosegriffon.fr"
              onChange={(e) => setSettings({ azaleeUrl: e.target.value })}
            />
          </div>
          <div className="mt-3 space-y-1.5">
            <Label>Service de modèles 3D</Label>
            <Input
              value={settings.modelServiceUrl}
              placeholder="https://cdn.rosegriffon.fr"
              onChange={(e) => setSettings({ modelServiceUrl: e.target.value })}
            />
            <p className="type-body-small text-tertiary">Avatar assemblé et rendu de menus. Vide = CDN Rose Griffon.</p>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Mises à jour</CardTitle>
          <CardDescription>
            Vérifie/télécharge/installe les nouvelles versions de niers. Endpoints (dans l'ordre) :{" "}
            <code>azalee.rosegriffon.fr/tools/niers</code> (page dédiée niers) puis, en repli, la
            dernière release GitHub (<code>latest.json</code>). Binaires signés (minisign), version
            actuelle : <Badge variant="secondary">v0.4.0</Badge>.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-3">
          <Button size="sm" onClick={checkForUpdate} disabled={checkingUpdate || installingUpdate}>
            {checkingUpdate ? "Vérification…" : "Vérifier les mises à jour"}
          </Button>
          {pendingUpdate && (
            <div className="space-y-1.5">
              <p className="type-body-medium text-on-surface">
                Version <strong>{pendingUpdate.version}</strong> disponible
                {pendingUpdate.date ? ` (${pendingUpdate.date})` : ""}.
              </p>
              {pendingUpdate.body && (
                <p className="type-body-small text-on-surface-variant whitespace-pre-wrap">{pendingUpdate.body}</p>
              )}
              <Button size="sm" variant="outline" onClick={installUpdate} disabled={installingUpdate}>
                {installingUpdate ? "Installation…" : "Télécharger et installer"}
              </Button>
            </div>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Index VFS (précision)</CardTitle>
          <CardDescription>
            Matérialise les ~255 800 fichiers du VFS dans une table SQL indexée (<code>vfs_files</code>) pour une
            résolution EXACTE par code interne — remplace le <code>.contains()</code> substring en mémoire (faux
            positifs possibles) par <code>code = ? OR code LIKE ?_%</code>. Utilisé par « Fichiers VFS liés »
            (Recherche) dès qu'il existe.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-3">
          {indexMeta ? (
            <div className="flex flex-wrap gap-2 type-body-medium">
              <Badge variant="secondary">{indexMeta.total.toLocaleString("fr-FR")} fichiers indexés</Badge>
              <Badge variant="outline">{new Date(indexMeta.reindexed_at).toLocaleString("fr-FR")}</Badge>
            </div>
          ) : (
            <p className="type-body-medium text-on-surface-variant">Pas encore construit — repli sur la recherche en mémoire.</p>
          )}
          {reindexing && (
            // `Progress` porté de `spaceui/primitives/ProgressBar.tsx` (spacedrive) sur
            // `@base-ui/react/progress`, cf. components/ui/progress.tsx.
            <Progress value={reindexProgress ? Math.round((reindexProgress.done / Math.max(reindexProgress.total, 1)) * 100) : null} />
          )}
          <div className="flex gap-2">
            <Button size="sm" onClick={reindex} disabled={reindexing}>
              {reindexing
                ? reindexProgress
                  ? `Réindexation… ${reindexProgress.done.toLocaleString("fr-FR")}/${reindexProgress.total.toLocaleString("fr-FR")}`
                  : "Scan du VFS…"
                : "Réindexer"}
            </Button>
            {reindexing && reindexTaskId && (
              <Button size="sm" variant="outline" onClick={cancelReindex}>
                Annuler
              </Button>
            )}
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Statistiques VFS</CardTitle>
        </CardHeader>
        <CardContent>
          {statsError && <p className="type-body-medium text-error">{statsError}</p>}
          {stats && (
            <div className="space-y-3">
              <div className="flex flex-wrap gap-2 type-body-medium">
                {/* Provenance d'abord : « 0 CPK » n'est une anomalie que sur une installation,
                    et c'est la normale sur un dump extrait. Sans ce badge, les compteurs
                    suivants se lisent de travers. */}
                <Badge variant={stats.montage === "dump" ? "outline" : "secondary"}>
                  {stats.montage === "dump" ? "dump extrait" : "packs CPK"}
                </Badge>
                <Badge variant="secondary">{stats.total.toLocaleString("fr-FR")} fichiers</Badge>
                {stats.montage === "packs" && (
                  <>
                    <Badge variant="secondary">{stats.cpk_count} CPK</Badge>
                    <Badge variant="secondary">{stats.extra_count} extra</Badge>
                  </>
                )}
                <Badge variant="secondary">{stats.loose_count.toLocaleString("fr-FR")} loose</Badge>
              </div>
              {stats.montage === "dump" && (
                <p className="type-body-small text-on-surface-variant">
                  Les fichiers sont servis depuis l'arborescence extraite : aucune archive à ouvrir, et
                  l'édition en place vaut pour tout le contenu — elle modifie le dump lui-même.
                </p>
              )}
              <table className="w-full type-body-small">
                <tbody>
                  {stats.top_ext.slice(0, 15).map(([e, c]) => (
                    <tr key={e} className="border-t border-app-line">
                      <td className="py-1 pr-2 font-mono text-on-surface">.{e}</td>
                      <td className="py-1 text-right text-on-surface-variant">{c.toLocaleString("fr-FR")}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </CardContent>
      </Card>

      <MemoireCard />

      <McpCard />

      <Button variant="ghost" size="sm" className="self-start" onClick={() => setSettings(getSettings())}>
        Rafraîchir
      </Button>
    </div>
  );
}

/**
 * Serveur MCP `niers-game` — l'explorateur le déclare aux clients MCP, et le laisse en retour
 * piloter cette fenêtre.
 *
 * Les deux moitiés du couple sont réunies ici : l'installation (écriture fusionnée dans la
 * config du client, côté Rust) et l'interrupteur du pont de contrôle (`@niers/bridge`).
 */
function McpCard() {
  const settings = useSettings();
  const [target, setTarget] = useState<McpTarget>("claude-code");
  const [status, setStatus] = useState<McpStatus | null>(null);
  const [busy, setBusy] = useState(false);

  const refresh = useCallback((t: McpTarget) => {
    api
      .mcpStatus(t)
      .then(setStatus)
      .catch((e) => {
        setStatus(null);
        toast.error(`État MCP indisponible : ${e}`);
      });
  }, []);

  useEffect(() => refresh(target), [refresh, target]);

  async function install() {
    setBusy(true);
    try {
      const r = await api.mcpInstall(target, settings.gameDir);
      toast.success(
        r.replaced
          ? `Serveur MCP mis à jour dans ${r.config_path}`
          : `Serveur MCP ajouté à ${r.config_path}`,
      );
      refresh(target);
    } catch (e) {
      toast.error(`Installation impossible : ${e}`);
    } finally {
      setBusy(false);
    }
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>Serveur MCP</CardTitle>
        <CardDescription>
          Expose le jeu (VFS, assets décodés, base de connaissance RE) à un assistant compatible MCP, et
          laisse celui-ci piloter cette fenêtre. Serveur et explorateur partagent les mêmes crates Rust.
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="space-y-2">
          <Label>Client à configurer</Label>
          <div className="flex gap-2">
            {(["claude-code", "claude-desktop"] as const).map((v) => (
              <Button
                key={v}
                size="sm"
                variant={target === v ? "default" : "outline"}
                onClick={() => setTarget(v)}
              >
                {v === "claude-code" ? "Claude Code (projet)" : "Claude Desktop"}
              </Button>
            ))}
          </div>
        </div>

        {status && (
          <div className="space-y-1 text-xs text-on-surface-variant">
            <div className="flex items-center gap-2">
              <Badge variant={status.installed ? "default" : "outline"}>
                {status.installed ? "installé" : "non installé"}
              </Badge>
              {!status.entrypoint_exists && <Badge variant="destructive">point d'entrée introuvable</Badge>}
            </div>
            <p className="font-mono break-all">{status.config_path}</p>
            {status.current_command && <p className="font-mono break-all">{status.current_command}</p>}
            {!status.entrypoint_exists && <p className="font-mono break-all">attendu : {status.entrypoint}</p>}
          </div>
        )}

        <Button size="sm" disabled={busy || status?.entrypoint_exists === false} onClick={install}>
          {status?.installed ? "Réinstaller" : "Installer"}
        </Button>

        <div className="flex items-center justify-between border-t border-app-line pt-4">
          <div className="space-y-0.5">
            <Label htmlFor="bridge-enabled">Pilotage à distance</Label>
            <p className="text-xs text-on-surface-variant">
              Autorise le serveur MCP à naviguer et ouvrir des fichiers dans cette fenêtre. Prend effet au
              prochain démarrage de l'application.
            </p>
          </div>
          <Switch
            id="bridge-enabled"
            checked={settings.bridgeEnabled}
            onCheckedChange={(v) => setSettings({ bridgeEnabled: v })}
          />
        </div>
      </CardContent>
    </Card>
  );
}
