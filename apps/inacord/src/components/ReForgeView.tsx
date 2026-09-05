// Onglet « Forge » — la part de `nie.exe` que le dépôt produit **réellement**, à l'octet.
//
// Le projet ne vise pas seulement à rejouer le jeu : il vise à produire le binaire. La forge est
// le juge — `nie-forge build` échoue si le fichier produit n'est pas byte-identique à la
// référence, et la métrique de progression est la masse d'octets réellement générés.
//
// Cette vue relit les mêmes artefacts que la CLI à chaque appel (`var/forge/cover.json`,
// `forge/registry.json`, `forge/asm/*.s`) : rien n'est figé, ce qui est affiché est l'état du
// disque. Le second panneau est la **liste de travail** — ce qui empêche encore de produire,
// trié par octets bloqués. C'est ce diagnostic chiffré, et non l'intuition, qui guide
// l'élargissement du dialecte assembleur.
import { useEffect, useState } from "react";
import { toast } from "sonner";
import { api, type ForgeBlocker, type ForgeReport } from "@/lib/api";
import { defaultReDbPath, reDb, type ClasseForge, type StatutForge } from "@/lib/reDb";
import { useSettings } from "@/lib/settings";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";

/** Sépare les milliers à la française — ces nombres se lisent, ils ne se survolent pas. */
function o(n: number): string {
  return n.toLocaleString("fr-FR");
}

/** Ce que chaque état signifie — repris mot pour mot des constantes de `nie-forge/src/kb.rs`,
 * pour que la colonne ne raconte pas autre chose que ce que la forge a écrit. */
const SENS_STATUT: Record<string, string> = {
  produit: "corps relevé et ré-encodé à l'octet près",
  bloque: "code que le relevé refuse encore, avec sa cause",
  regle: "octets régénérés par une règle du linker (bourrage int3, en-têtes)",
  donnees_inline: "données déposées au milieu du code (tables de sauts, constantes)",
  verbatim: "recopié de la référence : rien n'est prétendu",
  hors_decoupage: "fonction de la base sans unité correspondante",
};

export function ReForgeView() {
  const [report, setReport] = useState<ForgeReport | null>(null);
  const [blockers, setBlockers] = useState<ForgeBlocker[]>([]);
  const [loading, setLoading] = useState(false);
  const [scanning, setScanning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  /** Ce que la forge a écrit DANS la base (`nie-forge kb`) : l'état par unité et par classe. */
  const [statuts, setStatuts] = useState<StatutForge[]>([]);
  const [classes, setClasses] = useState<ClasseForge[]>([]);
  const parametres = useSettings();

  // La répartition et le classement viennent de `niers.sqlite`, pas des artefacts de `var/forge/`
  // que relit `forgeReport` : ce sont deux vues du même travail, et la base est la seule à savoir
  // à QUELLE classe appartient un corps bloqué.
  useEffect(() => {
    let vivant = true;
    defaultReDbPath(parametres.gameDir)
      .then(async (chemin) => {
        if (!chemin || !vivant) return;
        const [s, c] = await Promise.all([reDb.statutsForge(chemin), reDb.classesForge(chemin, 300)]);
        if (!vivant) return;
        setStatuts(s);
        setClasses(c);
      })
      .catch(() => {});
    return () => {
      vivant = false;
    };
  }, [parametres.gameDir]);

  async function load() {
    setLoading(true);
    setError(null);
    try {
      setReport(await api.forgeReport());
    } catch (e) {
      setError(String(e));
      setReport(null);
    } finally {
      setLoading(false);
    }
  }

  async function loadBlockers() {
    setScanning(true);
    try {
      setBlockers(await api.forgeBlockers(undefined, 25));
    } catch (e) {
      toast.error(String(e));
    } finally {
      setScanning(false);
    }
  }

  useEffect(() => {
    void load();
  }, []);

  if (error) {
    return (
      <div className="p-4">
        <Alert>
          <AlertTitle>Forge indisponible</AlertTitle>
          <AlertDescription>
            {error}
            <div className="mt-2 text-xs opacity-70">
              Le recouvrement se produit avec <code>nie-forge split --exe nie.exe --db var/niers.sqlite</code>,
              puis <code>nie-forge lift</code>.
            </div>
          </AlertDescription>
        </Alert>
      </div>
    );
  }

  return (
    <ScrollArea className="h-full">
      <div className="flex flex-col gap-4 p-1">
        <div className="flex items-center gap-2">
          <Button size="sm" variant="outline" onClick={() => void load()} disabled={loading}>
            {loading ? "Mesure…" : "Recalculer"}
          </Button>
          <Button size="sm" variant="outline" onClick={() => void loadBlockers()} disabled={scanning}>
            {scanning ? "Analyse de .text…" : "Ce qui bloque"}
          </Button>
          {report ? <span className="truncate text-xs opacity-60">{report.root}</span> : null}
        </div>

        {report ? (
          <>
            <div className="grid grid-cols-2 gap-3">
              <div className="rounded border p-3">
                <div className="text-xs uppercase opacity-60">Fichier produit</div>
                <div className="text-2xl font-semibold tabular-nums">{(report.produced_pct ?? 0).toFixed(3)} %</div>
                <div className="text-xs opacity-70">
                  {o(report.produced_bytes)} / {o(report.total_bytes)} octets
                </div>
              </div>
              <div className="rounded border p-3">
                <div className="text-xs uppercase opacity-60">.text produit</div>
                <div className="text-2xl font-semibold tabular-nums">{(report.code_pct ?? 0).toFixed(3)} %</div>
                <div className="text-xs opacity-70">sur {o(report.code_bytes)} octets de code</div>
              </div>
            </div>

            <div>
              <div className="mb-1 text-xs uppercase opacity-60">Par source</div>
              <table className="w-full text-sm">
                <thead className="text-xs opacity-60">
                  <tr>
                    <th className="text-left font-normal">source</th>
                    <th className="text-right font-normal">unités</th>
                    <th className="text-right font-normal">octets</th>
                  </tr>
                </thead>
                <tbody>
                  <tr>
                    <td>en-têtes PE recalculés (nie-pe)</td>
                    <td className="text-right tabular-nums">{o(report.emitted.units)}</td>
                    <td className="text-right tabular-nums">{o(report.emitted.bytes)}</td>
                  </tr>
                  <tr>
                    <td>corps réassemblés (nie-asm)</td>
                    <td className="text-right tabular-nums">{o(report.assembled.units)}</td>
                    <td className="text-right tabular-nums">{o(report.assembled.bytes)}</td>
                  </tr>
                  <tr>
                    <td>codegen Rust coïncidant</td>
                    <td className="text-right tabular-nums">{o(report.matched_bytes.units)}</td>
                    <td className="text-right tabular-nums">{o(report.matched_bytes.bytes)}</td>
                  </tr>
                  <tr className="opacity-60">
                    <td>validé sémantiquement — jamais compté comme produit</td>
                    <td className="text-right tabular-nums">{o(report.matched_semantic.units)}</td>
                    <td className="text-right tabular-nums">{o(report.matched_semantic.bytes)}</td>
                  </tr>
                </tbody>
              </table>
              <div className="mt-1 text-xs opacity-60">
                {o(report.total_units)} unités au recouvrement, dont {o(report.functions)} fonctions.
                {report.orphan_entries > 0
                  ? ` ${o(report.orphan_entries)} entrée(s) de registre sans unité correspondante.`
                  : null}
              </div>
            </div>

            {/* Ce que la forge a inscrit dans la base : l'état de CHAQUE unité du découpage.
                Les deux colonnes disent des choses différentes — le bourrage `int3` pèse des
                dizaines de milliers d'unités pour un peu plus d'un mégaoctet, quand une poignée
                d'unités recopiées en pèsent huit. Compter les unités seules donnerait au bourrage
                l'apparence du gros du travail. */}
            {statuts.length > 0 ? (
              <div>
                <div className="mb-1 text-xs uppercase opacity-60">Par état de production (base de connaissance)</div>
                <table className="w-full text-sm">
                  <thead className="text-xs opacity-60">
                    <tr>
                      <th className="text-left font-normal">état</th>
                      <th className="text-right font-normal">unités</th>
                      <th className="text-right font-normal">octets</th>
                      <th className="text-left font-normal">ce que cela veut dire</th>
                    </tr>
                  </thead>
                  <tbody>
                    {statuts.map((s) => (
                      <tr key={s.statut}>
                        <td className="font-mono">{s.statut}</td>
                        <td className="text-right tabular-nums">{o(s.unites)}</td>
                        <td className="text-right tabular-nums">{o(s.octets)}</td>
                        <td className="text-xs opacity-70">{SENS_STATUT[s.statut] ?? ""}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            ) : null}

            {/* Le tri par classe : à quelles classes appartiennent les corps qui bloquent encore.
                `résolues` borne la lecture — les adresses de vtable viennent de l'index Ghidra,
                les fonctions du découpage `#pdata` ; une classe dont peu de méthodes se résolvent
                décrit un autre agencement, et ses compteurs ne se lisent pas. */}
            {classes.length > 0 ? (
              <div>
                <div className="mb-1 text-xs uppercase opacity-60">
                  Classes RTTI où il reste à produire — {o(classes.length)} classes affichées
                </div>
                <table className="w-full text-sm">
                  <thead className="text-xs opacity-60">
                    <tr>
                      <th className="text-left font-normal">classe</th>
                      <th className="text-right font-normal">méthodes</th>
                      <th className="text-right font-normal">résolues</th>
                      <th className="text-right font-normal">produites</th>
                      <th className="text-right font-normal">bloquées</th>
                      <th className="text-right font-normal">octets</th>
                    </tr>
                  </thead>
                  <tbody>
                    {classes.slice(0, 60).map((c) => (
                      <tr key={c.classe} className={c.bloquees === 0 ? "opacity-60" : undefined}>
                        <td className="max-w-[22rem] truncate font-mono text-xs" title={c.classe}>
                          {c.classe}
                        </td>
                        <td className="text-right tabular-nums">{o(c.methodes)}</td>
                        <td className="text-right tabular-nums">{o(c.resolues)}</td>
                        <td className="text-right tabular-nums">{o(c.produites)}</td>
                        <td className="text-right tabular-nums font-semibold">{o(c.bloquees)}</td>
                        <td className="text-right tabular-nums">{o(c.octets)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            ) : null}

            {blockers.length > 0 ? (
              <div>
                <div className="mb-1 text-xs uppercase opacity-60">
                  Ce qui bloque encore — la première ligne est la prochaine cible
                </div>
                <table className="w-full text-sm">
                  <thead className="text-xs opacity-60">
                    <tr>
                      <th className="text-left font-normal">cause</th>
                      <th className="text-right font-normal">unités</th>
                      <th className="text-right font-normal">octets</th>
                      <th className="text-left font-normal">exemple</th>
                    </tr>
                  </thead>
                  <tbody>
                    {blockers.map((b) => (
                      <tr key={b.cause}>
                        <td className="font-mono">{b.cause}</td>
                        <td className="text-right tabular-nums">{o(b.units)}</td>
                        <td className="text-right tabular-nums">{o(b.bytes)}</td>
                        <td className="max-w-[24rem] truncate font-mono text-xs opacity-70" title={b.sample}>
                          {b.sample}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            ) : null}
          </>
        ) : null}
      </div>
    </ScrollArea>
  );
}
