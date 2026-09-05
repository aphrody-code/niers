import { useEffect, useState } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { toast } from "sonner";
import { api, type RemoteRosterEntry, type SaveBlobInfo, type SaveSummary } from "@/lib/api";
import { useSettings } from "@/lib/settings";
import { b64ToBytes, hexLines, humanSize } from "@/lib/bytes";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";

function formatPlaytime(secs: number | null): string {
  if (secs == null) return "?";
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  return `${h}h${m.toString().padStart(2, "0")}`;
}

export function SaveView() {
  const settings = useSettings();
  const [summary, setSummary] = useState<SaveSummary | null>(null);
  const [blobs, setBlobs] = useState<SaveBlobInfo[]>([]);
  const [blobHex, setBlobHex] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [roster, setRoster] = useState<RemoteRosterEntry[] | null>(null);
  const [rosterLoading, setRosterLoading] = useState(false);
  const [openedPath, setOpenedPath] = useState<string | null>(null);
  const [openedAuto, setOpenedAuto] = useState(false);
  const [autoDetecting, setAutoDetecting] = useState(true);

  async function openPath(path: string, auto: boolean) {
    setBusy(true);
    setError(null);
    try {
      const s = await api.saveOpen(path);
      setSummary(s);
      setBlobs(await api.saveListBlobs());
      setBlobHex(null);
      setRoster(null);
      setOpenedPath(path);
      setOpenedAuto(auto);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function pickAndOpen() {
    const path = await open({ title: "Fichier de sauvegarde Lives" });
    if (typeof path !== "string") return;
    await openPath(path, false);
  }

  // Auto-sélection au montage (§3 roadmap « heuristique plus complète/récente, auto-sélection
  // dans SaveView au lieu d'un open() systématique ») — repli silencieux sur le sélecteur manuel
  // si Steam/le jeu/toute sauvegarde valide est absent de ce poste (pas d'erreur affichée : c'est
  // l'état normal sur une machine de dev sans le jeu installé via Steam).
  useEffect(() => {
    api
      .defaultSavePath()
      .then((path) => {
        if (path) {
          toast.success(`Sauvegarde détectée automatiquement : ${path.split(/[/\\]/).pop()}`);
          return openPath(path, true);
        }
      })
      .catch(() => {})
      .finally(() => setAutoDetecting(false));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function viewBlob(index: number) {
    try {
      const b64 = await api.saveBlobHexB64(index);
      setBlobHex(hexLines(b64ToBytes(b64), 128).join("\n"));
    } catch (e) {
      toast.error(String(e));
    }
  }

  async function resolveRoster() {
    if (!summary) return;
    setRosterLoading(true);
    try {
      const ids = summary.roster.owned.map((r) => r.id);
      const res = await api.remoteResolveRoster(settings.azaleeUrl, ids);
      setRoster(res.resolved);
      toast.success(`${res.matched}/${res.total} personnages résolus (azalee)`);
    } catch (e) {
      toast.error(String(e));
    } finally {
      setRosterLoading(false);
    }
  }

  async function exportRoundTrip() {
    const dest = await save({ defaultPath: summary?.slot_name });
    if (!dest) return;
    try {
      const n = await api.saveExport(dest);
      toast.success(`${humanSize(n)} écrits → ${dest}`);
    } catch (e) {
      toast.error(String(e));
    }
  }

  return (
    <div className="mx-auto flex max-w-3xl flex-col gap-4 p-4">
      <Alert>
        <AlertTitle>Déchiffrement local uniquement</AlertTitle>
        <AlertDescription>
          Lecture/ré-encryptage via <code>nie-save</code> (round-trip octet-identique si rien n'est
          modifié). L'édition d'octets individuels (comme <code>niers save edit</code> en CLI)
          n'est pas encore câblée ici — export en lecture/backup pour l'instant.
        </AlertDescription>
      </Alert>

      <div className="flex items-center gap-2 self-start">
        <Button onClick={pickAndOpen} disabled={busy}>
          {summary ? "Ouvrir une autre sauvegarde…" : "Ouvrir une sauvegarde…"}
        </Button>
        {autoDetecting && <span className="type-body-small text-on-surface-variant">Détection Steam Cloud…</span>}
        {!autoDetecting && openedPath && (
          <span className="truncate type-body-small text-on-surface-variant" title={openedPath}>
            📂 {openedPath.split(/[/\\]/).pop()} {openedAuto && "(détecté automatiquement)"}
          </span>
        )}
      </div>
      {error && <p className="type-body-medium text-error">{error}</p>}

      {summary && (
        <>
          <Card>
            <CardHeader>
              <CardTitle>{summary.slot_name}</CardTitle>
            </CardHeader>
            <CardContent className="grid grid-cols-2 gap-2 type-body-medium sm:grid-cols-3">
              <div>
                <div className="type-label-small text-on-surface-variant">Joueur</div>
                <div className="text-on-surface">{summary.player_name || "?"}</div>
              </div>
              <div>
                <div className="type-label-small text-on-surface-variant">Niveau</div>
                <div className="text-on-surface">{summary.level_str || "?"}</div>
              </div>
              <div>
                <div className="type-label-small text-on-surface-variant">Temps de jeu</div>
                <div className="text-on-surface">{formatPlaytime(summary.playtime_secs)}</div>
              </div>
              <div>
                <div className="type-label-small text-on-surface-variant">Slots</div>
                <div className="text-on-surface">
                  {summary.used_slots ?? "?"} / {summary.max_slots ?? "?"}
                </div>
              </div>
              <div>
                <div className="type-label-small text-on-surface-variant">Roster</div>
                <div className="flex items-center gap-2 text-on-surface">
                  {Array.isArray(summary.roster?.owned) ? summary.roster.owned.length : "?"} personnage(s)
                  <Button size="sm" variant="link" className="h-auto p-0" onClick={resolveRoster} disabled={rosterLoading}>
                    {rosterLoading ? "résolution…" : "résoudre les noms (azalee)"}
                  </Button>
                </div>
              </div>
              <div>
                <div className="type-label-small text-on-surface-variant">ID unique</div>
                <div className="truncate font-mono type-body-small text-on-surface">{summary.unique_id || "?"}</div>
              </div>
            </CardContent>
          </Card>

          {roster && (
            <Card>
              <CardHeader>
                <CardTitle>Roster résolu (bonus — azalee, wiki distant)</CardTitle>
              </CardHeader>
              <CardContent>
                <ScrollArea className="h-56 rounded-lg border border-app-line bg-app-dark-box">
                  <div className="divide-y divide-app-line">
                    {roster.map((r) => (
                      <div key={r.id} className="state-layer flex items-center justify-between px-3 py-1.5 type-body-medium">
                        <span className="text-on-surface">{r.name ?? <span className="text-on-surface-variant">{r.id} (inconnu)</span>}</span>
                        <span className="flex gap-1">
                          {r.element && <Badge variant="outline">{r.element}</Badge>}
                          {r.position && <Badge variant="outline">{r.position}</Badge>}
                        </span>
                      </div>
                    ))}
                  </div>
                </ScrollArea>
              </CardContent>
            </Card>
          )}

          <Card>
            <CardHeader>
              <CardTitle className="flex items-center justify-between">
                <span>Blobs internes</span>
                <Button size="sm" variant="outline" onClick={exportRoundTrip}>
                  Exporter (round-trip)…
                </Button>
              </CardTitle>
            </CardHeader>
            <CardContent className="space-y-2">
              <div className="flex flex-wrap gap-2">
                {blobs.map((b, i) => (
                  <Button key={b.filename} size="sm" variant="secondary" onClick={() => viewBlob(i)}>
                    {b.filename} <Badge variant="outline" className="ml-1">{humanSize(b.size)}</Badge>
                  </Button>
                ))}
              </div>
              {blobHex && (
                <ScrollArea className="h-56 rounded-lg border border-app-line bg-app-dark-box">
                  <pre className="p-2 font-mono text-[11px] leading-relaxed text-on-surface">{blobHex}</pre>
                </ScrollArea>
              )}
            </CardContent>
          </Card>
        </>
      )}
    </div>
  );
}
