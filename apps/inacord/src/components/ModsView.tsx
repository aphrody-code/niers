import { useEffect, useState } from "react";
import { toast } from "sonner";
import { save } from "@tauri-apps/plugin-dialog";
import { modsDb, type ModFileRow, type ModRow } from "@/lib/modsDb";
import { deleteModWorkspace, exportMod, removeStagedFile, stageReplacement } from "@/lib/modWorkspace";
import { api } from "@/lib/api";
import { useSettings } from "@/lib/settings";
import { humanSize } from "@/lib/bytes";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { Switch } from "@/components/ui/switch";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { RenameInput } from "@/components/ui/rename-input";
import { Icon } from "@/components/ui/Icon";

export function ModsView({ onOpenFile }: { onOpenFile: (path: string) => void }) {
  const settings = useSettings();
  const [mods, setMods] = useState<ModRow[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [files, setFiles] = useState<ModFileRow[]>([]);
  const [newOpen, setNewOpen] = useState(false);
  const [newName, setNewName] = useState("");
  const [newDesc, setNewDesc] = useState("");
  const [vfsPathToStage, setVfsPathToStage] = useState("");
  const [busy, setBusy] = useState(false);
  /** Mod dont le nom est en cours d'édition en ligne (double-clic) — `null` = aucun. */
  const [renaming, setRenaming] = useState<string | null>(null);

  async function refresh() {
    setMods(await modsDb.listMods());
  }

  useEffect(() => {
    refresh();
  }, []);

  useEffect(() => {
    if (selected) modsDb.listFiles(selected).then(setFiles);
    else setFiles([]);
  }, [selected]);

  async function createMod() {
    if (!newName.trim()) return;
    const id = await modsDb.createMod(newName.trim(), newDesc.trim());
    setNewName("");
    setNewDesc("");
    setNewOpen(false);
    await refresh();
    setSelected(id);
  }

  async function toggle(mod: ModRow) {
    await modsDb.setEnabled(mod.id, !mod.enabled);
    await refresh();
  }

  /** Renomme un mod en ligne (double-clic sur son nom) — cf. `RenameInput`, comportement porté de
   * `packages/explorer/RenameInput.tsx` (spacedrive). Conserve la description existante. */
  async function renameMod(mod: ModRow, newName: string) {
    await modsDb.rename(mod.id, newName, mod.description);
    await refresh();
    toast.success(`Mod renommé « ${newName} »`);
  }

  async function del(mod: ModRow) {
    setBusy(true);
    try {
      const list = await modsDb.listFiles(mod.id);
      await deleteModWorkspace(mod.id, list);
      if (selected === mod.id) setSelected(null);
      await refresh();
      toast.success(`Mod « ${mod.name} » supprimé`);
    } finally {
      setBusy(false);
    }
  }

  async function stage() {
    if (!selected || !vfsPathToStage.trim()) return;
    setBusy(true);
    try {
      const ok = await stageReplacement(selected, { kind: "vfs", path: vfsPathToStage.trim() }, settings.gameDir);
      if (ok) {
        setVfsPathToStage("");
        setFiles(await modsDb.listFiles(selected));
        await refresh();
        toast.success("Fichier de remplacement ajouté");
      }
    } catch (e) {
      toast.error(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function unstage(f: ModFileRow) {
    await removeStagedFile(f.id, f.staged_file, f.original_file);
    if (selected) setFiles(await modsDb.listFiles(selected));
    await refresh();
  }

  async function doExport(intoGameDir: boolean) {
    if (!selected) return;
    setBusy(true);
    try {
      const dest = await exportMod(files, { intoGameDir, gameDir: settings.gameDir });
      if (dest) toast.success(`Exporté → ${dest}`);
    } catch (e) {
      toast.error(String(e));
    } finally {
      setBusy(false);
    }
  }

  /** Exporte le mod en `.cpk` autonome (§1.2 roadmap) — non chiffré/non compressé, cf. doc Rust
   * `nie_formats::cpk_encode`. Distinct de `doExport` (fichiers loose préservant l'arborescence
   * VFS) : produit une VRAIE archive rechargeable par `open_raw_cpk`/`RawCpkView`. */
  async function doExportCpk() {
    if (!selected || files.length === 0) return;
    const dest = await save({ defaultPath: `${current?.name ?? "mod"}.cpk`, filters: [{ name: "CPK", extensions: ["cpk"] }] });
    if (!dest) return;
    setBusy(true);
    try {
      const cpkFiles = files.map((f) => ({ vfs_path: f.vfs_path, staged_appdata_rel: f.staged_file }));
      const n = await api.exportModAsCpk(cpkFiles, dest);
      toast.success(`${humanSize(n)} écrits → ${dest}`);
    } catch (e) {
      toast.error(String(e));
    } finally {
      setBusy(false);
    }
  }

  const current = mods.find((m) => m.id === selected) ?? null;

  return (
    <div className="grid h-full grid-cols-[320px_1fr] gap-3 p-3">
      <div className="flex min-h-0 flex-col gap-2">
        <Dialog open={newOpen} onOpenChange={setNewOpen}>
          <DialogTrigger render={<Button size="sm">+ Nouveau mod</Button>} />
          <DialogContent>
            <DialogHeader>
              <DialogTitle>Nouveau mod</DialogTitle>
            </DialogHeader>
            <div className="space-y-3">
              <div className="space-y-1.5">
                <Label>Nom</Label>
                <Input value={newName} onChange={(e) => setNewName(e.target.value)} />
              </div>
              <div className="space-y-1.5">
                <Label>Description</Label>
                <Textarea value={newDesc} onChange={(e) => setNewDesc(e.target.value)} />
              </div>
            </div>
            <DialogFooter>
              <Button onClick={createMod} disabled={!newName.trim()}>
                Créer
              </Button>
            </DialogFooter>
          </DialogContent>
        </Dialog>

        <ScrollArea className="min-h-0 flex-1 rounded-2xl border border-app-line bg-app-dark-box">
          <div className="divide-y divide-app-line">
            {/* Rangée = `<div role="button">`, PAS un `<button>` : elle contient un `Switch`
             * (lui-même un `<button>`) et, en renommage, un `<input>`. Un bouton imbriqué dans un
             * bouton est du HTML invalide — le navigateur restructure le DOM et le contrôle
             * intérieur cesse de recevoir ses clics/son focus (l'interrupteur d'activation et le
             * champ de renommage étaient inutilisables). */}
            {mods.map((m) => (
              <div
                key={m.id}
                role="button"
                tabIndex={0}
                onClick={() => setSelected(m.id)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    setSelected(m.id);
                  }
                }}
                className={`state-layer flex w-full cursor-pointer items-start justify-between gap-2 px-3 py-2 text-left type-body-medium ${
                  selected === m.id ? "bg-secondary-container text-on-secondary-container" : "text-on-surface"
                }`}
              >
                <span className="min-w-0 flex-1">
                  {renaming === m.id ? (
                    <RenameInput
                      name={m.name}
                      onSave={async (n) => {
                        await renameMod(m, n);
                        setRenaming(null);
                      }}
                      onCancel={() => setRenaming(null)}
                    />
                  ) : (
                    <span
                      className="block truncate type-title-small"
                      onDoubleClick={(e) => {
                        e.stopPropagation();
                        setRenaming(m.id);
                      }}
                      title="Double-clic pour renommer"
                    >
                      {m.name}
                    </span>
                  )}
                  <span className="type-label-small text-on-surface-variant">{m.file_count} fichier(s)</span>
                </span>
                <span className="flex shrink-0 items-center gap-1">
                  <Button
                    size="icon-sm"
                    variant="ghost"
                    title="Renommer"
                    aria-label="Renommer"
                    onClick={(e) => {
                      e.stopPropagation();
                      setRenaming(m.id);
                    }}
                  >
                    <Icon name="edit" size={14} />
                  </Button>
                  <Switch
                    checked={!!m.enabled}
                    onCheckedChange={() => toggle(m)}
                    onClick={(e) => e.stopPropagation()}
                    title={m.enabled ? "Désactiver le mod" : "Activer le mod"}
                    aria-label={m.enabled ? "Désactiver le mod" : "Activer le mod"}
                  />
                </span>
              </div>
            ))}
            {mods.length === 0 && <p className="p-3 type-body-medium text-on-surface-variant">Aucun mod pour l'instant.</p>}
          </div>
        </ScrollArea>
      </div>

      <div className="min-h-0">
        {!current ? (
          <div className="flex h-full items-center justify-center type-body-medium text-on-surface-variant">
            Sélectionnez ou créez un mod.
          </div>
        ) : (
          <div className="flex h-full flex-col gap-3">
            <Card>
              <CardHeader>
                <CardTitle className="flex items-center justify-between">
                  {renaming === current.id ? (
                    <RenameInput
                      name={current.name}
                      onSave={async (n) => {
                        await renameMod(current, n);
                        setRenaming(null);
                      }}
                      onCancel={() => setRenaming(null)}
                    />
                  ) : (
                    <span onDoubleClick={() => setRenaming(current.id)} title="Double-clic pour renommer">
                      {current.name}
                    </span>
                  )}
                  <Button size="sm" variant="destructive" onClick={() => del(current)} disabled={busy}>
                    Supprimer le mod
                  </Button>
                </CardTitle>
              </CardHeader>
              <CardContent className="space-y-2">
                {current.description && <p className="type-body-medium text-on-surface-variant">{current.description}</p>}
                <div className="flex gap-2">
                  <Input
                    placeholder="Chemin VFS à remplacer (ex. data/dx11/chr/.../c01000100.g4tx)"
                    value={vfsPathToStage}
                    onChange={(e) => setVfsPathToStage(e.target.value)}
                  />
                  <Button size="sm" onClick={stage} disabled={busy || !vfsPathToStage.trim()}>
                    Choisir le remplacement…
                  </Button>
                </div>
                <p className="type-body-small text-on-surface-variant">
                  Astuce : dans l'Explorateur, ouvrez le fichier visé puis « Extraire vers… » pour obtenir une base à
                  éditer avant de la sélectionner ici.
                </p>
              </CardContent>
            </Card>

            <ScrollArea className="min-h-0 flex-1 rounded-2xl border border-app-line bg-app-dark-box">
              <div className="divide-y divide-app-line">
                {files.map((f) => (
                  <div key={f.id} className="flex items-center justify-between gap-2 px-3 py-2 type-body-medium">
                    <button
                      className="state-layer min-w-0 truncate rounded px-1 text-left text-on-surface hover:underline"
                      onClick={() => onOpenFile(f.vfs_path)}
                      title={f.vfs_path}
                    >
                      {f.vfs_path}
                    </button>
                    <span className="flex shrink-0 items-center gap-2 type-label-small text-on-surface-variant">
                      {f.staged_size != null && humanSize(f.staged_size)}
                      <Button
                        size="icon-sm"
                        variant="ghost"
                        onClick={() => unstage(f)}
                        title="Retirer du mod (le fichier part à la Corbeille)"
                        aria-label="Retirer du mod"
                      >
                        <Icon name="close" size={14} />
                      </Button>
                    </span>
                  </div>
                ))}
                {files.length === 0 && <p className="p-3 type-body-medium text-on-surface-variant">Aucun fichier remplacé.</p>}
              </div>
            </ScrollArea>

            <Alert>
              <AlertTitle>Export</AlertTitle>
              <AlertDescription>
                L'export « fichiers loose » et « override loose » n'écrivent jamais dans un pack du jeu en
                place. L'export en <code>.cpk</code> autonome (§1.2) produit une VRAIE archive — mais non
                chiffrée/non compressée, vérifiée par relecture (<code>CpkReader</code>), pas par un chargement
                réel dans <code>nie.exe</code> : voir <code>nie_formats::cpk_encode</code>.
              </AlertDescription>
            </Alert>
            <div className="flex flex-wrap gap-2">
              <Button variant="outline" onClick={() => doExport(false)} disabled={busy || files.length === 0}>
                Exporter vers un dossier…
              </Button>
              <Button variant="outline" onClick={doExportCpk} disabled={busy || files.length === 0}>
                📦 Exporter en .cpk…
              </Button>
              <Button variant="outline" onClick={() => doExport(true)} disabled={busy || files.length === 0}>
                ⚠️ Exporter dans le dossier du jeu (overlay loose-file, non confirmé)
              </Button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
