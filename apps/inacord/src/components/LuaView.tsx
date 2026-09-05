// Atelier Lua — la chaîne complète autour des scripts du moteur.
//
// Le moteur Level-5 « Lives » pilote menus, scènes et événements par des scripts Lua 5.2 livrés
// UNIQUEMENT compilés (~1 100 `.lua.bin`). L'app savait jusqu'ici dire « c'est du bytecode Lua ».
// Cette vue ouvre toute la chaîne :
//
//   Catalogue → Désassemblage → Exécution → Console → Éditeur de valeurs
//
// Le point important : les scripts tournent dans la VRAIE VM du jeu (mlua, PUC-Rio Lua 5.2.4
// vendored, cf. crate `nie-lua`), pas dans une réimplémentation. Le bytecode chargé est celui que
// `nie.exe` charge.
import { useEffect, useMemo, useRef, useState } from "react";
import Editor from "@monaco-editor/react";
import { useTheme } from "next-themes";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/Icon";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { SplitPane } from "@/components/ui/split-pane";
import { Switch } from "@/components/ui/switch";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { api, type LuaChunkInfo, type LuaExecResult, type LuaGlobal, type VfsEntry } from "@/lib/api";
import { humanSize } from "@/lib/bytes";
import { useSettings } from "@/lib/settings";
import { cn } from "@/lib/utils";

type Pane = "disasm" | "source" | "console" | "values" | "api";

const PANE_LABELS: Record<Pane, string> = {
  disasm: "Désassemblage",
  source: "Source",
  console: "Console",
  values: "Valeurs",
  api: "API moteur",
};

/** Points d'entrée diffusables — mêmes noms que le cycle de vie d'Overload (`nie_lua::session`). */
const LIFECYCLE = ["OnAwake", "OnStart", "OnEnable", "OnUpdate", "OnDisable", "OnDestroy"] as const;

/** Ligne de console : ce qui a été tapé, puis ce que la VM a répondu. */
interface ConsoleLine {
  input: string;
  output: string;
}

export function LuaView() {
  const settings = useSettings();
  const { resolvedTheme } = useTheme();

  const [scripts, setScripts] = useState<VfsEntry[]>([]);
  const [scriptsLoading, setScriptsLoading] = useState(true);
  const [scriptsError, setScriptsError] = useState<string | null>(null);
  const [filter, setFilter] = useState("");

  const [selected, setSelected] = useState<string | null>(null);
  const [pane, setPane] = useState<Pane>("disasm");
  const [info, setInfo] = useState<LuaChunkInfo | null>(null);
  const [disasm, setDisasm] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  /** Source éditée — quand elle est non vide, c'est ELLE qui est exécutée, pas le `.lua.bin`.
   * C'est ce qui permet d'écrire un script à côté du jeu et de le lancer dans sa VM. */
  const [source, setSource] = useState("");
  const [useSource, setUseSource] = useState(false);
  const [withMenuHost, setWithMenuHost] = useState(true);

  const [result, setResult] = useState<LuaExecResult | null>(null);
  const [globals, setGlobals] = useState<LuaGlobal[]>([]);
  const [overrides, setOverrides] = useState<[string, string][]>([]);
  const [includeStdlib, setIncludeStdlib] = useState(false);

  const [consoleLines, setConsoleLines] = useState<ConsoleLine[]>([]);
  const [consoleInput, setConsoleInput] = useState("");
  const consoleEndRef = useRef<HTMLDivElement | null>(null);

  /** Session PERSISTANTE (thread Rust dédié) : l'état survit d'une évaluation à l'autre — c'est ce
   * qui distingue une vraie console d'un simple « exécuter et jeter ». Décochée, chaque évaluation
   * repart d'une VM neuve, ce qui reste le bon mode pour analyser un script sans le contaminer. */
  const [persistent, setPersistent] = useState(true);
  const [attached, setAttached] = useState<{ name: string; callbacks: string[] }[]>([]);
  const [apiReport, setApiReport] = useState<{
    missing: string[];
    provided: string[];
    coverage_percent: number;
  } | null>(null);

  // Catalogue des scripts du VFS.
  useEffect(() => {
    setScriptsLoading(true);
    api
      .luaListScripts(settings.gameDir)
      .then(setScripts)
      .catch((e) => setScriptsError(String(e)))
      .finally(() => setScriptsLoading(false));
  }, [settings.gameDir]);

  const shown = useMemo(() => {
    const q = filter.trim().toLowerCase();
    const list = q ? scripts.filter((s) => s.path.toLowerCase().includes(q)) : scripts;
    // Plafond d'affichage : le VFS en contient plus d'un millier, tous les peindre d'un coup
    // ralentit la vue sans rien apporter — le filtre est là pour ça.
    return list.slice(0, 500);
  }, [scripts, filter]);

  /** Argument commun à toutes les commandes : soit la source éditée, soit le chemin VFS. */
  const payload = useMemo(
    () => ({
      path: useSource ? null : selected,
      source: useSource ? source : null,
    }),
    [useSource, selected, source],
  );

  // Décodage + désassemblage à la sélection.
  useEffect(() => {
    if (!selected || useSource) return;
    setBusy(true);
    setError(null);
    Promise.all([
      api.luaChunkInfo(selected, null, settings.gameDir),
      api.luaDisassemble(selected, null, settings.gameDir),
    ])
      .then(([i, d]) => {
        setInfo(i);
        setDisasm(d);
      })
      .catch((e) => {
        setError(String(e));
        setInfo(null);
        setDisasm("");
      })
      .finally(() => setBusy(false));
  }, [selected, useSource, settings.gameDir]);

  useEffect(() => {
    consoleEndRef.current?.scrollIntoView({ block: "end" });
  }, [consoleLines]);

  async function run() {
    if (!payload.path && !payload.source) {
      toast.error("Sélectionnez un script ou écrivez une source");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const res = await api.luaExecute(payload.path, payload.source, withMenuHost, null, settings.gameDir);
      setResult(res);
      if (res.error) toast.warning("Le script s'est arrêté sur une erreur — détail dans la console");
      else toast.success(`Exécuté en ${res.duration_ms} ms`);
      setPane("console");
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function refreshGlobals() {
    setBusy(true);
    try {
      if (persistent) {
        // Session vivante : les valeurs forcées sont posées SUR l'état courant, pas rejouées
        // depuis zéro — c'est ce qui permet d'ajuster une variable puis de rediffuser `OnUpdate`.
        for (const [name, expr] of overrides) {
          if (name.trim() && expr.trim()) await api.luaSessionSetGlobal(name.trim(), expr.trim());
        }
        setGlobals(await api.luaSessionGlobals(includeStdlib));
      } else {
        if (!payload.path && !payload.source) return;
        setGlobals(
          await api.luaGlobals(
            payload.path,
            payload.source,
            withMenuHost,
            overrides,
            includeStdlib,
            settings.gameDir,
          ),
        );
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function submitConsole() {
    const expr = consoleInput.trim();
    if (!expr) return;
    setConsoleInput("");
    try {
      // En mode persistant l'expression s'évalue dans l'état COURANT (`x = 1` puis `x` répond 1) ;
      // sinon chaque évaluation repart d'une VM neuve après réexécution du script.
      const out = persistent
        ? await api.luaSessionEval(expr)
        : await api.luaEval(payload.path, payload.source, expr, withMenuHost, settings.gameDir);
      setConsoleLines((prev) => [...prev, { input: expr, output: out }]);
      if (persistent) await drainSession();
    } catch (e) {
      setConsoleLines((prev) => [...prev, { input: expr, output: String(e) }]);
    }
  }

  /** Récupère la sortie accumulée par la session (print + `Debug.*`) et l'ajoute à la console. */
  async function drainSession() {
    try {
      const d = await api.luaSessionDrain();
      const lines: ConsoleLine[] = [
        ...d.stdout.map((s) => ({ input: "", output: s })),
        ...d.logs.map((l) => ({ input: "", output: `[${l.level}] ${l.message}` })),
      ];
      if (lines.length > 0) setConsoleLines((prev) => [...prev, ...lines]);
    } catch {
      // Une session indisponible ne doit pas casser la console : le mode non persistant reste utilisable.
    }
  }

  /** Exécute dans la SESSION vivante (état conservé). */
  async function runInSession() {
    if (!payload.path && !payload.source) {
      toast.error("Sélectionnez un script ou écrivez une source");
      return;
    }
    setBusy(true);
    try {
      const returned = await api.luaSessionExec(payload.path, payload.source, settings.gameDir);
      if (returned.length > 0) {
        setConsoleLines((prev) => [...prev, { input: "", output: `→ ${returned.join("\t")}` }]);
      }
      await drainSession();
      toast.success("Exécuté dans la session");
      setPane("console");
    } catch (e) {
      setConsoleLines((prev) => [...prev, { input: "", output: String(e) }]);
      setPane("console");
    } finally {
      setBusy(false);
    }
  }

  /** Attache le script comme comportement — il doit renvoyer une table (contrat d'Overload). */
  async function attachBehaviour() {
    if (!payload.path && !payload.source) return;
    setBusy(true);
    try {
      const callbacks = await api.luaSessionAttach(payload.path, payload.source, settings.gameDir);
      const name = payload.path ?? "source éditée";
      setAttached((prev) => [...prev, { name, callbacks }]);
      toast.success(
        callbacks.length > 0
          ? `Attaché — callbacks : ${callbacks.join(", ")}`
          : "Attaché, mais ce script ne définit aucun callback de cycle de vie",
      );
      await drainSession();
    } catch (e) {
      toast.error(String(e));
    } finally {
      setBusy(false);
    }
  }

  /** Diffuse un callback à tous les comportements attachés. */
  async function broadcast(callback: string) {
    try {
      const n = await api.luaSessionBroadcast(callback);
      setConsoleLines((prev) => [
        ...prev,
        { input: `broadcast ${callback}`, output: `${n} comportement(s) ont défini ce callback` },
      ]);
      await drainSession();
    } catch (e) {
      toast.error(String(e));
    }
  }

  /** Recrée la VM et ré-attache les comportements — le `RefreshAll` d'Overload. */
  async function reloadSession() {
    try {
      await api.luaSessionReload();
      setConsoleLines((prev) => [...prev, { input: "", output: "— session rechargée (VM neuve)" }]);
      toast.success("Session rechargée");
    } catch (e) {
      toast.error(String(e));
    }
  }

  async function refreshApiReport() {
    try {
      setApiReport(await api.luaSessionApiReport());
    } catch (e) {
      toast.error(String(e));
    }
  }

  return (
    <SplitPane
      axis="x"
      side="start"
      defaultSize={280}
      min={200}
      max={520}
      storageKey="lua-catalog"
      className="h-full"
      panel={
        <div className="flex h-full min-h-0 flex-col gap-2 border-r border-app-line p-2">
          <Input
            placeholder="Filtrer les scripts…"
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            className="h-7 text-xs"
          />
          <span className="text-tiny text-ink-faint">
            {scriptsLoading
              ? "chargement du catalogue…"
              : `${shown.length} affiché(s) / ${scripts.length} script(s)`}
          </span>
          {scriptsError && <p className="text-tiny text-status-error">{scriptsError}</p>}
          <ScrollArea className="min-h-0 flex-1 rounded-lg border border-app-line bg-app-dark-box">
            <div className="flex flex-col p-1">
              {shown.map((s) => (
                <button
                  key={s.path}
                  type="button"
                  onClick={() => {
                    setSelected(s.path);
                    setUseSource(false);
                    setResult(null);
                    setGlobals([]);
                  }}
                  title={`${s.path}\n${humanSize(s.size)}`}
                  className={cn(
                    "flex items-center gap-1.5 rounded px-1.5 py-1 text-left text-tiny transition-colors",
                    selected === s.path && !useSource
                      ? "bg-accent text-white"
                      : "text-ink-dull hover:bg-app-hover hover:text-ink",
                  )}
                >
                  <Icon name="edit_note" size={12} />
                  <span className="min-w-0 flex-1 truncate">{s.name}</span>
                </button>
              ))}
              {!scriptsLoading && shown.length === 0 && (
                <p className="p-2 text-tiny text-ink-faint">Aucun script ne correspond.</p>
              )}
            </div>
          </ScrollArea>
        </div>
      }
    >
      <div className="flex h-full min-h-0 flex-col">
        {/* Barre d'outils */}
        <div className="flex shrink-0 flex-wrap items-center gap-2 border-b border-app-line px-2 py-1.5">
          <span className="min-w-0 flex-1 truncate text-xs font-medium text-ink">
            {useSource ? "Source éditée" : (selected ?? "Aucun script sélectionné")}
          </span>

          <label className="flex shrink-0 items-center gap-1.5 text-tiny text-ink-dull">
            <Switch checked={useSource} onCheckedChange={setUseSource} />
            Exécuter la source éditée
          </label>
          <label
            className="flex shrink-0 items-center gap-1.5 text-tiny text-ink-dull"
            title="Installe l'hôte de menu reversé — les vrais scripts de menu vont alors bien au-delà du premier appel moteur"
          >
            <Switch checked={withMenuHost} onCheckedChange={setWithMenuHost} />
            Hôte de menu
          </label>

          <label
            className="flex shrink-0 items-center gap-1.5 text-tiny text-ink-dull"
            title="La VM reste vivante entre les appels : la console devient un vrai REPL. Décoché, chaque exécution repart d'une VM neuve (mode analyse)."
          >
            <Switch checked={persistent} onCheckedChange={setPersistent} />
            Session persistante
          </label>

          <Button size="xs" onClick={persistent ? runInSession : run} disabled={busy}>
            ▶ Exécuter
          </Button>
          {persistent && (
            <>
              <Button size="xs" variant="outline" onClick={attachBehaviour} disabled={busy}>
                Attacher
              </Button>
              <Button size="xs" variant="outline" onClick={reloadSession} disabled={busy}>
                Recharger
              </Button>
            </>
          )}
        </div>

        {/* Cycle de vie — visible dès qu'un comportement est attaché. */}
        {persistent && attached.length > 0 && (
          <div className="flex shrink-0 flex-wrap items-center gap-1.5 border-b border-app-line px-2 py-1">
            <span className="text-tiny text-ink-faint">
              {attached.length} comportement(s) :
            </span>
            {LIFECYCLE.map((cb) => (
              <Button key={cb} size="xs" variant="ghost" onClick={() => void broadcast(cb)}>
                {cb}
              </Button>
            ))}
          </div>
        )}

        {/* En-tête du chunk */}
        {info && !useSource && (
          <div className="flex shrink-0 flex-wrap items-center gap-2 border-b border-app-line px-2 py-1 text-tiny text-ink-faint">
            <Badge variant="outline">Lua {info.version.toString(16)}</Badge>
            <span>{info.total_instructions.toLocaleString("fr-FR")} instructions</span>
            <span>{info.total_protos} prototype(s)</span>
            <span>{info.constants} constante(s)</span>
            <span>{info.strings.length} chaîne(s)</span>
            {info.has_debug_info ? (
              <Badge variant="outline">infos de débogage</Badge>
            ) : (
              <Badge variant="outline">dépouillé</Badge>
            )}
            {info.source && <span className="truncate">source : {info.source}</span>}
          </div>
        )}

        {error && <p className="px-2 py-1 text-tiny text-status-error">{error}</p>}

        <Tabs value={pane} onValueChange={(v) => v && setPane(v as Pane)}>
          <TabsList variant="line" className="px-2 pt-1.5">
            {(Object.keys(PANE_LABELS) as Pane[]).map((p) => (
              <TabsTrigger key={p} value={p} className="text-xs">
                {PANE_LABELS[p]}
              </TabsTrigger>
            ))}
          </TabsList>
        </Tabs>

        <div className="min-h-0 flex-1 overflow-hidden p-2">
          {pane === "disasm" && (
            <div className="h-full overflow-hidden rounded-lg border border-app-line">
              <Editor
                height="100%"
                language="plaintext"
                theme={resolvedTheme === "light" ? "light" : "vs-dark"}
                value={disasm || "; sélectionnez un script pour voir son désassemblage"}
                options={{
                  readOnly: true,
                  minimap: { enabled: true },
                  fontSize: 12,
                  automaticLayout: true,
                  scrollBeyondLastLine: false,
                }}
              />
            </div>
          )}

          {pane === "source" && (
            <div className="flex h-full min-h-0 flex-col gap-1">
              <div className="flex flex-wrap items-center gap-2">
                <p className="flex-1 text-tiny text-ink-faint">
                  Le jeu ne distribue que du bytecode : cet éditeur sert à écrire ou coller du Lua
                  pour l'exécuter dans la VM du jeu — activez « Exécuter la source éditée ».
                </p>
                {/* Le binder `Live.*` (session persistante) lit la mémoire de nie.exe EN COURS
                 * d'exécution, en lecture seule. Ce modèle amorce un exemple prêt à lancer. */}
                <Button
                  size="xs"
                  variant="outline"
                  title="Insère un exemple qui lit la mémoire du jeu en cours (Live.*)"
                  onClick={() => {
                    setUseSource(true);
                    setPersistent(true);
                    setSource(
                      [
                        "-- Lecture du process nie.exe VIVANT (lecture seule, via nie-trace).",
                        "-- Nécessite le jeu lancé + la session persistante active.",
                        "local p = Live.FindProcess()",
                        "if not p then",
                        "  Debug.LogWarning('nie.exe n\\'est pas lancé')",
                        "  return",
                        "end",
                        "Debug.Log('pid = ' .. p.pid .. ', base = ' .. tostring(p.base))",
                        "",
                        "-- Exemple : lire 16 octets à la base du module et les afficher en hexa.",
                        "local base = tonumber(p.base)",
                        "local bytes = Live.Read(base, 16)",
                        "if bytes then",
                        "  local hex = {}",
                        "  for i = 1, #bytes do hex[i] = string.format('%02X', string.byte(bytes, i)) end",
                        "  Debug.Log(table.concat(hex, ' '))",
                        "end",
                      ].join("\n"),
                    );
                  }}
                >
                  Exemple Live (lecture process)
                </Button>
              </div>
              <div className="min-h-0 flex-1 overflow-hidden rounded-lg border border-app-line">
                <Editor
                  height="100%"
                  language="lua"
                  theme={resolvedTheme === "light" ? "light" : "vs-dark"}
                  value={source}
                  onChange={(v) => setSource(v ?? "")}
                  options={{
                    minimap: { enabled: false },
                    fontSize: 13,
                    automaticLayout: true,
                    scrollBeyondLastLine: false,
                  }}
                />
              </div>
            </div>
          )}

          {pane === "console" && (
            <div className="flex h-full min-h-0 flex-col gap-2">
              <ScrollArea className="min-h-0 flex-1 rounded-lg border border-app-line bg-app-darker-box p-2 font-mono text-tiny">
                {result && (
                  <>
                    {result.stdout.map((line, i) => (
                      <div key={`out-${i}`} className="text-ink-dull">
                        {line}
                      </div>
                    ))}
                    {result.returned.length > 0 && (
                      <div className="text-accent">→ {result.returned.join("\t")}</div>
                    )}
                    {result.error && <div className="text-status-error">{result.error}</div>}
                    {result.missing_host_calls.length > 0 && (
                      <div className="mt-2 text-status-warning">
                        API moteur réclamée par ce script ({result.missing_host_calls.length}) :{" "}
                        {result.missing_host_calls.join(", ")}
                      </div>
                    )}
                    <div className="mt-1 text-ink-faint">— exécuté en {result.duration_ms} ms</div>
                  </>
                )}
                {consoleLines.map((l, i) => (
                  <div key={`c-${i}`}>
                    <div className="text-accent">&gt; {l.input}</div>
                    <div className="text-ink-dull">{l.output}</div>
                  </div>
                ))}
                <div ref={consoleEndRef} />
              </ScrollArea>
              <div className="flex shrink-0 gap-2">
                <Input
                  placeholder="Expression Lua à évaluer dans l'état du script (Entrée)…"
                  value={consoleInput}
                  onChange={(e) => setConsoleInput(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") {
                      e.preventDefault();
                      void submitConsole();
                    }
                  }}
                  className="h-7 font-mono text-xs"
                />
                <Button size="xs" variant="outline" onClick={() => setConsoleLines([])}>
                  Effacer
                </Button>
              </div>
            </div>
          )}

          {pane === "api" && (
            <div className="flex h-full min-h-0 flex-col gap-2">
              <div className="flex shrink-0 items-center gap-2">
                <Button size="xs" onClick={refreshApiReport}>
                  Rafraîchir le rapport
                </Button>
                {apiReport && (
                  <span className="text-tiny text-ink-dull">
                    couverture {apiReport.coverage_percent} % — {apiReport.provided.length} fournis,{" "}
                    {apiReport.missing.length} manquants
                  </span>
                )}
              </div>
              <p className="shrink-0 text-tiny text-ink-faint">
                Ce que les scripts exécutés dans la session ont réclamé au moteur, face à ce que
                l'hôte fournit. C'est la liste de travail du portage : chaque nom manquant est une
                fonction de <code>nie.exe</code> à reverser puis à exposer comme binder.
              </p>
              <ScrollArea className="min-h-0 flex-1 rounded-lg border border-app-line bg-app-dark-box">
                <div className="grid grid-cols-2 gap-2 p-2">
                  <div>
                    <p className="pb-1 text-tiny font-semibold uppercase text-status-warning">
                      Réclamé et absent
                    </p>
                    <div className="flex flex-col gap-0.5 font-mono text-tiny text-ink-dull">
                      {apiReport?.missing.map((m) => <span key={m}>{m}</span>)}
                      {apiReport?.missing.length === 0 && (
                        <span className="text-ink-faint">rien — tout ce qui a été appelé existe</span>
                      )}
                    </div>
                  </div>
                  <div>
                    <p className="pb-1 text-tiny font-semibold uppercase text-status-success">
                      Fourni par l'hôte
                    </p>
                    <div className="flex flex-col gap-0.5 font-mono text-tiny text-ink-dull">
                      {apiReport?.provided.map((p) => <span key={p}>{p}</span>)}
                    </div>
                  </div>
                </div>
                {!apiReport && (
                  <p className="p-3 text-tiny text-ink-faint">
                    Exécutez un script en session persistante, puis rafraîchissez.
                  </p>
                )}
              </ScrollArea>
            </div>
          )}

          {pane === "values" && (
            <div className="flex h-full min-h-0 flex-col gap-2">
              <div className="flex shrink-0 flex-wrap items-center gap-2">
                <Button size="xs" onClick={refreshGlobals} disabled={busy}>
                  {persistent ? "Appliquer et inspecter" : "Exécuter et inspecter"}
                </Button>
                <label className="flex items-center gap-1.5 text-tiny text-ink-dull">
                  <Switch checked={includeStdlib} onCheckedChange={setIncludeStdlib} />
                  Inclure la bibliothèque standard
                </label>
                <Button
                  size="xs"
                  variant="outline"
                  onClick={() => setOverrides((p) => [...p, ["", ""]])}
                >
                  + Valeur forcée
                </Button>
              </div>

              {/* Valeurs forcées : posées AVANT l'exécution, pour rejouer un script « comme si »
               * une variable moteur valait autre chose. */}
              {overrides.length > 0 && (
                <div className="flex shrink-0 flex-col gap-1 rounded-lg border border-app-line p-2">
                  {overrides.map(([name, value], i) => (
                    <div key={i} className="flex items-center gap-1.5">
                      <Input
                        placeholder="nom"
                        value={name}
                        onChange={(e) =>
                          setOverrides((p) => p.map((o, j) => (j === i ? [e.target.value, o[1]] : o)))
                        }
                        className="h-6 w-40 font-mono text-tiny"
                      />
                      <span className="text-tiny text-ink-faint">=</span>
                      <Input
                        placeholder="expression Lua (ex. 999, 'texte', {a=1})"
                        value={value}
                        onChange={(e) =>
                          setOverrides((p) => p.map((o, j) => (j === i ? [o[0], e.target.value] : o)))
                        }
                        className="h-6 flex-1 font-mono text-tiny"
                      />
                      <Button
                        size="icon-xs"
                        variant="ghost"
                        onClick={() => setOverrides((p) => p.filter((_, j) => j !== i))}
                        aria-label="Retirer"
                      >
                        <Icon name="close" size={12} />
                      </Button>
                    </div>
                  ))}
                </div>
              )}

              <ScrollArea className="min-h-0 flex-1 rounded-lg border border-app-line bg-app-dark-box">
                <div className="divide-y divide-app-line">
                  {globals.map((g) => (
                    <div key={g.name} className="flex items-center gap-2 px-2 py-1 font-mono text-tiny">
                      <span className="min-w-0 flex-1 truncate text-ink">{g.name}</span>
                      <Badge variant="outline">{g.type_name}</Badge>
                      <span className="max-w-[40%] truncate text-ink-dull">
                        {g.value}
                        {g.len != null ? ` (${g.len})` : ""}
                      </span>
                      <Button
                        size="icon-xs"
                        variant="ghost"
                        title="Forcer cette valeur"
                        aria-label="Forcer cette valeur"
                        onClick={() => setOverrides((p) => [...p, [g.name, g.value]])}
                      >
                        <Icon name="edit" size={12} />
                      </Button>
                    </div>
                  ))}
                  {globals.length === 0 && (
                    <p className="p-3 text-tiny text-ink-faint">
                      « Exécuter et inspecter » lance le script dans une VM neuve et liste ce qu'il
                      a posé.
                    </p>
                  )}
                </div>
              </ScrollArea>
            </div>
          )}
        </div>
      </div>
    </SplitPane>
  );
}
