// Vue **Tableau de bord** — la page d'accueil de l'explorateur : ce que la machine sait faire,
// mesuré, avant d'ouvrir quoi que ce soit.
//
// ## Pourquoi cette vue existe
//
// Les quatre sources de l'application (VFS du jeu, miroir du wiki, base de reverse, index VFS)
// se résolvent chacune de leur côté, silencieusement. Une source absente ne se voyait qu'en
// arrivant dans l'onglet qui en dépend — et parfois même pas : l'index VFS comptait 0 ligne sur
// cette machine, la recherche par code retombait sur une approximation, et rien ne le disait.
//
// Chaque chiffre affiché ici est LU au montage, jamais mémorisé dans le code : `api.stats` pour
// le VFS, `wikiDb.stats` / `reDb.stats` pour les deux bases, `vfsIndexDb.meta` pour l'index,
// `api.forgeReport` pour la forge. Une carte sans donnée dit pourquoi, et ce qu'il faut faire.
import { useEffect, useMemo, useState } from "react";

import { api, type VfsStats } from "@/lib/api";
import { useSettings } from "@/lib/settings";
import { wikiDb, type StatsMiroir } from "@/lib/wikiDb";
import { defaultReDbPath, reDb, type ReStats, type StatutForge } from "@/lib/reDb";
import { vfsIndexDb, type VfsIndexMeta } from "@/lib/vfsIndexDb";
import { modsDb } from "@/lib/modsDb";
import { useT } from "@/lib/i18n";
import { animeDb, defaultAnimeDbPath } from "@/lib/animeDb";
import { vue } from "@/lib/vues";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Icon } from "@/components/ui/Icon";

/** Disponibilité d'une capacité : ce qui décide de la pastille et du ton de la carte. */
type Etat = "pret" | "partiel" | "absent";

const TON: Record<Etat, { badge: string; libelle: string; icone: string }> = {
  pret: { badge: "bg-emerald-500/15 text-emerald-500 border-emerald-500/30", libelle: "prêt", icone: "check_circle" },
  partiel: { badge: "bg-amber-500/15 text-amber-500 border-amber-500/30", libelle: "partiel", icone: "warning" },
  absent: { badge: "bg-muted text-muted-foreground border-border", libelle: "indisponible", icone: "block" },
};

function nb(n: number | null | undefined): string {
  return n === null || n === undefined ? "—" : n.toLocaleString("fr-FR");
}

/** Une carte du socle : un chiffre gros, sa source, et ce qui manque le cas échéant. */
function CarteSource(props: {
  titre: string;
  icone: string;
  etat: Etat;
  valeur: string;
  detail: string;
  sous?: string[];
  action?: { libelle: string; onClick: () => void };
}) {
  const ton = TON[props.etat];
  return (
    <Card className="gap-3 py-4">
      <CardHeader className="px-4">
        <CardTitle className="flex items-center gap-2 text-sm font-medium">
          <Icon name={props.icone} size={16} className="text-muted-foreground" />
          <span className="flex-1 truncate">{props.titre}</span>
          <Badge variant="outline" className={ton.badge}>
            {ton.libelle}
          </Badge>
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-1 px-4">
        <div className="text-2xl font-semibold tabular-nums">{props.valeur}</div>
        <div className="text-xs text-muted-foreground">{props.detail}</div>
        {props.sous?.map((ligne) => (
          <div key={ligne} className="truncate font-mono text-[11px] text-muted-foreground/80" title={ligne}>
            {ligne}
          </div>
        ))}
        {props.action && (
          <Button size="sm" variant="outline" className="mt-2 h-7 text-xs" onClick={props.action.onClick}>
            {props.action.libelle}
          </Button>
        )}
      </CardContent>
    </Card>
  );
}

/**
 * Une carte d'onglet : ce que l'onglet sait faire ICI, et le clic qui y mène.
 *
 * Ni le titre ni l'icône ne sont passés — ils viennent du registre des vues (`lib/vues.ts`), le
 * même que lisent la barre latérale, le menu Affichage et la palette. Seuls la mesure et son
 * interprétation appartiennent à cette page.
 */
function CarteOnglet(props: { id: string; etat: Etat; metrique: string; note?: string; onClick: () => void }) {
  const t = useT();
  const ton = TON[props.etat];
  const v = vue(props.id);
  return (
    <button
      type="button"
      onClick={props.onClick}
      className="group flex w-full flex-col gap-1 rounded-lg border border-border bg-card p-3 text-left transition hover:border-accent hover:bg-accent/5"
    >
      <div className="flex items-center gap-2">
        <Icon name={v?.icone ?? "article"} size={16} className="text-muted-foreground group-hover:text-accent" />
        <span className="flex-1 truncate text-sm font-medium">{v ? t(v.cle) : props.id}</span>
        <Icon name={ton.icone} size={14} className={props.etat === "pret" ? "text-emerald-500" : props.etat === "partiel" ? "text-amber-500" : "text-muted-foreground/50"} />
      </div>
      <div className="text-lg font-semibold tabular-nums">{props.metrique}</div>
      {/* Une note propre à l'état de CETTE machine si la carte en fournit une (« le jeu n'est pas
          lancé »), sinon ce que la vue fait en général — jamais de case vide. */}
      <div className="line-clamp-2 text-xs text-muted-foreground">{props.note ?? v?.description}</div>
    </button>
  );
}

export function DashboardView({ onSelectTab }: { onSelectTab: (id: string) => void }) {
  const settings = useSettings();
  const [vfs, setVfs] = useState<VfsStats | null>(null);
  const [vfsErreur, setVfsErreur] = useState<string | null>(null);
  const [miroir, setMiroir] = useState<StatsMiroir | null>(null);
  const [re, setRe] = useState<ReStats | null>(null);
  const [cheminRe, setCheminRe] = useState<string | null>(null);
  const [index, setIndex] = useState<VfsIndexMeta | null>(null);
  const [anime, setAnime] = useState<{ saisons: number; episodes: number } | null>(null);
  /** Répartition des unités du découpage par état — écrite dans la base par `nie-forge kb`. */
  const [statutsForge, setStatutsForge] = useState<StatutForge[]>([]);
  const [packs, setPacks] = useState<number | null>(null);
  const [mods, setMods] = useState<number | null>(null);
  const [processusJeu, setProcessusJeu] = useState<{ pid: number; process_name: string } | null>(null);
  const [forge, setForge] = useState<{ pct: number | null; code: number | null } | null>(null);
  const [sauvegarde, setSauvegarde] = useState<string | null>(null);

  // Tout est chargé en parallèle et chaque source échoue SEULE : une base absente ne doit pas
  // vider le tableau de bord des trois autres — c'est précisément ce qu'il est censé montrer.
  useEffect(() => {
    let annule = false;
    const pose = <T,>(setter: (v: T) => void) => (v: T) => {
      if (!annule) setter(v);
    };

    api.stats(settings.gameDir).then(pose(setVfs)).catch((e) => !annule && setVfsErreur(String(e)));
    api.listPacksDir(settings.gameDir).then((p) => pose(setPacks)(p.length)).catch(() => pose(setPacks)(0));
    vfsIndexDb.meta().then(pose(setIndex)).catch(() => {});
    modsDb.listMods().then((m) => pose(setMods)(m.length)).catch(() => {});
    api.reTraceFindProcess().then(pose(setProcessusJeu)).catch(() => {});
    api.defaultSavePath().then(pose(setSauvegarde)).catch(() => {});
    api
      .forgeReport()
      .then((r) => pose(setForge)({ pct: r.produced_pct, code: r.code_pct }))
      .catch(() => {});

    const chemin = settings.wikiDb.trim();
    if (chemin) wikiDb.stats(chemin).then(pose(setMiroir)).catch(() => {});

    defaultAnimeDbPath(settings.gameDir)
      .then((p) => (p ? animeDb.stats(p).then(pose(setAnime)) : undefined))
      .catch(() => {});

    defaultReDbPath(settings.gameDir)
      .then(async (p) => {
        if (!p) return;
        pose(setCheminRe)(p);
        const [stats, statuts] = await Promise.all([reDb.stats(p), reDb.statutsForge(p)]);
        pose(setRe)(stats);
        pose(setStatutsForge)(statuts);
      })
      .catch(() => {});

    return () => {
      annule = true;
    };
  }, [settings.gameDir, settings.wikiDb]);

  /** Compte les entrées du VFS portant l'une de ces extensions (histogramme déjà calculé). */
  const parExt = useMemo(() => {
    const table = new Map<string, number>(vfs?.top_ext ?? []);
    return (...exts: string[]) => exts.reduce((somme, e) => somme + (table.get(e) ?? 0), 0);
  }, [vfs]);

  const vfsPret = !!vfs && vfs.total > 0;
  const miroirPret = !!miroir && (miroir.personnages ?? 0) > 0;
  const rePret = !!re && re.fonctions > 0;
  // L'index décrit-il TOUJOURS le montage courant ? Un total qui a bougé (jeu mis à jour, bascule
  // packs↔dump) rend l'index périmé sans le rendre faux : d'où « partiel », pas « absent ».
  const indexEtat: Etat = !index ? "absent" : vfs && index.total !== vfs.total ? "partiel" : "pret";

  return (
    <div className="h-full overflow-auto p-4">
      <div className="mx-auto max-w-6xl space-y-6">
        <div>
          <h1 className="text-xl font-semibold">Tableau de bord</h1>
          <p className="text-sm text-muted-foreground">
            L'état réel des quatre sources et de chaque onglet, relu à chaque ouverture de cette page.
          </p>
        </div>

        {/* ── Le socle : les quatre sources dont tout le reste dépend ─────────────────────── */}
        <section className="grid gap-3 sm:grid-cols-2 xl:grid-cols-5">
          <CarteSource
            titre="Jeu (VFS)"
            icone="folder_open"
            etat={vfsPret ? "pret" : "absent"}
            valeur={vfsPret ? nb(vfs?.total) : "—"}
            detail={
              vfsPret
                ? `fichiers · montage ${vfs?.montage === "dump" ? "dump extrait" : "packs CPK"} · ${nb(packs)} paquets`
                : (vfsErreur ?? "Jeu non détecté — indiquez son dossier dans les Paramètres")
            }
            sous={vfsPret ? [settings.gameDir || "(racine auto-détectée)"] : undefined}
            action={vfsPret ? undefined : { libelle: "Paramètres", onClick: () => onSelectTab("settings") }}
          />
          <CarteSource
            titre="Miroir du wiki"
            icone="database"
            etat={miroirPret ? "pret" : "absent"}
            valeur={miroirPret ? nb(miroir?.personnages) : "—"}
            detail={
              miroirPret
                ? `personnages · ${nb(miroir?.techniques)} techniques · ${nb(miroir?.objets)} objets · ${nb(miroir?.tables)} tables`
                : "Aucun miroir — livré avec l'application, ou à choisir dans les Paramètres"
            }
            sous={settings.wikiDb ? [settings.wikiDb] : undefined}
            action={miroirPret ? undefined : { libelle: "Paramètres", onClick: () => onSelectTab("settings") }}
          />
          <CarteSource
            titre="Base de reverse"
            icone="memory"
            etat={rePret ? "pret" : "absent"}
            valeur={rePret ? nb(re?.fonctions) : "—"}
            detail={
              rePret
                ? `fonctions · ${nb(re?.nommees)} nommées (${(((re?.nommees ?? 0) / (re?.fonctions || 1)) * 100).toFixed(1)} %) · ${nb(re?.racines)} racines .pdata`
                : "Aucune base RE trouvée (var/niers.sqlite, NIE_RE_DB, ou livrée avec l'application)"
            }
            sous={rePret && re?.sha256 ? [`${re.binaire} · ${re.sha256.slice(0, 12)}…`] : cheminRe ? [cheminRe] : undefined}
          />
          <CarteSource
            titre="Série (épisodes)"
            icone="movie"
            etat={anime && anime.episodes > 0 ? "pret" : "absent"}
            valeur={anime ? nb(anime.episodes) : "—"}
            detail={
              anime && anime.episodes > 0
                ? `épisodes · ${nb(anime.saisons)} saisons — vue Cinéma, à côté de Victory Road`
                : "Aucun catalogue d'épisodes (data/anime/episodes.db, NIE_ANIME_DB, ou livré avec l'application)"
            }
            action={
              anime && anime.episodes > 0
                ? { libelle: "Ouvrir le Cinéma", onClick: () => onSelectTab("cinema") }
                : undefined
            }
          />
          <CarteSource
            titre="Index du VFS"
            icone="table_chart"
            etat={indexEtat}
            valeur={index ? nb(index.total) : "—"}
            detail={
              index
                ? indexEtat === "partiel"
                  ? `entrées indexées, mais le VFS en compte ${nb(vfs?.total)} — index périmé`
                  : `entrées · réindexé le ${new Date(index.reindexed_at).toLocaleString("fr-FR")}`
                : "Index vide — la recherche par code retombe sur une correspondance approximative"
            }
            action={{ libelle: "Réindexer", onClick: () => onSelectTab("settings") }}
          />
        </section>

        {/* ── La forge : ce que le dépôt produit du binaire, à l'octet ────────────────────── */}
        {forge && (
          <section>
            <Card className="gap-2 py-4">
              <CardHeader className="px-4">
                <CardTitle className="flex items-center gap-2 text-sm font-medium">
                  <Icon name="construction" size={16} className="text-muted-foreground" />
                  Forge — part du binaire produite par le dépôt
                </CardTitle>
              </CardHeader>
              <CardContent className="px-4">
                <div className="flex flex-wrap items-baseline gap-x-6 gap-y-1">
                  <div>
                    <span className="text-2xl font-semibold tabular-nums">
                      {forge.pct === null ? "—" : `${forge.pct.toFixed(2)} %`}
                    </span>
                    <span className="ml-2 text-xs text-muted-foreground">du fichier</span>
                  </div>
                  <div>
                    <span className="text-2xl font-semibold tabular-nums">
                      {forge.code === null ? "—" : `${forge.code.toFixed(2)} %`}
                    </span>
                    <span className="ml-2 text-xs text-muted-foreground">du .text</span>
                  </div>
                  {/* Ce que la forge a inscrit dans la base : les deux nombres qui disent où en
                      est le travail, là où les pourcentages disent seulement à quelle hauteur. */}
                  {statutsForge.length > 0 && (
                    <div className="flex items-baseline gap-4 text-xs text-muted-foreground">
                      {statutsForge
                        .filter((s) => s.statut === "produit" || s.statut === "bloque")
                        .map((s) => (
                          <span key={s.statut}>
                            <span
                              className={
                                s.statut === "produit"
                                  ? "font-semibold tabular-nums text-emerald-500"
                                  : "font-semibold tabular-nums text-amber-500"
                              }
                            >
                              {nb(s.unites)}
                            </span>{" "}
                            {s.statut === "produit" ? "unités produites" : "bloquées"} ({nb(s.octets)} o)
                          </span>
                        ))}
                    </div>
                  )}
                  <Button size="sm" variant="outline" className="h-7 text-xs" onClick={() => onSelectTab("re")}>
                    Ouvrir la forge
                  </Button>
                </div>
              </CardContent>
            </Card>
          </section>
        )}

        {/* ── Aperçu de chaque onglet ──────────────────────────────────────────────────────── */}
        <section className="space-y-2">
          <h2 className="text-sm font-medium text-muted-foreground">Aperçu des onglets</h2>
          <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
            <CarteOnglet
              id="editor"
              etat={vfsPret ? "pret" : "absent"}
              metrique={vfsPret ? nb(parExt("g4md", "g4mg", "g4sk")) : "—"}
              note={vfsPret ? "Modèles, squelettes et animations du montage courant." : undefined}
              onClick={() => onSelectTab("editor")}
            />
            <CarteOnglet
              id="explorer"
              etat={vfsPret ? "pret" : "absent"}
              metrique={vfsPret ? nb(vfs?.total) : "—"}
              note={vfsPret ? `Arborescence complète du montage ${vfs?.montage}.` : "Nécessite le dossier du jeu."}
              onClick={() => onSelectTab("explorer")}
            />
            <CarteOnglet
              id="search"
              etat={indexEtat}
              metrique={index ? nb(index.total) : "—"}
              note={indexEtat === "pret" ? "Correspondance exacte par code interne." : "Index absent ou périmé : correspondance approximative."}
              onClick={() => onSelectTab("search")}
            />
            <CarteOnglet
              id="cinema"
              etat={vfsPret && parExt("usm") > 0 ? "pret" : "absent"}
              metrique={vfsPret ? nb(parExt("usm")) : "—"}
              onClick={() => onSelectTab("cinema")}
            />
            <CarteOnglet
              id="data"
              etat={vfsPret ? "pret" : miroirPret ? "partiel" : "absent"}
              metrique={miroirPret ? nb(miroir?.techniques) : "—"}
              note={vfsPret ? undefined : "Sans le jeu, seuls les chiffres du miroir sont disponibles."}
              onClick={() => onSelectTab("data")}
            />
            <CarteOnglet
              id="gallery"
              etat={vfsPret ? "pret" : "absent"}
              metrique={vfsPret ? nb(parExt("g4tx", "png")) : "—"}
              onClick={() => onSelectTab("gallery")}
            />
            <CarteOnglet
              id="cpk"
              etat={(packs ?? 0) > 0 ? "pret" : "absent"}
              metrique={nb(packs)}
              note={(packs ?? 0) > 0 ? undefined : "Aucun paquet — le montage dump n'en utilise pas."}
              onClick={() => onSelectTab("cpk")}
            />
            <CarteOnglet
              id="save"
              etat={sauvegarde ? "pret" : "absent"}
              metrique={sauvegarde ? "détectée" : "—"}
              note={sauvegarde ?? "Aucune sauvegarde Steam Cloud trouvée sur cette machine."}
              onClick={() => onSelectTab("save")}
            />
            <CarteOnglet
              id="tools"
              etat={miroirPret ? "pret" : "absent"}
              metrique={miroirPret ? nb(miroir?.equipes) : "—"}
              note={miroirPret ? `${nb(miroir?.equipes)} équipes dans le miroir.` : "Demande le miroir du wiki."}
              onClick={() => onSelectTab("tools")}
            />
            <CarteOnglet
              id="mods"
              etat={vfsPret ? "pret" : "absent"}
              metrique={nb(mods)}
              note={(mods ?? 0) > 0 ? `${nb(mods)} mod(s) au registre local.` : undefined}
              onClick={() => onSelectTab("mods")}
            />
            <CarteOnglet
              id="re"
              etat={rePret ? "pret" : "absent"}
              metrique={rePret ? nb(re?.classes) : "—"}
              note={rePret ? `${nb(re?.classes)} classes RTTI indexées.` : undefined}
              onClick={() => onSelectTab("re")}
            />
            <CarteOnglet
              id="viola"
              etat={vfs?.montage === "packs" ? "pret" : "absent"}
              metrique={vfs?.montage === "packs" ? nb(packs) : "—"}
              note={vfs?.montage === "packs" ? undefined : "Demande une installation en paquets CPK, pas un dump."}
              onClick={() => onSelectTab("viola")}
            />
            <CarteOnglet
              id="livemod"
              etat={processusJeu ? "pret" : "absent"}
              metrique={processusJeu ? `PID ${processusJeu.pid}` : "—"}
              note={
                processusJeu
                  ? `${processusJeu.process_name} tourne : lecture de sa mémoire vivante.`
                  : "Le jeu n'est pas lancé."
              }
              onClick={() => onSelectTab("livemod")}
            />
            <CarteOnglet
              id="lua"
              etat={vfsPret ? "pret" : "absent"}
              metrique={vfsPret ? nb(parExt("lua", "luac")) : "—"}
              onClick={() => onSelectTab("lua")}
            />
          </div>
        </section>
      </div>
    </div>
  );
}
