// **Progression** — combien d'expérience sépare deux niveaux, et à quoi ressemble la courbe.
//
// Outil ABSENT du wiki : la table `chara_exp_table_config` n'y est pas publiée. Elle est ici lue
// directement du jeu (`api.gameDataExpTable`), et le cumul est calculé, pas recopié.
import { useEffect, useMemo, useState } from "react";

import { api, type ExpLevel } from "@/lib/api";
import { useSettings } from "@/lib/settings";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";

/** Borne un niveau saisi à l'intervalle réellement couvert par la table. */
function borner(v: number, min: number, max: number) {
  return Math.min(max, Math.max(min, Number.isFinite(v) ? v : min));
}

export function ProgressionPanel() {
  const settings = useSettings();
  const [table, setTable] = useState<ExpLevel[]>([]);
  const [erreur, setErreur] = useState<string | null>(null);
  const [depart, setDepart] = useState(1);
  const [arrivee, setArrivee] = useState(99);

  useEffect(() => {
    let annule = false;
    api
      .gameDataExpTable(settings.gameDir)
      .then((t) => {
        if (!annule) setTable(t);
        return null;
      })
      .catch((e) => {
        if (!annule) setErreur(String(e));
      });
    return () => {
      annule = true;
    };
  }, [settings.gameDir]);

  const niveauMax = table.length ? Math.max(...table.map((e) => e.level ?? 0)) : 99;
  const cumulDe = (niveau: number) =>
    table.find((e) => e.level === niveau)?.cumulative ?? 0;

  const besoin = useMemo(() => {
    const a = borner(depart, 1, niveauMax);
    const b = borner(arrivee, 1, niveauMax);
    return Math.max(0, (cumulDe(b) ?? 0) - (cumulDe(a) ?? 0));
  }, [depart, arrivee, table, niveauMax]);

  // Repère visuel : la part de l'EXP totale déjà franchie à chaque palier.
  const total = cumulDe(niveauMax) || 1;

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      {erreur && (
        <Alert variant="destructive">
          <AlertTitle>Table d'expérience indisponible</AlertTitle>
          <AlertDescription>{erreur}</AlertDescription>
        </Alert>
      )}

      <div className="flex flex-wrap items-end gap-3 rounded-xl border border-app-line bg-app-box/60 p-3">
        <label className="flex flex-col gap-1 text-xs text-ink-faint">
          Du niveau
          <Input
            type="number"
            min={1}
            max={niveauMax}
            value={depart}
            onChange={(e) => setDepart(Number(e.target.value))}
            className="w-24"
          />
        </label>
        <label className="flex flex-col gap-1 text-xs text-ink-faint">
          Au niveau
          <Input
            type="number"
            min={1}
            max={niveauMax}
            value={arrivee}
            onChange={(e) => setArrivee(Number(e.target.value))}
            className="w-24"
          />
        </label>
        <div className="flex flex-col gap-1">
          <span className="text-xs text-ink-faint">Expérience nécessaire</span>
          <span className="text-2xl font-semibold tabular-nums text-ink">
            {besoin.toLocaleString("fr-FR")}
          </span>
        </div>
        <Badge variant="outline" title="Dernier palier de la table du jeu">
          niveau max {niveauMax}
        </Badge>
        <Badge variant="secondary" title="Expérience cumulée du niveau 1 au niveau max">
          {total.toLocaleString("fr-FR")} au total
        </Badge>
      </div>

      <ScrollArea className="min-h-0 flex-1 rounded-xl border border-app-line bg-app-dark-box">
        <table className="w-full text-xs">
          <thead className="sticky top-0 bg-app-box/90 text-ink-faint">
            <tr>
              <th className="px-3 py-2 text-left">Niveau</th>
              <th className="px-3 py-2 text-right">EXP du palier</th>
              <th className="px-3 py-2 text-right">EXP cumulée</th>
              <th className="px-3 py-2 text-left">Part de la courbe</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-app-line">
            {table.map((e) => {
              const dans = (e.level ?? 0) > Math.min(depart, arrivee) && (e.level ?? 0) <= Math.max(depart, arrivee);
              return (
                <tr key={e.level} className={dans ? "bg-accent/10" : undefined}>
                  <td className="px-3 py-1 tabular-nums text-ink">{e.level}</td>
                  <td className="px-3 py-1 text-right tabular-nums text-ink-dull">
                    {(e.need_exp ?? 0).toLocaleString("fr-FR")}
                  </td>
                  <td className="px-3 py-1 text-right tabular-nums text-ink-dull">
                    {(e.cumulative ?? 0).toLocaleString("fr-FR")}
                  </td>
                  <td className="px-3 py-1">
                    <div className="h-1.5 w-full rounded bg-app-selected/20">
                      <div
                        className="h-full rounded bg-accent"
                        style={{ width: `${((e.cumulative ?? 0) / total) * 100}%` }}
                      />
                    </div>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </ScrollArea>
    </div>
  );
}
