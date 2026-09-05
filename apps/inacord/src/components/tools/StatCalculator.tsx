// Calculateur de statistiques — **une seule implémentation, deux points de montage** : l'onglet
// « Calculateur de stats » de `GameDataView` (où il vivait) et la vue Outils. Il était défini en
// local dans `GameDataView.tsx` ; l'y laisser aurait obligé la vue Outils à en écrire un second.
//
// C'est aussi l'outil du wiki (`/tools/stats`, `components/wiki/StatCalculator.tsx`) — sauf que
// celui-ci part d'un PERSONNAGE RÉEL et non de paramètres abstraits. Le web fait choisir
// « position / rareté / pattern de croissance » à la main parce que `growth_pattern` n'était pas
// exporté dans ses données ; ici, `api.gameDataCharaPicker` lit `chara_param` directement dans le
// VFS et `api.gameDataCalculateStats` applique `nie_core::growth::calculate_stats` sur les tables
// de croissance IEVR embarquées. On ne simule pas une courbe : on calcule celle du personnage.
import { useEffect, useState } from "react";

import { api, type CharaPicker, type StatBlock } from "@/lib/api";
import { useFiltered } from "@/lib/filtrage";
import { useSettings } from "@/lib/settings";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";

/** `rarityCode` → libellé FR (cf. doc Rust `game_data::calculate_character_stats`). */
export const RARITY_LABELS: [number, string][] = [
  [0, "N"], [2, "R"], [3, "SR"], [4, "SSR"], [5, "UR"], [6, "LR"], [7, "Legend"], [20, "BASARA"],
];

export function StatCalculator() {
  const settings = useSettings();
  const [roster, setRoster] = useState<CharaPicker[]>([]);
  const [rosterLoading, setRosterLoading] = useState(true);
  const [rosterError, setRosterError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<CharaPicker | null>(null);
  const [level, setLevel] = useState(50);
  const [rarity, setRarity] = useState(0);
  const [stats, setStats] = useState<StatBlock | null>(null);
  const [statsLoading, setStatsLoading] = useState(false);
  const [statsError, setStatsError] = useState<string | null>(null);

  useEffect(() => {
    setRosterLoading(true);
    api
      .gameDataCharaPicker(settings.gameDir)
      .then(setRoster)
      .catch((e) => setRosterError(String(e)))
      .finally(() => setRosterLoading(false));
  }, [settings.gameDir]);

  const filtered = useFiltered(roster, query, (c) => [c.name, c.main_position]);

  useEffect(() => {
    if (!selected) {
      setStats(null);
      return;
    }
    setStatsLoading(true);
    setStatsError(null);
    api
      .gameDataCalculateStats(selected.chara_param_id, level, rarity, settings.gameDir)
      .then(setStats)
      .catch((e) => setStatsError(String(e)))
      .finally(() => setStatsLoading(false));
  }, [selected, level, rarity, settings.gameDir]);

  return (
    <div className="grid min-h-0 flex-1 grid-cols-[minmax(240px,1fr)_1.2fr] gap-3">
      <div className="flex min-h-0 flex-col gap-2">
        <Input placeholder="Rechercher un personnage…" value={query} onChange={(e) => setQuery(e.target.value)} />
        {rosterError && (
          <Alert variant="destructive">
            <AlertTitle>Échec du décodage</AlertTitle>
            <AlertDescription>{rosterError}</AlertDescription>
          </Alert>
        )}
        <ScrollArea className="min-h-0 flex-1 rounded-2xl border border-app-line bg-app-dark-box">
          <div className="divide-y divide-app-line">
            {filtered.map((c) => (
              <button
                key={c.chara_param_id}
                className={`state-layer flex w-full items-center justify-between gap-2 px-3 py-2 text-left type-body-medium ${
                  selected?.chara_param_id === c.chara_param_id ? "bg-secondary-container text-on-secondary-container" : "text-on-surface"
                }`}
                onClick={() => setSelected(c)}
              >
                <span className="min-w-0 flex-1 truncate">{c.name}</span>
                <Badge variant="outline">{c.main_position}</Badge>
              </button>
            ))}
            {!rosterLoading && filtered.length === 0 && (
              <p className="p-4 type-body-small text-on-surface-variant">Aucun personnage ne correspond.</p>
            )}
          </div>
        </ScrollArea>
      </div>

      <div className="flex min-h-0 flex-col gap-3 rounded-2xl border border-app-line bg-app-dark-box p-4">
        {!selected ? (
          <p className="type-body-medium text-on-surface-variant">Sélectionnez un personnage à gauche.</p>
        ) : (
          <>
            <h3 className="type-title-small text-on-surface">
              {selected.name} <span className="type-label-small text-on-surface-variant">({selected.main_position})</span>
            </h3>
            <div className="flex flex-wrap items-end gap-4">
              <div className="space-y-1.5">
                <label className="type-label-small text-on-surface-variant" htmlFor="stat-level">
                  Niveau
                </label>
                <Input
                  id="stat-level"
                  type="number"
                  min={1}
                  max={99}
                  value={level}
                  onChange={(e) => setLevel(Math.min(99, Math.max(1, Number(e.target.value) || 1)))}
                  className="w-24"
                />
              </div>
              <div className="space-y-1.5">
                <span className="type-label-small text-on-surface-variant">Rareté</span>
                <Select value={String(rarity)} onValueChange={(v) => v && setRarity(Number(v))}>
                  <SelectTrigger className="w-32">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {RARITY_LABELS.map(([code, label]) => (
                      <SelectItem key={code} value={String(code)}>
                        {label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
            </div>

            {statsError && (
              <Alert variant="destructive">
                <AlertTitle>Échec du calcul</AlertTitle>
                <AlertDescription>{statsError}</AlertDescription>
              </Alert>
            )}
            {stats && (
              <div className="grid grid-cols-4 gap-2 type-body-medium">
                {(
                  [
                    ["Kick", stats.kc],
                    ["Control", stats.cr],
                    ["Technique", stats.tc],
                    ["Power", stats.pr],
                    ["Pressure", stats.ps],
                    ["Agility", stats.ag],
                    ["Intelligence", stats.it],
                  ] as [string, number][]
                ).map(([label, val]) => (
                  <div key={label} className="rounded-lg bg-surface-container p-2 text-center">
                    <div className="type-label-small text-on-surface-variant">{label}</div>
                    <div className="type-title-small text-on-surface">{val}</div>
                  </div>
                ))}
                <div className="col-span-4 rounded-lg bg-primary-container p-2 text-center text-on-primary-container">
                  <div className="type-label-small">Total</div>
                  <div className="type-title-medium">{stats.total}</div>
                </div>
              </div>
            )}
            {statsLoading && <p className="type-body-small text-on-surface-variant">calcul…</p>}
          </>
        )}
      </div>
    </div>
  );
}
