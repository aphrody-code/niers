// **Comparateur de personnages** — deux joueurs côte à côte, stats et techniques.
//
// Portage de `apps/azalee/components/wiki/CharacterComparator.tsx` (816 lignes) et de la page
// serveur `app/tools/compare/page.tsx` qui l'alimentait.
//
// ## Le web interpole, l'explorateur calcule
//
// `interpolateStats()` du wiki reconstruit une courbe par segments entre les paliers `lv1`,
// `lv30`, `lv50` et `lv99` de la base — sauf que dans le miroir, `$.stats.lv30` est **nul sur
// 6 166 lignes sur 6 166** (relevé le 2026-09-02). La branche « paliers complets » n'est donc
// jamais prise : le wiki retombe sur une droite lv1→lv99, et affiche une valeur inventée à tout
// niveau intermédiaire.
//
// Ici, `api.gameDataCalculateStats` appelle `nie_core::growth::calculate_stats` sur les tables de
// croissance IEVR embarquées, à partir du `chara_param_id` — qui est exactement
// `inagle_characters.id` (`0x23DC2602`, vérifié). Le niveau demandé donne la vraie valeur, pas
// une interpolation. Le repli sur les colonnes Lv99 du miroir n'existe que si le VFS n'est pas
// monté, et il est ANNONCÉ plutôt que silencieux.
//
// Les techniques viennent de `wikiDb.techniquesDuPersonnage` : deux requêtes au total, là où la
// page serveur appelait `wikiService.getSkill` une fois par technique.
import { useEffect, useMemo, useState } from "react";

import { api, type StatBlock } from "@/lib/api";
import { LIBELLE_POSTE, cheminVisage, codePoste, type Joueur } from "@/lib/equipe";
import { useFiltered } from "@/lib/filtrage";
import { useSettings } from "@/lib/settings";
import { useThumbnail } from "@/lib/thumbs";
import { wikiDb } from "@/lib/wikiDb";
import type { LigneTechnique } from "@/lib/wikiQueries";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Icon } from "@/components/ui/Icon";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Slider } from "@/components/ui/slider";

/** Les sept stats, dans l'ordre du jeu, avec la clé du bloc rendu par le moteur. */
const STATS: { cle: keyof Omit<StatBlock, "total">; libelle: string }[] = [
  { cle: "kc", libelle: "Frappe" },
  { cle: "cr", libelle: "Contrôle" },
  { cle: "tc", libelle: "Technique" },
  { cle: "pr", libelle: "Pression" },
  { cle: "ps", libelle: "Physique" },
  { cle: "ag", libelle: "Agilité" },
  { cle: "it", libelle: "Intelligence" },
];

/** Stats Lv99 du miroir, remises au format du moteur — repli quand le VFS n'est pas monté. */
function blocDepuisMiroir(j: Joueur): StatBlock {
  const s = j.stats;
  return {
    kc: s.kick,
    cr: s.control,
    tc: s.technique,
    pr: s.pressure,
    ps: s.physical,
    ag: s.agility,
    it: s.intelligence,
    total:
      s.kick + s.control + s.technique + s.pressure + s.physical + s.agility + s.intelligence,
  };
}

/** Sélecteur de personnage : recherche + liste, avec le visage décodé du VFS. */
function Selecteur({
  titre,
  roster,
  choisi,
  onChoisir,
  gameDir,
}: {
  titre: string;
  roster: Joueur[];
  choisi: Joueur | null;
  onChoisir: (j: Joueur) => void;
  gameDir?: string;
}) {
  const [requete, setRequete] = useState("");
  const filtres = useFiltered(roster, requete, (j) => [j.nom, j.poste, j.element, j.rarete]);

  return (
    <div className="flex min-h-0 flex-col gap-2">
      <div className="flex items-center gap-2">
        <span className="type-label-medium text-on-surface-variant">{titre}</span>
        {choisi && <Badge variant="secondary">{choisi.nom}</Badge>}
      </div>
      <Input
        placeholder="Rechercher…"
        value={requete}
        onChange={(e) => setRequete(e.target.value)}
      />
      <ScrollArea className="h-40 min-h-0 rounded-xl border border-app-line bg-app-dark-box">
        <div className="divide-y divide-app-line">
          {filtres.slice(0, 300).map((j) => (
            <button
              key={j.id}
              type="button"
              className={`state-layer flex w-full items-center gap-2 px-2 py-1.5 text-left type-body-small ${
                choisi?.id === j.id
                  ? "bg-secondary-container text-on-secondary-container"
                  : "text-on-surface"
              }`}
              onClick={() => onChoisir(j)}
            >
              <span className="min-w-0 flex-1 truncate">{j.nom}</span>
              <Badge variant="outline">{LIBELLE_POSTE[codePoste(j.poste)] ?? j.poste}</Badge>
              <span className="w-16 shrink-0 truncate type-label-small text-on-surface-variant">
                {j.element}
              </span>
            </button>
          ))}
          {filtres.length === 0 && (
            <p className="p-3 type-body-small text-on-surface-variant">Aucun personnage.</p>
          )}
        </div>
      </ScrollArea>
      {choisi && <Visage joueur={choisi} gameDir={gameDir} />}
    </div>
  );
}

/** Portrait décodé du VFS (`10_icon_chr/face/<code>_l.g4tx`) — aucun réseau. */
function Visage({ joueur, gameDir }: { joueur: Joueur; gameDir?: string }) {
  const chemin = cheminVisage(joueur.code) ?? "";
  const { ref, src } = useThumbnail(chemin, "g4tx", gameDir);
  return (
    <div className="flex items-center gap-3 rounded-xl border border-app-line bg-app-dark-box p-2">
      <div ref={ref} className="size-16 shrink-0 overflow-hidden rounded-lg bg-surface-container-highest">
        {src ? (
          <img src={src} alt="" className="h-full w-full object-contain" />
        ) : (
          <div className="flex h-full items-center justify-center">
            <Icon name="person" size={22} className="text-on-surface-variant/50" />
          </div>
        )}
      </div>
      <div className="min-w-0">
        <p className="truncate type-title-small text-on-surface">{joueur.nom}</p>
        <p className="truncate type-label-small text-on-surface-variant">
          {joueur.element} · {joueur.poste} · {joueur.rarete}
          {joueur.serie ? ` · ${joueur.serie}` : ""}
        </p>
        {joueur.code && (
          <p className="truncate type-label-small text-on-surface-variant">{joueur.code}</p>
        )}
      </div>
    </div>
  );
}

/** Liste de techniques d'un côté, avec surlignage de celles partagées. */
function Techniques({
  liste,
  communes,
  titre,
}: {
  liste: LigneTechnique[];
  communes: ReadonlySet<string>;
  titre: string;
}) {
  return (
    <div className="min-w-0 space-y-1">
      <p className="type-label-small uppercase text-on-surface-variant">{titre}</p>
      {liste.length === 0 ? (
        <p className="type-body-small italic text-on-surface-variant/60">Aucune technique</p>
      ) : (
        liste.map((t) => {
          const nom = t.name_fr || t.name_en || t.id;
          const commune = communes.has(nom.toLowerCase());
          return (
            <div
              key={t.id}
              className={`flex items-center gap-2 rounded-md border border-app-line px-2 py-1 type-body-small ${
                commune ? "ring-1 ring-accent/40" : ""
              }`}
            >
              <span className="min-w-0 flex-1 truncate text-on-surface">{nom}</span>
              {t.element && (
                <span className="shrink-0 type-label-small text-on-surface-variant">{t.element}</span>
              )}
              {t.power_max !== null && (
                <span className="shrink-0 tabular-nums type-label-small text-on-surface">
                  {t.power_max}
                </span>
              )}
              {t.tp_cost !== null && (
                <span className="shrink-0 type-label-small text-on-surface-variant">
                  {t.tp_cost}T
                </span>
              )}
              {commune && <Icon name="link" size={12} className="shrink-0 text-accent" />}
            </div>
          );
        })
      )}
    </div>
  );
}

export function ComparatorPanel({ roster }: { roster: Joueur[] }) {
  const settings = useSettings();
  const [gauche, setGauche] = useState<Joueur | null>(null);
  const [droite, setDroite] = useState<Joueur | null>(null);
  const [niveau, setNiveau] = useState(99);
  const [blocs, setBlocs] = useState<{ g: StatBlock | null; d: StatBlock | null }>({
    g: null,
    d: null,
  });
  /** Vrai quand le moteur de croissance n'a pas répondu et qu'on affiche les Lv99 du miroir. */
  const [repli, setRepli] = useState(false);
  const [techG, setTechG] = useState<LigneTechnique[]>([]);
  const [techD, setTechD] = useState<LigneTechnique[]>([]);

  // Stats au niveau demandé — le moteur d'abord, le miroir en repli ANNONCÉ.
  useEffect(() => {
    let annule = false;
    async function calculer(j: Joueur | null): Promise<{ bloc: StatBlock | null; repli: boolean }> {
      if (!j) return { bloc: null, repli: false };
      try {
        const bloc = await api.gameDataCalculateStats(
          j.id,
          niveau,
          j.codeRarete ?? 0,
          settings.gameDir,
        );
        return { bloc, repli: false };
      } catch {
        return { bloc: blocDepuisMiroir(j), repli: true };
      }
    }
    Promise.all([calculer(gauche), calculer(droite)]).then(([g, d]) => {
      if (!annule) {
        setBlocs({ g: g.bloc, d: d.bloc });
        setRepli(g.repli || d.repli);
      }
      return null;
    });
    return () => {
      annule = true;
    };
  }, [gauche, droite, niveau, settings.gameDir]);

  // Techniques — deux requêtes par personnage, pas une par technique.
  useEffect(() => {
    const chemin = settings.wikiDb.trim();
    if (!chemin) return;
    let annule = false;
    (async () => {
      const [g, d] = await Promise.all([
        gauche ? wikiDb.techniquesDuPersonnage(chemin, gauche.id).catch(() => []) : [],
        droite ? wikiDb.techniquesDuPersonnage(chemin, droite.id).catch(() => []) : [],
      ]);
      if (annule) return;
      setTechG(g);
      setTechD(d);
    })();
    return () => {
      annule = true;
    };
  }, [gauche, droite, settings.wikiDb]);

  const communes = useMemo(() => {
    const aGauche = new Set(techG.map((t) => (t.name_fr || t.name_en || t.id).toLowerCase()));
    return new Set(
      techD
        .map((t) => (t.name_fr || t.name_en || t.id).toLowerCase())
        .filter((n) => aGauche.has(n)),
    );
  }, [techG, techD]);

  const victoires = useMemo(() => {
    if (!blocs.g || !blocs.d) return { g: 0, d: 0 };
    let g = 0;
    let d = 0;
    for (const { cle } of STATS) {
      if (blocs.g[cle] > blocs.d[cle]) g++;
      else if (blocs.g[cle] < blocs.d[cle]) d++;
    }
    return { g, d };
  }, [blocs]);

  /** Échelle des barres : la plus grande stat affichée, au minimum 100 pour éviter un plein écran
   * de barres saturées sur un personnage de bas niveau. */
  const echelle = useMemo(() => {
    const valeurs = STATS.flatMap(({ cle }) => [blocs.g?.[cle] ?? 0, blocs.d?.[cle] ?? 0]);
    return Math.max(100, ...valeurs);
  }, [blocs]);

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <div className="grid gap-3 md:grid-cols-2">
        <Selecteur
          titre="Personnage 1"
          roster={roster}
          choisi={gauche}
          onChoisir={setGauche}
          gameDir={settings.gameDir}
        />
        <Selecteur
          titre="Personnage 2"
          roster={roster}
          choisi={droite}
          onChoisir={setDroite}
          gameDir={settings.gameDir}
        />
      </div>

      <div className="flex items-center gap-3">
        <span className="type-label-medium text-on-surface-variant">Niveau</span>
        <Slider
          className="max-w-md flex-1"
          min={1}
          max={99}
          step={1}
          value={[niveau]}
          onValueChange={(v) => setNiveau((Array.isArray(v) ? v[0] : v) ?? niveau)}
        />
        <Badge variant="secondary">Lv {niveau}</Badge>
      </div>

      {repli && (
        <Alert>
          <AlertTitle>Moteur de croissance indisponible</AlertTitle>
          <AlertDescription>
            Les stats affichées sont les Lv99 du miroir, quel que soit le niveau choisi —
            `api.gameDataCalculateStats` n'a pas répondu (VFS non monté ?).
          </AlertDescription>
        </Alert>
      )}

      <ScrollArea className="min-h-0 flex-1 rounded-2xl border border-app-line bg-app-dark-box p-3">
        {!gauche || !droite ? (
          <p className="type-body-medium text-on-surface-variant">
            Choisissez deux personnages pour les comparer.
          </p>
        ) : (
          <div className="space-y-4">
            <div className="flex items-center justify-between">
              <h3 className="type-title-small text-on-surface">Statistiques · Lv {niveau}</h3>
              <span className="tabular-nums type-label-medium text-on-surface-variant">
                {victoires.g} — {victoires.d}
              </span>
            </div>

            <div className="space-y-1.5">
              {STATS.map(({ cle, libelle }) => {
                const vg = blocs.g?.[cle] ?? 0;
                const vd = blocs.d?.[cle] ?? 0;
                const ecart = vd - vg;
                return (
                  <div key={cle} className="space-y-0.5">
                    <div className="flex items-center gap-2 type-label-small">
                      <span className="w-24 shrink-0 uppercase text-on-surface-variant">
                        {libelle}
                      </span>
                      <span
                        className={`w-10 text-right tabular-nums ${ecart < 0 ? "font-bold text-accent" : "text-on-surface"}`}
                      >
                        {vg}
                      </span>
                      <span className="text-on-surface-variant/50">vs</span>
                      <span
                        className={`w-10 tabular-nums ${ecart > 0 ? "font-bold text-accent" : "text-on-surface"}`}
                      >
                        {vd}
                      </span>
                      <span className="w-12 text-right tabular-nums text-on-surface-variant">
                        {ecart > 0 ? `+${ecart}` : ecart === 0 ? "=" : ecart}
                      </span>
                    </div>
                    <div className="flex h-2 gap-1">
                      <div className="flex flex-1 justify-end overflow-hidden rounded-full bg-surface-container-highest">
                        <div
                          className="h-full rounded-full bg-primary"
                          style={{ width: `${Math.min(100, (vg / echelle) * 100)}%` }}
                        />
                      </div>
                      <div className="flex-1 overflow-hidden rounded-full bg-surface-container-highest">
                        <div
                          className="h-full rounded-full bg-tertiary"
                          style={{ width: `${Math.min(100, (vd / echelle) * 100)}%` }}
                        />
                      </div>
                    </div>
                  </div>
                );
              })}
              <div className="flex items-center justify-between border-t border-app-line pt-2 type-label-medium">
                <span className="uppercase text-on-surface">Total</span>
                <span className="tabular-nums text-on-surface">
                  {blocs.g?.total ?? 0} vs {blocs.d?.total ?? 0}
                </span>
              </div>
            </div>

            <div>
              <div className="mb-2 flex items-center gap-2">
                <h3 className="type-title-small text-on-surface">Techniques</h3>
                {communes.size > 0 && (
                  <Badge variant="outline">{communes.size} en commun</Badge>
                )}
              </div>
              <div className="grid gap-3 md:grid-cols-2">
                <Techniques liste={techG} communes={communes} titre={gauche.nom} />
                <Techniques liste={techD} communes={communes} titre={droite.nom} />
              </div>
            </div>
          </div>
        )}
      </ScrollArea>
    </div>
  );
}
