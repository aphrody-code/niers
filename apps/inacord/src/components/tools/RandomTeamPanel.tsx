// **Générateur d'équipe aléatoire** — une composition tirée au sort, poste par poste.
//
// Portage de `apps/azalee/components/wiki/RandomTeamGenerator.tsx` (1 337 lignes). La logique de
// tirage, de verrouillage et de filtrage vit dans `lib/equipe.ts` ; ce fichier est la surface.
//
// Trois écarts assumés avec le wiki :
//
//  1. **le filtre « style de jeu » n'est pas porté.** Il lit `sheetData.playstyle`, nul sur
//     6 166 lignes sur 6 166 du miroir : la garde `minCount` du wiki l'annule en silence, ce qui
//     donne l'illusion d'un filtre actif. Un filtre qui ne filtre rien vaut moins que pas de
//     filtre du tout.
//  2. **les filtres ignorés sont annoncés.** Le wiki abandonne un filtre trop restrictif sans
//     rien dire ; ici, `filtrerVivier` rend la liste des filtres écartés et la vue l'affiche.
//  3. **les 91 formations réelles**, pas douze écrites à la main. `FORMATIONS` de
//     `@rosegriffon/azalee/game` porte 8 dispositions héritées et 83 décodées du jeu par
//     `nie-data` (`formation_config`), coordonnées `f32` comprises.
import { useCallback, useEffect, useMemo, useState } from "react";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { toast } from "sonner";

import {
  FORMATIONS,
  LIBELLE_POSTE,
  cheminVisage,
  filtrerVivier,
  genererEquipe,
  seriesDisponibles,
  type FiltresGenerateur,
  type Joueur,
  type TeamMember,
} from "@/lib/equipe";
import { useSettings } from "@/lib/settings";
import { useThumbnail } from "@/lib/thumbs";
import { Badge } from "@/components/ui/badge";
import { Icon } from "@/components/ui/Icon";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { encodeTeamCode } from "@rosegriffon/azalee/game/team-code";

/** Éléments proposés — libellés FR du miroir, ce sont les valeurs réellement stockées. */
const ELEMENTS = ["Feu", "Vent", "Forêt", "Montagne"];

/** Genres du miroir : `M` / `F` (et non `0` / `1` comme le modèle du wiki). */
const GENRES: { valeur: string; libelle: string }[] = [
  { valeur: "M", libelle: "Garçon" },
  { valeur: "F", libelle: "Fille" },
];

/** Raretés telles que `rarity_label` les écrit. */
const RARETES = ["Normal", "Expérimenté", "Héros", "BASARA"];

/** Carte d'un joueur tiré, avec son verrou. */
function CarteJoueur({
  membre,
  verrouille,
  onBasculer,
  gameDir,
}: {
  membre: TeamMember;
  verrouille: boolean;
  onBasculer: () => void;
  gameDir?: string;
}) {
  const { ref, src } = useThumbnail(cheminVisage(membre.internalCode ?? null) ?? "", "g4tx", gameDir);
  return (
    <div className="relative overflow-hidden rounded-lg border border-app-line bg-app-dark-box">
      <div ref={ref} className="aspect-square w-full overflow-hidden bg-surface-container-highest">
        {src ? (
          <img src={src} alt="" className="h-full w-full object-contain" />
        ) : (
          <div className="flex h-full items-center justify-center">
            <Icon name="person" size={20} className="text-on-surface-variant/40" />
          </div>
        )}
      </div>
      <div className="space-y-0.5 p-1.5">
        <p className="truncate type-label-small text-on-surface" title={membre.name}>
          {membre.name}
        </p>
        <div className="flex items-center gap-1">
          <Badge variant="outline">{LIBELLE_POSTE[membre.position] ?? membre.position}</Badge>
          <span className="truncate type-label-small text-on-surface-variant">{membre.rarity}</span>
        </div>
      </div>
      <button
        type="button"
        aria-pressed={verrouille}
        aria-label={verrouille ? `Déverrouiller ${membre.name}` : `Verrouiller ${membre.name}`}
        title={verrouille ? "Déverrouiller" : "Garder ce joueur au prochain tirage"}
        className={`absolute right-1 top-1 rounded-full p-1 ${
          verrouille ? "bg-primary text-on-primary" : "bg-app/70 text-on-surface-variant"
        }`}
        onClick={onBasculer}
      >
        <Icon name={verrouille ? "lock" : "lock_open"} size={13} />
      </button>
    </div>
  );
}

export function RandomTeamPanel({ roster }: { roster: Joueur[] }) {
  const settings = useSettings();
  const [indexFormation, setIndexFormation] = useState(0);
  const [filtres, setFiltres] = useState<FiltresGenerateur>({
    element: null,
    genre: null,
    rarete: null,
    serie: null,
  });
  const [equipe, setEquipe] = useState<Record<string, TeamMember>>({});
  const [verrous, setVerrous] = useState<Set<string>>(new Set());

  const formation = FORMATIONS[indexFormation] ?? FORMATIONS[0];
  const series = useMemo(() => seriesDisponibles(roster), [roster]);

  /** Filtres qui n'ont pas pu s'appliquer, mesurés sur le vivier entier (pas par poste). */
  const ignores = useMemo(
    () => filtrerVivier(roster, filtres, 11).ignores,
    [roster, filtres],
  );

  const tirerEquipe = useCallback(() => {
    const gardes: Record<string, TeamMember> = {};
    for (const creneau of verrous) {
      const m = equipe[creneau];
      if (m) gardes[creneau] = m;
    }
    setEquipe(genererEquipe(roster, formation, filtres, gardes));
  }, [roster, formation, filtres, equipe, verrous]);

  // Premier tirage dès que le roster arrive, puis à chaque changement de formation ou de filtre.
  useEffect(() => {
    if (roster.length === 0) return;
    setEquipe(genererEquipe(roster, formation, filtres, {}));
    setVerrous(new Set());
    // Volontairement sans `tirerEquipe` : ce déclencheur repart d'une composition VIERGE (les
    // verrous d'une formation précédente ne désignent plus les mêmes créneaux).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [roster, indexFormation, filtres]);

  function basculerVerrou(creneau: string) {
    setVerrous((prec) => {
      const suivant = new Set(prec);
      if (suivant.has(creneau)) suivant.delete(creneau);
      else suivant.add(creneau);
      return suivant;
    });
  }

  async function copierCode() {
    const slots = Object.values(equipe).map((m) => ({ slot: m.slot, charaId: m.charaId }));
    const code = encodeTeamCode(formation.id, slots);
    try {
      await writeText(code);
      toast.success("Code d'équipe copié — collable dans le constructeur, ici comme sur le wiki");
    } catch (e) {
      toast.error(String(e));
    }
  }

  const membres = formation.positions.map((p) => ({
    creneau: `field-${p.index}`,
    role: p.role,
    membre: equipe[`field-${p.index}`] ?? null,
  }));

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      <div className="flex flex-wrap items-center gap-2">
        <Select
          value={String(indexFormation)}
          onValueChange={(v) => v && setIndexFormation(Number(v))}
        >
          <SelectTrigger className="w-56">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {FORMATIONS.map((f, i) => (
              <SelectItem key={f.id} value={String(i)}>
                {f.name}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>

        <button
          type="button"
          className="state-layer rounded-lg bg-primary px-3 py-1.5 type-label-medium text-on-primary"
          onClick={tirerEquipe}
        >
          <Icon name="casino" size={16} /> Retirer au sort
        </button>
        <button
          type="button"
          className="state-layer rounded-lg px-3 py-1.5 type-label-medium text-on-surface-variant"
          onClick={copierCode}
        >
          <Icon name="content_copy" size={16} /> Copier le code d'équipe
        </button>
        <Badge variant="secondary">{formation.positions.length} postes</Badge>
        <Badge variant="outline">{roster.length.toLocaleString("fr-FR")} joueurs</Badge>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        {[
          { cle: "element" as const, vide: "Tous éléments", options: ELEMENTS.map((e) => ({ valeur: e, libelle: e })) },
          { cle: "genre" as const, vide: "Tous genres", options: GENRES },
          { cle: "rarete" as const, vide: "Toutes raretés", options: RARETES.map((r) => ({ valeur: r, libelle: r })) },
          { cle: "serie" as const, vide: "Toutes séries", options: series.map((s) => ({ valeur: s, libelle: s })) },
        ].map(({ cle, vide, options }) => (
          <Select
            key={cle}
            value={filtres[cle] ?? "*"}
            onValueChange={(v) =>
              setFiltres((f) => ({ ...f, [cle]: !v || v === "*" ? null : v }))
            }
          >
            <SelectTrigger className="w-44">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="*">{vide}</SelectItem>
              {options.map((o) => (
                <SelectItem key={o.valeur} value={o.valeur}>
                  {o.libelle}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        ))}
        {ignores.length > 0 && (
          <Badge variant="outline" title="Trop peu de joueurs correspondants pour composer l'équipe">
            filtre(s) ignoré(s) : {ignores.join(", ")}
          </Badge>
        )}
      </div>

      <ScrollArea className="min-h-0 flex-1 rounded-2xl border border-app-line bg-app-dark-box p-3">
        {roster.length === 0 ? (
          <p className="type-body-medium text-on-surface-variant">
            Roster vide — le miroir wiki est-il configuré ?
          </p>
        ) : (
          <div className="grid grid-cols-[repeat(auto-fill,minmax(120px,1fr))] gap-2">
            {membres.map(({ creneau, role, membre }) =>
              membre ? (
                <CarteJoueur
                  key={creneau}
                  membre={membre}
                  verrouille={verrous.has(creneau)}
                  onBasculer={() => basculerVerrou(creneau)}
                  gameDir={settings.gameDir}
                />
              ) : (
                <div
                  key={creneau}
                  className="flex aspect-square items-center justify-center rounded-lg border border-dashed border-app-line type-label-small text-on-surface-variant"
                >
                  {LIBELLE_POSTE[role] ?? role}
                </div>
              ),
            )}
          </div>
        )}
      </ScrollArea>
    </div>
  );
}
