// **Probabilités** — butin des matchs et tirages de capsules, lus du jeu et convertis en chances.
//
// Outil ABSENT du wiki : `soccer_drop_config` et `capsule_config` n'y sont pas publiés. Les poids
// bruts du jeu (`weight`, `rate`) n'ont aucun sens hors de leur table ; ils sont donc rapportés au
// total de leur table côté Rust (`share_pct`), et affichés ici triés du plus probable au moins
// probable — la seule lecture honnête d'un poids.
import { useEffect, useMemo, useState } from "react";

import { api, type CapsuleRate, type Drop } from "@/lib/api";
import { useSettings } from "@/lib/settings";
import { conditionLisible } from "@/lib/valeurs";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";

type Onglet = "butin" | "capsules";

export function ProbabilitesPanel() {
  const settings = useSettings();
  const [onglet, setOnglet] = useState<Onglet>("butin");
  const [butin, setButin] = useState<Drop[]>([]);
  const [capsules, setCapsules] = useState<CapsuleRate[]>([]);
  const [erreur, setErreur] = useState<string | null>(null);
  const [filtre, setFiltre] = useState("");

  useEffect(() => {
    let annule = false;
    Promise.all([api.gameDataDrops(settings.gameDir), api.gameDataCapsuleRates(settings.gameDir)])
      .then(([d, c]) => {
        if (annule) return null;
        setButin(d);
        setCapsules(c);
        return null;
      })
      .catch((e) => {
        if (!annule) setErreur(String(e));
      });
    return () => {
      annule = true;
    };
  }, [settings.gameDir]);

  const butinTrie = useMemo(() => {
    const q = filtre.trim().toLowerCase();
    return [...butin]
      .filter((d) => !q || (d.name ?? d.chara_id).toLowerCase().includes(q) || d.run_cond.toLowerCase().includes(q))
      .sort((a, b) => (b.share_pct ?? 0) - (a.share_pct ?? 0));
  }, [butin, filtre]);

  const capsulesTriees = useMemo(() => {
    const q = filtre.trim().toLowerCase();
    return [...capsules]
      .filter((c) => !q || c.table_id.toLowerCase().includes(q))
      .sort((a, b) => a.table_id.localeCompare(b.table_id) || (b.share_pct ?? 0) - (a.share_pct ?? 0));
  }, [capsules, filtre]);

  /** Nombre de tirages pour atteindre 50 % de chances d'obtenir au moins une fois l'entrée. */
  function tiragesPour50(part: number): string {
    if (part <= 0) return "—";
    const p = part / 100;
    return Math.ceil(Math.log(0.5) / Math.log(1 - p)).toLocaleString("fr-FR");
  }

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      {erreur && (
        <Alert variant="destructive">
          <AlertTitle>Tables de tirage indisponibles</AlertTitle>
          <AlertDescription>{erreur}</AlertDescription>
        </Alert>
      )}

      <div className="flex flex-wrap items-center gap-2">
        <Tabs value={onglet} onValueChange={(v) => v && setOnglet(v as Onglet)}>
          <TabsList>
            <TabsTrigger value="butin">Butin de match</TabsTrigger>
            <TabsTrigger value="capsules">Capsules</TabsTrigger>
          </TabsList>
        </Tabs>
        <Input
          placeholder="Filtrer…"
          value={filtre}
          onChange={(e) => setFiltre(e.target.value)}
          className="max-w-xs"
        />
        <Badge variant="secondary">
          {(onglet === "butin" ? butinTrie.length : capsulesTriees.length).toLocaleString("fr-FR")} entrée(s)
        </Badge>
      </div>

      <ScrollArea className="min-h-0 flex-1 rounded-xl border border-app-line bg-app-dark-box">
        <table className="w-full text-xs">
          <thead className="sticky top-0 bg-app-box/90 text-ink-faint">
            {onglet === "butin" ? (
              <tr>
                <th className="px-3 py-2 text-left">Personnage</th>
                <th className="px-3 py-2 text-right">Poids</th>
                <th className="px-3 py-2 text-right">Chance</th>
                <th className="px-3 py-2 text-right" title="Tirages pour 50 % de chances d'en obtenir au moins un">
                  50 % en
                </th>
                <th className="px-3 py-2 text-left">Condition</th>
              </tr>
            ) : (
              <tr>
                <th className="px-3 py-2 text-left">Table</th>
                <th className="px-3 py-2 text-right">Rang</th>
                <th className="px-3 py-2 text-right">Taux brut</th>
                <th className="px-3 py-2 text-right">Chance</th>
                <th className="px-3 py-2 text-right">50 % en</th>
              </tr>
            )}
          </thead>
          <tbody className="divide-y divide-app-line">
            {onglet === "butin"
              ? butinTrie.map((d, i) => (
                  <tr key={`${d.chara_id}-${i}`}>
                    <td className="px-3 py-1 text-ink">{d.name ?? d.chara_id}</td>
                    <td className="px-3 py-1 text-right tabular-nums text-ink-dull">
                      {(d.weight ?? 0).toLocaleString("fr-FR")}
                    </td>
                    <td className="px-3 py-1 text-right tabular-nums text-ink">
                      {(d.share_pct ?? 0).toFixed(3)} %
                    </td>
                    <td className="px-3 py-1 text-right tabular-nums text-ink-dull">
                      {tiragesPour50(d.share_pct ?? 0)}
                    </td>
                    <td className="px-3 py-1 truncate text-ink-faint" title={d.run_cond}>
                      {conditionLisible(d.run_cond)}
                    </td>
                  </tr>
                ))
              : capsulesTriees.map((c, i) => (
                  <tr key={`${c.table_id}-${c.rank}-${i}`}>
                    <td className="px-3 py-1 text-ink">{c.table_id}</td>
                    <td className="px-3 py-1 text-right tabular-nums text-ink-dull">{c.rank}</td>
                    <td className="px-3 py-1 text-right tabular-nums text-ink-dull">
                      {(c.rate ?? 0).toLocaleString("fr-FR")}
                    </td>
                    <td className="px-3 py-1 text-right tabular-nums text-ink">
                      {(c.share_pct ?? 0).toFixed(2)} %
                    </td>
                    <td className="px-3 py-1 text-right tabular-nums text-ink-dull">
                      {tiragesPour50(c.share_pct ?? 0)}
                    </td>
                  </tr>
                ))}
          </tbody>
        </table>
      </ScrollArea>
    </div>
  );
}
