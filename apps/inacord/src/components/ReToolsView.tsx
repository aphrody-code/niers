// Outils RE (reverse engineering) — cf. demande utilisatrice « niers est un outil de mod ET de
// re, garde ça en tête ». Deux sources : la base `var/niers.sqlite` (statique, HORS LIGNE, 248 Mo
// — ~113 000 fonctions labellisées + ~3 150 classes RTTI + ~507 000 xrefs, produites par `nie-re`
// en analysant un dump mémoire déjà capturé et le binaire désassemblé) et l'onglet **Live**
// (`nie-trace`, lecture SEULE de la mémoire vivante de `nie.exe`/`nie_eacpatched.exe` — décision
// utilisatrice tranchée, cf. `ROADMAP.md` §4.3/§5 et `src-tauri/src/re_trace.rs`).
import { useEffect, useState } from "react";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { open } from "@tauri-apps/plugin-dialog";
import { toast } from "sonner";
import { useSettings } from "@/lib/settings";
import {
  reDb,
  defaultReDbPath,
  toStaticHex,
  type FunctionRow,
  type RttiClassRow,
  type StatutProduction,
  type XrefRow,
} from "@/lib/reDb";
import { api, type ReDumpHit, type ReDumpInfo, type ReTraceDumpStats, type ReTraceProcess, type ReTraceRegion } from "@/lib/api";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { ReForgeView } from "@/components/ReForgeView";

type SubTab = "functions" | "classes" | "forge" | "live" | "aob";

/** Onglet « Live » — lecture directe de la mémoire du process en cours (lecture seule, jamais
 * d'écriture). Détection du process → liste des plages du module principal → lecture ponctuelle
 * d'octets à une adresse, ou dump complet des plages lisibles vers `AppData/re-dumps/`. */
function ReLiveView() {
  const [proc, setProc] = useState<ReTraceProcess | null | undefined>(undefined);
  const [checking, setChecking] = useState(false);
  const [regions, setRegions] = useState<ReTraceRegion[]>([]);
  const [regionsError, setRegionsError] = useState<string | null>(null);
  const [addr, setAddr] = useState("");
  const [len, setLen] = useState("64");
  const [readResult, setReadResult] = useState<string | null>(null);
  const [readError, setReadError] = useState<string | null>(null);
  const [dumping, setDumping] = useState(false);
  const [dumpResult, setDumpResult] = useState<ReTraceDumpStats | null>(null);

  async function refresh() {
    setChecking(true);
    setRegions([]);
    setRegionsError(null);
    setDumpResult(null);
    try {
      const p = await api.reTraceFindProcess();
      setProc(p);
    } catch (e) {
      toast.error(String(e));
      setProc(null);
    } finally {
      setChecking(false);
    }
  }

  useEffect(() => {
    void refresh();
  }, []);

  async function loadRegions() {
    if (!proc) return;
    try {
      setRegions(await api.reTraceModuleRegions(proc.pid));
      setRegionsError(null);
    } catch (e) {
      setRegionsError(String(e));
    }
  }

  async function readBytes() {
    if (!proc || !addr.trim()) return;
    setReadError(null);
    setReadResult(null);
    try {
      const b64 = await api.reTraceReadBytesB64(proc.pid, addr.trim(), Number(len) || 64);
      const bin = atob(b64);
      let hex = "";
      for (let i = 0; i < bin.length; i++) hex += bin.charCodeAt(i).toString(16).padStart(2, "0") + (i % 16 === 15 ? "\n" : " ");
      setReadResult(hex.trim());
    } catch (e) {
      setReadError(String(e));
    }
  }

  async function dumpModule() {
    if (!proc) return;
    setDumping(true);
    try {
      setDumpResult(await api.reTraceDumpModule(proc.pid));
      toast.success("Dump terminé");
    } catch (e) {
      toast.error(String(e));
    } finally {
      setDumping(false);
    }
  }

  return (
    <div className="flex h-full flex-col gap-3 overflow-auto p-3">
      <Alert>
        <AlertTitle>Lecture seule</AlertTitle>
        <AlertDescription>
          Cet onglet lit la mémoire vivante du process — aucune écriture. RE single-player offline d'un jeu possédé (accord RG-L5-VR-2026-001).
        </AlertDescription>
      </Alert>

      <div className="flex items-center gap-2">
        <Button size="sm" variant="outline" onClick={() => void refresh()} disabled={checking}>
          {checking ? "Recherche…" : "Rafraîchir"}
        </Button>
        {proc === undefined ? (
          <span className="type-body-small text-on-surface-variant">…</span>
        ) : proc === null ? (
          <span className="type-body-small text-on-surface-variant">Process introuvable — le jeu n'est pas lancé.</span>
        ) : (
          <span className="type-body-small text-on-surface">
            <Badge variant="outline">{proc.process_name}</Badge> pid {proc.pid}
            {proc.module_base && <span className="text-on-surface-variant"> · base {proc.module_base}</span>}
          </span>
        )}
      </div>

      {proc && (
        <>
          <div className="flex items-center gap-2">
            <Button size="sm" variant="outline" onClick={() => void loadRegions()}>
              Lister les plages du module
            </Button>
            <Button size="sm" variant="outline" onClick={() => void dumpModule()} disabled={dumping}>
              {dumping ? "Dump…" : "Dumper les plages lisibles"}
            </Button>
          </div>
          {regionsError && <p className="type-body-small text-error">{regionsError}</p>}
          {dumpResult && (
            <p className="type-body-small text-on-surface-variant">
              {dumpResult.regions} plage(s), {(dumpResult.bytes ?? 0).toLocaleString("fr-FR")} o → {dumpResult.out_dir}
            </p>
          )}
          {regions.length > 0 && (
            <ScrollArea className="max-h-40 rounded-2xl border border-app-line bg-app-dark-box">
              <div className="divide-y divide-app-line">
                {regions.map((r, i) => (
                  <button
                    key={i}
                    className="state-layer flex w-full items-center gap-2 px-3 py-1 text-left font-mono type-label-small text-on-surface"
                    onClick={() => setAddr(r.start)}
                    title="Utiliser comme adresse de lecture"
                  >
                    <span>
                      {r.start} – {r.end}
                    </span>
                    <span className="text-on-surface-variant">
                      {r.perms} · {(r.size ?? 0).toLocaleString("fr-FR")} o
                    </span>
                  </button>
                ))}
              </div>
            </ScrollArea>
          )}

          <div className="flex items-center gap-2">
            <Input placeholder="adresse 0x…" value={addr} onChange={(e) => setAddr(e.target.value)} className="w-48 font-mono" />
            <Input placeholder="octets" value={len} onChange={(e) => setLen(e.target.value)} className="w-24" />
            <Button size="sm" variant="outline" onClick={() => void readBytes()}>
              Lire
            </Button>
          </div>
          {readError && <p className="type-body-small text-error">{readError}</p>}
          {readResult && (
            <ScrollArea className="max-h-60 rounded-lg border border-app-line bg-app-box p-3">
              <pre className="whitespace-pre-wrap font-mono type-label-small text-on-surface">{readResult}</pre>
            </ScrollArea>
          )}
        </>
      )}
    </div>
  );
}

/** Onglet « Scan AOB » — recherche de motif dans un minidump `.dmp` DÉJÀ capturé (par l'onglet
 * Live, ou par n'importe quel outil qui écrit un `MiniDumpWriteDump`). Rien n'est attaché au
 * process du jeu ici : c'est un fichier lu en lecture seule. */
function ReAobView() {
  const [dmp, setDmp] = useState<string | null>(null);
  const [info, setInfo] = useState<ReDumpInfo | null>(null);
  const [motif, setMotif] = useState("");
  const [limite, setLimite] = useState("200");
  const [hits, setHits] = useState<ReDumpHit[] | null>(null);
  const [tronque, setTronque] = useState(false);
  const [scanning, setScanning] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function pickDmp() {
    const path = await open({ title: "Minidump de nie.exe", filters: [{ name: "Minidump", extensions: ["dmp"] }] });
    if (typeof path !== "string") return;
    setHits(null);
    setError(null);
    setInfo(null);
    setDmp(path);
    try {
      setInfo(await api.reDumpOpen(path));
    } catch (e) {
      setError(String(e));
      setDmp(null);
    }
  }

  async function scan() {
    if (!dmp || !motif.trim()) return;
    setScanning(true);
    setError(null);
    setHits(null);
    try {
      const r = await api.reDumpScan(dmp, motif.trim(), Number(limite) || 0);
      setHits(r.hits);
      setTronque(r.tronque);
      if (r.hits.length === 0) toast.info("Aucun coup pour ce motif");
    } catch (e) {
      setError(String(e));
    } finally {
      setScanning(false);
    }
  }

  async function copy(addr: string) {
    await writeText(addr);
    toast.success("Adresse copiée");
  }

  return (
    <div className="flex h-full flex-col gap-3 overflow-auto p-3">
      <Alert>
        <AlertTitle>Fichier, pas process</AlertTitle>
        <AlertDescription>
          Le scan porte sur un minidump déjà capturé — aucune attache au jeu en cours, aucune écriture. Un scan relit tout le
          volume capturé depuis le disque : comptez plusieurs secondes, jamais un résultat instantané.
        </AlertDescription>
      </Alert>

      <div className="flex items-center gap-2">
        <Button size="sm" variant="outline" onClick={() => void pickDmp()}>
          Choisir un .dmp…
        </Button>
        {dmp && <span className="truncate font-mono type-label-small text-on-surface-variant">{dmp}</span>}
      </div>

      {info && (
        <p className="type-body-small text-on-surface-variant">
          {info.modules.length} module(s) · {info.ranges.toLocaleString("fr-FR")} plage(s) ·{" "}
          {(info.mapped_bytes ?? 0).toLocaleString("fr-FR")} o capturés
          {info.nie_base ? (
            <>
              {" · "}
              <span className="font-mono">nie.exe @ {info.nie_base}</span>
            </>
          ) : (
            " · nie.exe absent de cette capture"
          )}
        </p>
      )}

      <div className="flex items-center gap-2">
        <Input
          placeholder="motif AOB — ex. 44 8B ?? 10"
          value={motif}
          onChange={(e) => setMotif(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") void scan();
          }}
          className="flex-1 font-mono"
          disabled={!dmp}
        />
        <Input placeholder="limite" value={limite} onChange={(e) => setLimite(e.target.value)} className="w-24" disabled={!dmp} />
        <Button size="sm" variant="outline" onClick={() => void scan()} disabled={!dmp || !motif.trim() || scanning}>
          {scanning ? "Scan…" : "Scanner"}
        </Button>
      </div>
      <p className="type-label-small text-on-surface-variant">
        `??`, `?` ou `*` = joker ; les autres jetons sont des octets hexadécimaux.
      </p>

      {error && <p className="type-body-small text-error">{error}</p>}

      {hits && hits.length > 0 && (
        <>
          <p className="type-label-small text-on-surface-variant">
            {hits.length.toLocaleString("fr-FR")} coup(s){tronque && " — limite atteinte, d'autres existent au-delà"}
          </p>
          <ScrollArea className="min-h-0 flex-1 rounded-2xl border border-app-line bg-app-dark-box">
            <div className="divide-y divide-app-line">
              {hits.map((h, i) => (
                <div key={i} className="flex items-center gap-2 px-3 py-1 font-mono type-label-small text-on-surface">
                  <button className="hover:underline" onClick={() => void copy(h.va)} title="Copier l'adresse live">
                    {h.va}
                  </button>
                  {h.module && <Badge variant="outline">{h.module}</Badge>}
                  {h.rva && <span className="text-on-surface-variant">+{h.rva}</span>}
                  {h.statique && (
                    <button
                      className="ml-auto text-on-surface-variant hover:underline"
                      onClick={() => void copy(h.statique as string)}
                      title="Copier l'adresse statique (base image 0x140000000)"
                    >
                      statique {h.statique}
                    </button>
                  )}
                </div>
              ))}
            </div>
          </ScrollArea>
        </>
      )}
    </div>
  );
}

function XrefList({ title, rows, side }: { title: string; rows: XrefRow[]; side: "from_addr" | "to_addr" }) {
  return (
    <div>
      <p className="pb-1 type-label-small text-on-surface-variant">
        {title} ({rows.length})
      </p>
      <div className="flex flex-col gap-0.5">
        {rows.map((x, i) => (
          <span key={i} className="truncate font-mono type-label-small text-on-surface">
            {toStaticHex(x[side])} <span className="text-on-surface-variant">({x.kind})</span>
          </span>
        ))}
        {rows.length === 0 && <span className="type-label-small text-on-surface-variant">aucun</span>}
      </div>
    </div>
  );
}

export function ReToolsView() {
  const settings = useSettings();
  const [dbPath, setDbPath] = useState<string | null>(null);
  const [dbError, setDbError] = useState<string | null>(null);
  const [subTab, setSubTab] = useState<SubTab>("functions");
  const [query, setQuery] = useState("");
  const [functions, setFunctions] = useState<FunctionRow[]>([]);
  const [classes, setClasses] = useState<RttiClassRow[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [selectedFn, setSelectedFn] = useState<FunctionRow | null>(null);
  const [xrefs, setXrefs] = useState<{ callers: XrefRow[]; callees: XrefRow[] } | null>(null);

  // Édition des labels (§5 roadmap « l'onglet RE est 100 % lecture seule ») — id de la ligne en
  // cours de renommage (fonction OU classe, les deux listes n'ont jamais le même `id` affiché en
  // même temps puisque `subTab` bascule l'affichage) + valeur en cours de saisie.
  const [editingId, setEditingId] = useState<number | null>(null);
  const [editingValue, setEditingValue] = useState("");

  function startEdit(id: number, current: string) {
    setEditingId(id);
    setEditingValue(current);
  }

  async function commitEdit(kind: SubTab, id: number) {
    if (!dbPath) return;
    try {
      if (kind === "functions") {
        await reDb.renameFunction(dbPath, id, editingValue);
        setFunctions((prev) => prev.map((f) => (f.id === id ? { ...f, name: editingValue.trim() || null, name_source: "user-edit" } : f)));
        setSelectedFn((prev) => (prev?.id === id ? { ...prev, name: editingValue.trim() || null, name_source: "user-edit" } : prev));
      } else {
        await reDb.renameRttiClass(dbPath, id, editingValue);
        setClasses((prev) => prev.map((c) => (c.id === id ? { ...c, name: editingValue.trim() } : c)));
      }
      toast.success("Nom enregistré dans niers.sqlite");
    } catch (e) {
      toast.error(String(e));
    } finally {
      setEditingId(null);
    }
  }

  useEffect(() => {
    defaultReDbPath(settings.gameDir)
      .then((p) => {
        setDbPath(p);
        setDbError(p ? null : "var/niers.sqlite introuvable sous la racine du jeu — base RE non générée sur ce poste.");
      })
      .catch((e) => setDbError(String(e)));
  }, [settings.gameDir]);

  useEffect(() => {
    if (!dbPath) return;
    setLoading(true);
    setError(null);
    const req = subTab === "functions" ? reDb.searchFunctions(dbPath, query).then(setFunctions) : reDb.searchRttiClasses(dbPath, query).then(setClasses);
    req.catch((e) => setError(String(e))).finally(() => setLoading(false));
  }, [dbPath, subTab, query]);

  async function selectFn(f: FunctionRow) {
    setSelectedFn(f);
    setXrefs(null);
    if (!dbPath) return;
    try {
      setXrefs(await reDb.xrefsFor(dbPath, f.vaddr));
    } catch (e) {
      toast.error(String(e));
    }
  }

  async function copyAddr(vaddr: number) {
    await writeText(toStaticHex(vaddr));
    toast.success("Adresse copiée");
  }

  const tabsHeader = (
    <Tabs value={subTab} onValueChange={(v) => setSubTab(v as SubTab)}>
      <TabsList>
        <TabsTrigger value="functions">Fonctions</TabsTrigger>
        <TabsTrigger value="classes">Classes RTTI</TabsTrigger>
        <TabsTrigger value="forge">Forge</TabsTrigger>
        <TabsTrigger value="live">Live</TabsTrigger>
        <TabsTrigger value="aob">Scan AOB</TabsTrigger>
      </TabsList>
    </Tabs>
  );

  if (subTab === "live" || subTab === "aob" || subTab === "forge") {
    return (
      <div className="flex h-full flex-col gap-2 p-3">
        {tabsHeader}
        <div className="min-h-0 flex-1">
          {subTab === "live" ? <ReLiveView /> : subTab === "aob" ? <ReAobView /> : <ReForgeView />}
        </div>
      </div>
    );
  }

  // La barre d'onglets reste affichee : l'onglet « Forge » (et « Live »/« Scan AOB ») ne dependent
  // pas de `var/niers.sqlite`, les rendre inaccessibles parce que la base manque serait faux.
  if (dbError) {
    return (
      <div className="flex h-full flex-col gap-2 p-3">
        {tabsHeader}
        <Alert>
          <AlertTitle>Base RE indisponible</AlertTitle>
          <AlertDescription>{dbError}</AlertDescription>
        </Alert>
      </div>
    );
  }

  return (
    <div className="grid h-full grid-cols-[minmax(320px,1fr)_1fr] gap-3 p-3">
      <div className="flex min-h-0 flex-col gap-2">
        <div className="flex items-center gap-2">
          {tabsHeader}
          <Input
            placeholder={subTab === "functions" ? "Nom, sous-système, rôle, ou adresse 0x…" : "Nom de classe, namespace…"}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            className="flex-1"
          />
        </div>
        <p className="type-label-small text-on-surface-variant">
          {loading ? "chargement…" : subTab === "functions" ? `${functions.length.toLocaleString("fr-FR")} fonction(s)` : `${classes.length.toLocaleString("fr-FR")} classe(s)`}
        </p>
        {error && <p className="type-body-small text-error">{error}</p>}

        <ScrollArea className="min-h-0 flex-1 rounded-2xl border border-app-line bg-app-dark-box">
          <div className="divide-y divide-app-line">
            {subTab === "functions"
              ? functions.map((f) => (
                  <button
                    key={f.id}
                    className={`state-layer flex w-full flex-col items-start gap-0.5 px-3 py-2 text-left type-body-medium ${
                      selectedFn?.id === f.id ? "bg-secondary-container text-on-secondary-container" : "text-on-surface"
                    }`}
                    onClick={() => selectFn(f)}
                  >
                    <span className="flex w-full items-center gap-2">
                      <span className="truncate font-mono">{f.name ?? `FUN_${f.vaddr.toString(16)}`}</span>
                      {/* Ce que la FORGE fait de cette fonction. Le reverse répond « à quoi sert
                          ce code » ; la forge répond « le dépôt sait-il le produire ». Les deux
                          questions se posent devant la même ligne, elles s'affichent ensemble. */}
                      <BadgeForge statut={f.statut} cause={f.cause} />
                      {f.subsystem && (
                        <Badge variant="outline" className="shrink-0">
                          {f.subsystem}
                        </Badge>
                      )}
                    </span>
                    <span className="type-label-small text-on-surface-variant">
                      {toStaticHex(f.vaddr)} · conf {(f.confidence * 100).toFixed(0)}% · {f.n_calls_in} in / {f.n_calls_out} out
                    </span>
                  </button>
                ))
              : classes.map((c) => (
                  <div key={c.id} className="flex flex-col gap-0.5 px-3 py-2 type-body-medium text-on-surface">
                    {editingId === c.id ? (
                      <span className="flex items-center gap-1">
                        <Input
                          autoFocus
                          className="h-7 flex-1 font-mono"
                          value={editingValue}
                          onChange={(e) => setEditingValue(e.target.value)}
                          onKeyDown={(e) => {
                            if (e.key === "Enter") void commitEdit("classes", c.id);
                            if (e.key === "Escape") setEditingId(null);
                          }}
                        />
                        <Button size="sm" variant="ghost" onClick={() => commitEdit("classes", c.id)}>
                          ✓
                        </Button>
                      </span>
                    ) : (
                      <span className="flex items-center gap-2">
                        <span className="truncate font-mono">{c.name}</span>
                        {c.namespace && (
                          <Badge variant="outline" className="shrink-0">
                            {c.namespace}
                          </Badge>
                        )}
                        <button
                          className="ml-auto shrink-0 type-label-small text-on-surface-variant hover:underline"
                          onClick={() => startEdit(c.id, c.name)}
                          title="Renommer cette classe"
                        >
                          ✏️
                        </button>
                      </span>
                    )}
                    {c.vtable_vaddr != null && (
                      <button
                        className="w-fit type-label-small text-on-surface-variant hover:underline"
                        onClick={() => copyAddr(c.vtable_vaddr as number)}
                        title="Copier l'adresse de la vtable"
                      >
                        vtable {toStaticHex(c.vtable_vaddr)}
                      </button>
                    )}
                  </div>
                ))}
            {!loading && (subTab === "functions" ? functions.length === 0 : classes.length === 0) && (
              <p className="p-4 type-body-small text-on-surface-variant">Aucun résultat.</p>
            )}
          </div>
        </ScrollArea>
      </div>

      <div className="min-h-0 overflow-hidden rounded-2xl border border-app-line bg-app-dark-box">
        {!selectedFn ? (
          <div className="flex h-full items-center justify-center type-body-medium text-on-surface-variant">
            Sélectionnez une fonction pour voir ses xrefs.
          </div>
        ) : (
          <div className="flex h-full flex-col gap-3 p-4">
            <div>
              {editingId === selectedFn.id ? (
                <span className="flex items-center gap-1">
                  <Input
                    autoFocus
                    className="h-8 flex-1 font-mono"
                    value={editingValue}
                    onChange={(e) => setEditingValue(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") void commitEdit("functions", selectedFn.id);
                      if (e.key === "Escape") setEditingId(null);
                    }}
                  />
                  <Button size="sm" variant="ghost" onClick={() => commitEdit("functions", selectedFn.id)}>
                    ✓
                  </Button>
                </span>
              ) : (
                <h3 className="flex items-center gap-2 truncate type-title-small text-on-surface">
                  {selectedFn.name ?? `FUN_${selectedFn.vaddr.toString(16)}`}
                  <button
                    className="shrink-0 type-label-small text-on-surface-variant hover:underline"
                    onClick={() => startEdit(selectedFn.id, selectedFn.name ?? "")}
                    title="Renommer cette fonction"
                  >
                    ✏️
                  </button>
                </h3>
              )}
              <div className="mt-1 flex flex-wrap items-center gap-2">
                <button className="font-mono type-body-small text-on-surface-variant hover:underline" onClick={() => copyAddr(selectedFn.vaddr)}>
                  {toStaticHex(selectedFn.vaddr)}
                </button>
                {selectedFn.subsystem && <Badge variant="outline">{selectedFn.subsystem}</Badge>}
                {selectedFn.role && <Badge variant="outline">{selectedFn.role}</Badge>}
                {selectedFn.name_source && (
                  <Badge variant="secondary" title="Origine du nom (heuristique, RE manuel, propagation…)">
                    {selectedFn.name_source}
                  </Badge>
                )}
                <BadgeForge statut={selectedFn.statut} cause={selectedFn.cause} />
              </div>
              <p className="mt-1 type-body-small text-on-surface-variant">
                taille {selectedFn.size} octets · confiance {(selectedFn.confidence * 100).toFixed(0)}% · pagerank {selectedFn.pagerank.toFixed(4)}
              </p>
              {/* La cause, en toutes lettres : c'est elle qui dit quoi implémenter dans `nie-asm`
                  pour que ce corps précis devienne productible. */}
              {selectedFn.statut === "bloque" && selectedFn.cause && (
                <p className="mt-1 font-mono type-label-small text-amber-500" title="Ce que le relevé refuse encore">
                  forge bloquée : {selectedFn.cause}
                </p>
              )}
            </div>
            <ScrollArea className="min-h-0 flex-1 rounded-lg border border-app-line bg-app-box p-3">
              <div className="flex flex-col gap-4">
                {xrefs ? (
                  <>
                    <XrefList title="Appelants (callers)" rows={xrefs.callers} side="from_addr" />
                    <XrefList title="Appelés (callees)" rows={xrefs.callees} side="to_addr" />
                  </>
                ) : (
                  <p className="type-body-small text-on-surface-variant">chargement des xrefs…</p>
                )}
              </div>
            </ScrollArea>
            <Button size="sm" variant="outline" onClick={() => copyAddr(selectedFn.vaddr)}>
              Copier l'adresse statique
            </Button>
          </div>
        )}
      </div>
    </div>
  );
}

/**
 * Pastille d'état de production, telle que `nie-forge kb` l'a inscrite dans `forge_unit`.
 *
 * Rien n'est affiché quand la base ne porte pas cette table (base antérieure à la forge, ou base
 * tierce) : une pastille « inconnu » sur chaque ligne dirait quelque chose de la fonction alors
 * qu'elle ne dirait que l'âge du fichier. Les états non-code (`regle`, `donnees_inline`,
 * `verbatim`) ne s'affichent pas non plus ici — ce ne sont pas des fonctions.
 */
function BadgeForge({ statut, cause }: { statut?: StatutProduction; cause?: string | null }) {
  if (statut === "produit") {
    return (
      <Badge variant="outline" className="shrink-0 border-emerald-500/40 text-emerald-500" title="Le dépôt produit ce corps à l'octet près">
        produit
      </Badge>
    );
  }
  if (statut === "bloque") {
    return (
      <Badge
        variant="outline"
        className="shrink-0 border-amber-500/40 text-amber-500"
        title={cause ? `Le relevé refuse ce corps : ${cause}` : "Le relevé refuse encore ce corps"}
      >
        bloqué
      </Badge>
    );
  }
  if (statut === "hors_decoupage") {
    return (
      <Badge variant="outline" className="shrink-0 opacity-50" title="Aucune unité du découpage ne couvre cette adresse">
        hors découpage
      </Badge>
    );
  }
  return null;
}
