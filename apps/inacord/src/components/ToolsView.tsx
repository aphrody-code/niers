// Vue **Outils** — les cinq outils du wiki (`/tools`), réunis dans une seule vue à onglets, PLUS
// deux que le site n'a pas : « Progression » (courbe d'expérience, `chara_exp_table_config`) et
// « Probabilités » (butin de match et tirages de capsules, `soccer_drop_config` /
// `capsule_config`). Ces trois tables ne sont publiées nulle part sur le wiki ; elles sont lues
// ici directement du jeu monté.
//
// Le roster des outils d'équipe a DEUX sources (cf. le `useEffect` plus bas) : le miroir wiki
// quand il est configuré, sinon le jeu lui-même. Sans ce repli, l'absence de miroir rendait les
// quatre outils d'équipe muets sur une machine où le jeu, lui, était bien monté.
//
// ## Pourquoi une vue à onglets et non cinq entrées de barre latérale
//
// Trois raisons mesurées, pas une préférence :
//
//  1. la barre latérale porte déjà douze entrées, et son groupe « Outils » désigne les outils DU
//     DÉPÔT (mods, RE, Viola, Live mod, Lua). Y verser cinq outils de wiki mélangerait deux
//     natures d'objet dans le même groupe ;
//  2. `AppMenu.VIEW_TABS` n'attribue un accélérateur qu'aux **neuf premières** vues (`Ctrl+1…9`,
//     `useAppMenuShortcuts` ne lit qu'un chiffre) : cinq entrées de plus rendraient muettes cinq
//     vues existantes ;
//  3. les trois outils d'équipe partagent le MÊME roster (6 166 lignes du miroir). Chargé ici,
//     une fois, il sert les trois onglets ; réparti sur cinq vues, il serait rechargé cinq fois.
//
// Le calculateur de stats n'est pas dupliqué : c'est le composant `tools/StatCalculator`, celui-là
// même que monte l'onglet « Calculateur de stats » de `GameDataView`.
import { useEffect, useMemo, useState } from "react";

import { api } from "@/lib/api";
import { versJoueur, versJoueurDepuisJeu, type Joueur } from "@/lib/equipe";
import { useSettings } from "@/lib/settings";
import { wikiDb } from "@/lib/wikiDb";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { ComparatorPanel } from "@/components/tools/ComparatorPanel";
import { RandomTeamPanel } from "@/components/tools/RandomTeamPanel";
import { StatCalculator } from "@/components/tools/StatCalculator";
import { TeamBuilderPanel } from "@/components/tools/TeamBuilderPanel";
import { TranslatorPanel } from "@/components/tools/TranslatorPanel";
import { ProbabilitesPanel } from "@/components/tools/ProbabilitesPanel";
import { ProgressionPanel } from "@/components/tools/ProgressionPanel";

type Outil =
  | "traducteur"
  | "stats"
  | "comparateur"
  | "aleatoire"
  | "equipe"
  | "progression"
  | "probabilites";

const LIBELLES: Record<Outil, string> = {
  traducteur: "Traducteur",
  stats: "Calculateur de stats",
  comparateur: "Comparateur",
  aleatoire: "Équipe aléatoire",
  equipe: "Mon équipe",
  // Les deux suivants n'ont AUCUN équivalent sur le wiki : leurs tables
  // (`chara_exp_table_config`, `soccer_drop_config`, `capsule_config`) n'y sont pas publiées.
  progression: "Progression",
  probabilites: "Probabilités",
};

export function ToolsView({ onOpenSearch }: { onOpenSearch?: (query: string) => void }) {
  const settings = useSettings();
  const [outil, setOutil] = useState<Outil>("traducteur");
  const [roster, setRoster] = useState<Joueur[]>([]);
  /** D'où viennent les joueurs affichés — l'utilisatrice doit pouvoir le lire, pas le deviner. */
  const [sourceRoster, setSourceRoster] = useState<"miroir" | "jeu" | null>(null);
  const [erreur, setErreur] = useState<string | null>(null);
  const [chargement, setChargement] = useState(true);

  // Un seul chargement du roster pour les trois outils d'équipe. Les postes d'encadrement
  // (`Entraîneur`) sont écartés du vivier de joueurs : ils ne tiennent aucun créneau de terrain.
  //
  // Deux sources, dans cet ordre : le miroir wiki s'il est configuré (il porte la rareté réelle
  // et les stats de chaque exemplaire), sinon le JEU lui-même (`api.gameDataCharas`, stats Lv99
  // au rang UR calculées par les tables de croissance embarquées). Avant, l'absence de miroir
  // rendait les quatre outils d'équipe muets — alors que le jeu est monté et suffit.
  useEffect(() => {
    let annule = false;
    setChargement(true);
    setErreur(null);

    const chemin = settings.wikiDb.trim();
    const depuisLeJeu = () =>
      api.gameDataCharas(settings.gameDir).then((charas) => {
        if (annule) return null;
        setRoster(charas.map(versJoueurDepuisJeu));
        setSourceRoster("jeu");
        return null;
      });

    const promesse = chemin
      ? wikiDb
          .chargerRoster(chemin)
          .then((lignes) => {
            if (annule) return null;
            setRoster(lignes.map(versJoueur).filter((j) => j.poste !== "Entraîneur"));
            setSourceRoster("miroir");
            return null;
          })
          // Un miroir illisible (fichier déplacé, schéma d'une autre version) ne doit pas rendre
          // les outils inutilisables : on retombe sur le jeu et on le DIT.
          .catch(() => depuisLeJeu())
      : depuisLeJeu();

    promesse
      .catch((e) => {
        if (!annule) setErreur(String(e));
      })
      .finally(() => {
        if (!annule) setChargement(false);
      });
    return () => {
      annule = true;
    };
  }, [settings.wikiDb, settings.gameDir]);

  const onglets = useMemo(() => Object.entries(LIBELLES) as [Outil, string][], []);

  return (
    <div className="flex h-full min-h-0 flex-col gap-3 p-3">
      <div className="flex flex-wrap items-center gap-2">
        <Tabs value={outil} onValueChange={(v) => v && setOutil(v as Outil)}>
          <TabsList>
            {onglets.map(([cle, libelle]) => (
              <TabsTrigger key={cle} value={cle}>
                {libelle}
              </TabsTrigger>
            ))}
          </TabsList>
        </Tabs>
        {chargement ? (
          <Badge variant="outline">chargement du roster…</Badge>
        ) : (
          <>
            <Badge variant="secondary">{roster.length.toLocaleString("fr-FR")} joueurs</Badge>
            <Badge
              variant="outline"
              title={
                sourceRoster === "jeu"
                  ? "Roster décodé du jeu (chara_param + chara_base) — stats au niveau 99, rang UR, calculées par les tables de croissance embarquées"
                  : "Roster du miroir wiki — rareté et stats de chaque exemplaire"
              }
            >
              {sourceRoster === "jeu" ? "source : jeu" : "source : miroir wiki"}
            </Badge>
          </>
        )}
      </div>

      {erreur && (
        <Alert variant="destructive">
          <AlertTitle>Miroir wiki indisponible</AlertTitle>
          <AlertDescription>{erreur}</AlertDescription>
        </Alert>
      )}

      <div className="min-h-0 flex-1">
        {outil === "traducteur" && <TranslatorPanel onOpenCode={onOpenSearch} />}
        {outil === "stats" && <StatCalculator />}
        {outil === "comparateur" && <ComparatorPanel roster={roster} />}
        {outil === "aleatoire" && <RandomTeamPanel roster={roster} />}
        {outil === "equipe" && <TeamBuilderPanel roster={roster} />}
        {outil === "progression" && <ProgressionPanel />}
        {outil === "probabilites" && <ProbabilitesPanel />}
      </div>
    </div>
  );
}
