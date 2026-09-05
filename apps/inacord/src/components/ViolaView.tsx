// Onglet **Viola** — les opérations de modding LEVEL-5, servies par le crate `nie-viola` EN
// PROCESS (aucun binaire externe, contrairement à l'outil amont et aux gestionnaires de mods qui
// se contentent de piloter `Viola.exe`).
//
// Quatre opérations, dans l'ordre du parcours réel d'une moddeuse : extraire le jeu (Dump),
// empaqueter ce qu'on a édité (Pack), combiner plusieurs mods (Merge), et manipuler les
// conteneurs audio chiffrés (Criware).
import { useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";
import { api } from "@/lib/api";
import type { ViolaMergeDto, ViolaPackDto, ViolaPlatform } from "@/lib/bindings";
import { jobsDb } from "@/lib/jobsDb";
import { humanSize } from "@/lib/bytes";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import { Switch } from "@/components/ui/switch";
import { Progress } from "@/components/ui/progress";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { getSettings } from "@/lib/settings";

// Les charges utiles d'événements ne passent pas par `tauri-specta` (il n'exporte que les types
// des signatures de commandes) : on les décrit ici, à l'image de `lib/vfsIndexDb.ts`.
interface DumpProgress {
  run_id: string;
  faits: number;
  total: number;
  octets: number;
}
interface DumpDone {
  run_id: string;
  extraits: number;
  sautes: number;
  echecs: number;
  octets: number;
  packs_repris: number;
  annule: boolean;
  erreur: string | null;
}

/** Sélecteur de chemin réutilisé par les quatre panneaux — champ libre + bouton natif. */
function ChampChemin({
  label,
  valeur,
  onChange,
  dossier,
  placeholder,
  filtres,
}: {
  label: string;
  valeur: string;
  onChange: (v: string) => void;
  dossier?: boolean;
  placeholder?: string;
  filtres?: { name: string; extensions: string[] }[];
}) {
  return (
    <div className="space-y-1.5">
      <Label>{label}</Label>
      <div className="flex gap-2">
        <Input value={valeur} placeholder={placeholder} onChange={(e) => onChange(e.target.value)} />
        <Button
          variant="outline"
          onClick={async () => {
            const r = await open(dossier ? { directory: true } : { filters: filtres });
            if (typeof r === "string") onChange(r);
          }}
        >
          Parcourir…
        </Button>
      </div>
    </div>
  );
}

export function ViolaView() {
  const [sousOnglet, setSousOnglet] = useState("dump");

  // ── Dump ────────────────────────────────────────────────────────────────────────────────────
  const [dumpSortie, setDumpSortie] = useState("");
  const [dumpFiltre, setDumpFiltre] = useState("");
  const [dumpReprise, setDumpReprise] = useState(true);
  const [dumpSauter, setDumpSauter] = useState(true);
  const [dumpRun, setDumpRun] = useState<string | null>(null);
  const [dumpAvance, setDumpAvance] = useState<DumpProgress | null>(null);
  const [dumpBilan, setDumpBilan] = useState<DumpDone | null>(null);
  const jobRef = useRef<string | null>(null);

  useEffect(() => {
    // Les deux écoutes sont posées une fois pour toutes : un `listen` par démarrage de dump
    // laisserait des abonnés derrière lui à chaque exécution.
    const unProgres = listen<DumpProgress>("viola-dump-progress", (e) => {
      setDumpAvance(e.payload);
      if (jobRef.current) {
        void jobsDb.progress(jobRef.current, e.payload.faits, e.payload.total);
      }
    });
    const unFin = listen<DumpDone>("viola-dump-done", (e) => {
      setDumpRun(null);
      setDumpBilan(e.payload);
      const id = jobRef.current;
      jobRef.current = null;
      if (id) {
        void jobsDb.finish(
          id,
          e.payload.erreur ? "error" : e.payload.annule ? "canceled" : "done",
          e.payload.erreur ?? undefined,
        );
      }
      if (e.payload.erreur) toast.error(e.payload.erreur);
      else if (e.payload.annule) toast.info("Dump interrompu — il reprendra où il s'est arrêté.");
      else toast.success(`Dump terminé : ${e.payload.extraits} fichiers, ${humanSize(e.payload.octets)}`);
    });
    return () => {
      void unProgres.then((f) => f());
      void unFin.then((f) => f());
    };
  }, []);

  async function lancerDump() {
    if (!dumpSortie.trim()) {
      toast.error("Choisis un dossier de sortie.");
      return;
    }
    setDumpBilan(null);
    setDumpAvance(null);
    try {
      const id = await api.violaDumpStart(
        dumpSortie,
        { filtre: dumpFiltre, reprise: dumpReprise, sauterIdentiques: dumpSauter },
        getSettings().gameDir,
      );
      setDumpRun(id);
      jobRef.current = await jobsDb.create("viola-dump", `Dump vers ${dumpSortie}`);
    } catch (e) {
      toast.error(String(e));
    }
  }

  // ── Pack ────────────────────────────────────────────────────────────────────────────────────
  const [packCpkList, setPackCpkList] = useState("");
  const [packMod, setPackMod] = useState("");
  const [packSortie, setPackSortie] = useState("");
  const [packPlateforme, setPackPlateforme] = useState<ViolaPlatform>("pc");
  const [packBilan, setPackBilan] = useState<ViolaPackDto | null>(null);
  const [packOccupe, setPackOccupe] = useState(false);

  // ── Merge ───────────────────────────────────────────────────────────────────────────────────
  const [mergeSources, setMergeSources] = useState<string[]>([]);
  const [mergeSortie, setMergeSortie] = useState("");
  const [mergeSemantique, setMergeSemantique] = useState(true);
  const [mergeBilan, setMergeBilan] = useState<ViolaMergeDto | null>(null);
  const [mergeOccupe, setMergeOccupe] = useState(false);

  // ── Criware ─────────────────────────────────────────────────────────────────────────────────
  const [cryptoEntree, setCryptoEntree] = useState("");
  const [cryptoSortie, setCryptoSortie] = useState("");
  const [cryptoCle, setCryptoCle] = useState("");
  const [cryptoOccupe, setCryptoOccupe] = useState(false);

  return (
    <div className="h-full overflow-auto p-4">
      <Tabs value={sousOnglet} onValueChange={setSousOnglet} className="space-y-4">
        <TabsList>
          <TabsTrigger value="dump">Dump</TabsTrigger>
          <TabsTrigger value="pack">Pack</TabsTrigger>
          <TabsTrigger value="merge">Merge</TabsTrigger>
          <TabsTrigger value="crypto">Criware</TabsTrigger>
        </TabsList>

        {/* ── Dump ─────────────────────────────────────────────────────────────────────────── */}
        <TabsContent value="dump" className="space-y-4">
          <Card>
            <CardHeader>
              <CardTitle>Extraire le jeu</CardTitle>
              <CardDescription>
                Écrit les ~255 000 fichiers des archives CPK dans une arborescence lisible. Les
                paquets sont traités du plus gros au plus petit et mappés en mémoire, jamais
                recopiés dans un dossier temporaire.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <ChampChemin label="Dossier de sortie" valeur={dumpSortie} onChange={setDumpSortie} dossier />
              <div className="space-y-1.5">
                <Label>Filtre (facultatif)</Label>
                <Input
                  value={dumpFiltre}
                  placeholder="*.g4tx  ·  data/common/gamedata/*"
                  onChange={(e) => setDumpFiltre(e.target.value)}
                />
                <p className="text-app-faint text-xs">
                  Seul le joker <code>*</code> est reconnu. Vide = tout extraire.
                </p>
              </div>
              <div className="flex items-center gap-6">
                <label className="flex items-center gap-2 text-sm">
                  <Switch checked={dumpReprise} onCheckedChange={setDumpReprise} />
                  Reprendre un dump interrompu
                </label>
                <label className="flex items-center gap-2 text-sm">
                  <Switch checked={dumpSauter} onCheckedChange={setDumpSauter} />
                  Sauter les fichiers déjà à la bonne taille
                </label>
              </div>

              {dumpRun ? (
                <div className="space-y-2">
                  <Progress value={dumpAvance ? (dumpAvance.faits / Math.max(1, dumpAvance.total)) * 100 : 0} />
                  <div className="flex items-center justify-between text-sm">
                    <span>
                      {dumpAvance ? `${dumpAvance.faits} / ${dumpAvance.total} fichiers` : "Indexation du VFS…"}
                      {dumpAvance ? ` · ${humanSize(dumpAvance.octets)}` : ""}
                    </span>
                    <Button variant="outline" onClick={() => void api.violaCancel(dumpRun)}>
                      Annuler
                    </Button>
                  </div>
                </div>
              ) : (
                <Button onClick={() => void lancerDump()}>Lancer le dump</Button>
              )}

              {dumpBilan && !dumpBilan.erreur && (
                <Alert>
                  <AlertTitle>{dumpBilan.annule ? "Dump interrompu" : "Dump terminé"}</AlertTitle>
                  <AlertDescription>
                    {dumpBilan.extraits} extraits · {dumpBilan.sautes} déjà à jour · {dumpBilan.echecs} en
                    échec · {humanSize(dumpBilan.octets)}
                    {dumpBilan.packs_repris > 0 ? ` · ${dumpBilan.packs_repris} paquets repris` : ""}
                  </AlertDescription>
                </Alert>
              )}
            </CardContent>
          </Card>
        </TabsContent>

        {/* ── Pack ─────────────────────────────────────────────────────────────────────────── */}
        <TabsContent value="pack" className="space-y-4">
          <Card>
            <CardHeader>
              <CardTitle>Empaqueter un mod</CardTitle>
              <CardDescription>
                Aucune archive n'est fabriquée : le jeu charge un fichier depuis le disque dès que
                son entrée du <code>cpk_list.cfg.bin</code> ne désigne plus de paquet. Donne le
                <strong> cpk_list d'origine</strong>, sauvegardé avant tout modding.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <ChampChemin
                label="cpk_list.cfg.bin d'origine"
                valeur={packCpkList}
                onChange={setPackCpkList}
                filtres={[{ name: "cfg.bin", extensions: ["bin"] }]}
              />
              <ChampChemin label="Dossier du mod" valeur={packMod} onChange={setPackMod} dossier />
              <ChampChemin label="Dossier de sortie" valeur={packSortie} onChange={setPackSortie} dossier />
              <div className="space-y-1.5">
                <Label>Plateforme</Label>
                <div className="flex gap-2">
                  {(["pc", "switch"] as ViolaPlatform[]).map((p) => (
                    <Button
                      key={p}
                      variant={packPlateforme === p ? "default" : "outline"}
                      onClick={() => setPackPlateforme(p)}
                    >
                      {p === "pc" ? "PC (Steam)" : "Nintendo Switch"}
                    </Button>
                  ))}
                </div>
              </div>
              <Button
                disabled={packOccupe}
                onClick={async () => {
                  setPackOccupe(true);
                  setPackBilan(null);
                  try {
                    const r = await api.violaPack(packCpkList, packMod, packSortie, packPlateforme);
                    setPackBilan(r);
                    toast.success(`Mod empaqueté : ${r.mis_a_jour} remplacés, ${r.ajoutes} ajoutés`);
                  } catch (e) {
                    toast.error(String(e));
                  } finally {
                    setPackOccupe(false);
                  }
                }}
              >
                {packOccupe ? "Empaquetage…" : "Empaqueter"}
              </Button>

              {packBilan && (
                <Alert>
                  <AlertTitle>
                    Mod empaqueté <Badge variant="outline">{packBilan.enveloppe}</Badge>
                  </AlertTitle>
                  <AlertDescription>
                    {packBilan.mis_a_jour} entrées remplacées · {packBilan.ajoutes} ajoutées ·{" "}
                    {packBilan.copies} fichiers copiés · {packBilan.total} entrées au total.
                    {(packBilan.loose_avant ?? 0) > 16 && (
                      <span className="text-app-red block pt-1">
                        Attention : ce cpk_list contient déjà {packBilan.loose_avant} entrées hors
                        paquet. Il a probablement déjà servi à empaqueter un mod — repartir de
                        celui-ci empile les modifications précédentes.
                      </span>
                    )}
                  </AlertDescription>
                </Alert>
              )}
            </CardContent>
          </Card>
        </TabsContent>

        {/* ── Merge ────────────────────────────────────────────────────────────────────────── */}
        <TabsContent value="merge" className="space-y-4">
          <Card>
            <CardHeader>
              <CardTitle>Fusionner des mods</CardTitle>
              <CardDescription>
                L'ordre est celui de la priorité : le premier l'emporte. La fusion au champ compare
                chaque valeur au jeu d'origine, ce qui rend compatibles deux mods qui touchent des
                valeurs différentes d'un même fichier — impossible avec une fusion au fichier.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-2">
                <Label>Dossiers de mods, du plus prioritaire au moins prioritaire</Label>
                {mergeSources.map((s, i) => (
                  <div key={`${s}-${i}`} className="flex items-center gap-2">
                    <Badge variant="outline">{i + 1}</Badge>
                    <Input
                      value={s}
                      onChange={(e) =>
                        setMergeSources(mergeSources.map((v, j) => (j === i ? e.target.value : v)))
                      }
                    />
                    <Button
                      variant="ghost"
                      disabled={i === 0}
                      onClick={() => {
                        const c = [...mergeSources];
                        [c[i - 1], c[i]] = [c[i], c[i - 1]];
                        setMergeSources(c);
                      }}
                    >
                      ↑
                    </Button>
                    <Button
                      variant="ghost"
                      onClick={() => setMergeSources(mergeSources.filter((_, j) => j !== i))}
                    >
                      ✕
                    </Button>
                  </div>
                ))}
                <Button
                  variant="outline"
                  onClick={async () => {
                    const d = await open({ directory: true });
                    if (typeof d === "string") setMergeSources([...mergeSources, d]);
                  }}
                >
                  Ajouter un mod…
                </Button>
              </div>

              <ChampChemin label="Dossier de sortie" valeur={mergeSortie} onChange={setMergeSortie} dossier />
              <label className="flex items-center gap-2 text-sm">
                <Switch checked={mergeSemantique} onCheckedChange={setMergeSemantique} />
                Fusion au champ des <code>.cfg.bin</code> (utilise le jeu comme référence)
              </label>

              <Button
                disabled={mergeOccupe || mergeSources.length === 0}
                onClick={async () => {
                  setMergeOccupe(true);
                  setMergeBilan(null);
                  try {
                    const r = await api.violaMerge(
                      mergeSources,
                      mergeSortie,
                      mergeSemantique,
                      getSettings().gameDir,
                    );
                    setMergeBilan(r);
                    toast.success(`Fusion terminée : ${r.fusionnes} fichiers fusionnés au champ`);
                  } catch (e) {
                    toast.error(String(e));
                  } finally {
                    setMergeOccupe(false);
                  }
                }}
              >
                {mergeOccupe ? "Fusion…" : "Fusionner"}
              </Button>

              {mergeBilan && (
                <div className="space-y-2">
                  <Alert>
                    <AlertTitle>Fusion terminée</AlertTitle>
                    <AlertDescription>
                      {mergeBilan.copies} fichiers copiés · {mergeBilan.fusionnes} fusionnés au champ ·{" "}
                      {mergeBilan.conflits.length} chemins disputés
                    </AlertDescription>
                  </Alert>
                  {mergeBilan.conflits.length > 0 && (
                    <div className="border-app-line max-h-64 overflow-auto rounded-md border text-sm">
                      {mergeBilan.conflits.map((c) => (
                        <div key={c.chemin} className="border-app-line border-b p-2 last:border-b-0">
                          <div className="font-mono text-xs">{c.chemin}</div>
                          <div className="text-app-faint text-xs">
                            {c.repli
                              ? `fusion au fichier — ${c.repli}`
                              : `${c.champs_fusionnes} valeurs fusionnées, ${c.champs_en_desaccord} en désaccord`}
                          </div>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              )}
            </CardContent>
          </Card>
        </TabsContent>

        {/* ── Criware ──────────────────────────────────────────────────────────────────────── */}
        <TabsContent value="crypto" className="space-y-4">
          <Card>
            <CardHeader>
              <CardTitle>Chiffrer / déchiffrer un fichier Criware</CardTitle>
              <CardDescription>
                Le chiffrement est involutif : le même bouton sert dans les deux sens. Le fichier
                est traité par tranches, sa taille n'a donc pas d'importance. Sans clé, elle est
                dérivée du nom du fichier — la règle des paquets CPK.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <ChampChemin label="Fichier d'entrée" valeur={cryptoEntree} onChange={setCryptoEntree} />
              <ChampChemin label="Fichier de sortie" valeur={cryptoSortie} onChange={setCryptoSortie} />
              <div className="space-y-1.5">
                <Label>Clé hexadécimale (facultatif)</Label>
                <Input
                  value={cryptoCle}
                  placeholder="1717E18E"
                  onChange={(e) => setCryptoCle(e.target.value)}
                />
              </div>
              <Button
                disabled={cryptoOccupe}
                onClick={async () => {
                  setCryptoOccupe(true);
                  try {
                    const n = await api.violaCrypto(cryptoEntree, cryptoSortie, cryptoCle);
                    toast.success(`${humanSize(n ?? 0)} traités`);
                  } catch (e) {
                    toast.error(String(e));
                  } finally {
                    setCryptoOccupe(false);
                  }
                }}
              >
                {cryptoOccupe ? "Traitement…" : "Traiter le fichier"}
              </Button>
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </div>
  );
}
