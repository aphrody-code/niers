// La fiche d'un titre — ce qui s'ouvre au clic sur une carte, avant de lancer quoi que ce soit.
//
// ## Pourquoi une fiche, alors qu'un clic lançait la lecture
//
// Le catalogue mélange des cinématiques de six secondes et des épisodes de vingt-deux minutes.
// Lancer directement, c'était répondre à la question « lequel est-ce ? » en ouvrant le fichier —
// donc en démultiplexant jusqu'à 300 Mo pour s'apercevoir qu'on s'est trompé. La fiche répond
// avant : elle montre l'aperçu, la durée, le résumé, la position de reprise et les épisodes
// voisins. C'est le geste de Netflix et de Disney+, et il vaut ici pour la même raison.
//
// La lecture reste à un clic : le bouton ▶ des cartes court-circuite la fiche.
//
// ## Ce qui n'est pas dupliqué
//
// L'aperçu vidéo emploie le même `urlVideo` et la même capture d'affiche que les cartes, par le
// cache partagé `affiches` (`lib/cinema.ts`) : ouvrir la fiche d'un film déjà survolé ne redécode
// rien. Le sélecteur de saison est le `Select` du design system, comme partout ailleurs.
import { useEffect, useMemo, useRef, useState } from "react";

import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";
import { Icon } from "@/components/ui/Icon";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { formaterDuree, urlVideo } from "@/components/VideoPlayer";
import { afficheConnue, poserAffiche } from "@/lib/affiches";
import {
  formaterOctets,
  INSTANT_AFFICHE,
  vignetteDe,
  type ElementCinema,
  type Reprises,
  type SaisonCinema,
} from "@/lib/cinema";
import { empreinte } from "@/lib/serie";
import type { SourceLecture } from "@/lib/sources";
import { cn } from "@/lib/utils";

export interface FicheDetailProps {
  element: ElementCinema;
  /** Les autres titres de la saison affichée — la liste d'épisodes de la fiche. */
  fratrie: ElementCinema[];
  /** Toutes les façons de regarder ce titre : langues, montages, chaînes (`lib/sources.ts`). */
  sources: SourceLecture[];
  /** Saisons proposées par le sélecteur. Vide pour un titre du jeu. */
  saisons: SaisonCinema[];
  /** Clé de la saison dont la fratrie est affichée. */
  saisonAffichee: string;
  vus: ReadonlySet<string>;
  liste: ReadonlySet<string>;
  reprises: Reprises;
  /** Fiche technique visible — masquée pour un profil jeunesse. */
  technique: boolean;
  onLire: (el: ElementCinema, source?: SourceLecture) => void;
  onBasculerVu: (el: ElementCinema) => void;
  onBasculerListe: (cle: string) => void;
  onChoisirSaison: (cle: string) => void;
  onChoisirElement: (el: ElementCinema) => void;
  onPrecharger?: (chemin: string) => void;
  onReveler?: (chemin: string) => void;
  onFermer: () => void;
}

export function FicheDetail({
  element,
  fratrie,
  sources,
  saisons,
  saisonAffichee,
  vus,
  liste,
  reprises,
  technique,
  onLire,
  onBasculerVu,
  onBasculerListe,
  onChoisirSaison,
  onChoisirElement,
  onPrecharger,
  onReveler,
  onFermer,
}: FicheDetailProps) {
  const film = element.film;
  const episode = element.episode;
  const reprise = reprises[element.cle];
  const vu = episode?.episode != null && vus.has(empreinte(episode.saison, episode.episode));
  const dansListe = liste.has(element.cle);

  /**
   * La source retenue, par son identifiant.
   *
   * `null` signifie « celle que le catalogue propose par défaut », pas « aucune » : tant que
   * personne n'a choisi, la fiche suit la langue de la barre et la meilleure définition
   * disponible (cf. le tri de `sourcesDe`). Le choix se réinitialise en changeant de titre — le
   * garder ferait lire l'épisode suivant dans une langue qu'on n'a pas demandée pour lui.
   */
  const [choisie, setChoisie] = useState<string | null>(null);
  useEffect(() => setChoisie(null), [element.cle]);
  const source = sources.find((s) => s.id === choisie) ?? sources.find((s) => s.defaut) ?? null;

  // Le préchargement part dès l'ouverture de la fiche : à ce stade l'intention n'a plus rien
  // d'ambigu — on ne survole pas une fiche par accident. C'est le seul endroit de la vue où on
  // prépare des octets sans attendre un délai.
  useEffect(() => {
    if (film && film.lisible !== false) onPrecharger?.(film.chemin);
  }, [film, onPrecharger]);

  const progression = reprise && reprise.duree > 0 ? (reprise.position / reprise.duree) * 100 : 0;

  const meta = useMemo(() => {
    if (episode) {
      return [
        episode.publie ? new Date(episode.publie).getFullYear().toString() : null,
        episode.episode ? `Épisode ${episode.episode}` : null,
        episode.langue ? episode.langue.toUpperCase() : null,
        episode.publie ? new Date(episode.publie).toLocaleDateString("fr-FR") : null,
      ].filter((x): x is string => Boolean(x));
    }
    if (film) {
      return [
        film.rubrique,
        film.duree != null ? formaterDuree(film.duree) : null,
        film.largeur ? `${film.largeur}×${film.hauteur}` : null,
        film.codec?.toUpperCase() ?? null,
        technique ? formaterOctets(film.octets) : null,
      ].filter((x): x is string => Boolean(x));
    }
    return [];
  }, [episode, film, technique]);

  const nomSaison = saisons.find((s) => s.cle === saisonAffichee)?.titre ?? "";
  const avecEpisodes = fratrie.length > 1;

  return (
    <Dialog open onOpenChange={(ouvert) => !ouvert && onFermer()}>
      <DialogContent
        showCloseButton={false}
        className="max-h-[90vh] w-[min(56rem,calc(100vw-3rem))] max-w-[min(56rem,calc(100vw-3rem))] gap-0 overflow-y-auto bg-app p-0 sm:max-w-[min(56rem,calc(100vw-3rem))]"
      >
        {/* ── L'en-tête, qui joue ─────────────────────────────────────────── */}
        <div className="relative aspect-video w-full shrink-0 overflow-hidden bg-app-dark-box">
          <ApercuTitre element={element} />
          {/* Deux dégradés superposés : l'un éteint le bas pour porter le titre, l'autre la
              gauche pour que le texte reste lisible sur une image claire. */}
          <div className="pointer-events-none absolute inset-0 bg-gradient-to-t from-app via-app/55 to-transparent" />
          <div className="pointer-events-none absolute inset-0 bg-gradient-to-r from-app/85 via-transparent to-transparent" />

          <button
            type="button"
            onClick={onFermer}
            aria-label="Fermer"
            title="Fermer"
            className="absolute right-3 top-3 flex size-9 items-center justify-center rounded-full bg-black/60 text-white transition-colors hover:bg-black/80"
          >
            <Icon name="close" size={18} />
          </button>

          <div className="absolute inset-x-0 bottom-0 p-5">
            <DialogTitle className="max-w-2xl text-2xl font-semibold leading-tight text-ink drop-shadow-sm">
              {element.titre}
            </DialogTitle>
            {(episode?.titreJp || film?.nom_origine) && (
              <div className="mt-1 truncate text-xs text-ink-dull">
                {episode?.titreJp ?? <span className="font-mono">{film?.nom_origine}</span>}
                {episode?.romaji ? <span className="ml-2 italic opacity-70">{episode.romaji}</span> : null}
              </div>
            )}

            {progression > 0 && (
              <div className="mt-3 flex max-w-md items-center gap-2">
                <div className="h-1 flex-1 overflow-hidden rounded-full bg-ink/20">
                  <div className="h-full bg-accent" style={{ width: `${Math.min(100, progression)}%` }} />
                </div>
                <span className="shrink-0 text-tiny text-ink-dull">
                  {Math.round((reprise?.position ?? 0) / 60)} sur {Math.round((reprise?.duree ?? 0) / 60)} min
                </span>
              </div>
            )}

            <div className="mt-3 flex flex-wrap items-center gap-2">
              <Button onClick={() => onLire(element, source ?? undefined)}>
                <Icon name="play_arrow" size={16} />
                {progression > 0 ? "Reprendre" : "Lecture"}
              </Button>

              {/* Le choix de source, et seulement quand il y en a un.
                  Mesuré sur ce corpus : 4 titres du jeu existent en 6 à 9 langues, et les 97
                  films ont chacun deux montages (`common` et `dx11`). Les 93 autres n'ont qu'une
                  version — leur afficher un sélecteur à une entrée serait un choix pour rien. */}
              {sources.length > 1 && (
                <Select value={source?.id ?? ""} onValueChange={(v) => v && setChoisie(v)}>
                  <SelectTrigger size="sm" className="h-9 w-64 text-xs" aria-label="Choisir la source">
                    <SelectValue placeholder="Source" />
                  </SelectTrigger>
                  <SelectContent>
                    {sources.map((s) => (
                      <SelectItem key={s.id} value={s.id} disabled={!s.lisible}>
                        {s.libelle}
                        {s.detail ? <span className="ml-2 text-ink-faint">{s.detail}</span> : null}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              )}
              <BoutonRond
                actif={dansListe}
                titre={dansListe ? "Retirer de ma liste" : "Ajouter à ma liste"}
                icone={dansListe ? "check" : "add"}
                onClick={() => onBasculerListe(element.cle)}
              />
              {episode?.episode != null && (
                <BoutonRond
                  actif={vu}
                  titre={vu ? "Marquer comme non vu" : "Marquer comme vu"}
                  icone={vu ? "check_circle" : "radio_button_unchecked"}
                  onClick={() => onBasculerVu(element)}
                />
              )}
              {film && onReveler && (
                <BoutonRond
                  titre="Voir le fichier dans l'Explorateur"
                  icone="folder_open"
                  onClick={() => onReveler(film.chemin)}
                />
              )}
            </div>
          </div>
        </div>

        {/* ── Le corps ────────────────────────────────────────────────────── */}
        <div className="px-5 pb-5 pt-4">
          <div className="flex flex-wrap items-center gap-2 text-xs text-ink-dull">
            {meta.map((m) => (
              <span key={m} className="rounded bg-app-line/60 px-1.5 py-0.5">
                {m}
              </span>
            ))}
            {film?.lisible === false && (
              <span className="rounded bg-status-warning/20 px-1.5 py-0.5 text-status-warning">
                non lisible dans cette fenêtre
              </span>
            )}
          </div>

          {episode?.description && (
            <p className="mt-3 max-w-3xl text-sm leading-relaxed text-ink">{episode.description}</p>
          )}

          {/* `key` sur la présence de la liste : sans lui, passer d'un titre à épisodes à un
              titre isolé garderait l'onglet « Épisodes » sélectionné alors qu'il a disparu. */}
          <Tabs
            key={avecEpisodes ? "avec-episodes" : "seul"}
            defaultValue={avecEpisodes ? "episodes" : "details"}
            className="mt-5"
          >
            <TabsList variant="line" className="h-8">
              {avecEpisodes && <TabsTrigger value="episodes">Épisodes</TabsTrigger>}
              <TabsTrigger value="details">Détails</TabsTrigger>
            </TabsList>

            {avecEpisodes && (
              <TabsContent value="episodes" className="pt-3">
                {saisons.length > 1 && (
                  <div className="mb-3 flex items-center gap-2">
                    <Select value={saisonAffichee} onValueChange={(v) => v && onChoisirSaison(v)}>
                      <SelectTrigger size="sm" className="h-7 w-56 text-xs" aria-label="Choisir la saison">
                        <SelectValue placeholder={nomSaison} />
                      </SelectTrigger>
                      <SelectContent>
                        {saisons.map((s) => (
                          <SelectItem key={s.cle} value={s.cle}>
                            {s.titre}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                    <span className="text-tiny text-ink-faint">
                      {fratrie.length} titre{fratrie.length > 1 ? "s" : ""}
                    </span>
                  </div>
                )}

                <div className="flex flex-col divide-y divide-app-line">
                  {fratrie.map((el, i) => (
                    <LigneEpisode
                      key={el.cle}
                      element={el}
                      rang={el.episode?.episode ?? i + 1}
                      courant={el.cle === element.cle}
                      vu={
                        el.episode?.episode != null &&
                        vus.has(empreinte(el.episode.saison, el.episode.episode))
                      }
                      reprise={reprises[el.cle]}
                      onOuvrir={() => onChoisirElement(el)}
                      onLire={() => onLire(el)}
                    />
                  ))}
                </div>
              </TabsContent>
            )}

            <TabsContent value="details" className="pt-3">
              <FicheTechnique element={element} technique={technique} />
            </TabsContent>
          </Tabs>
        </div>
      </DialogContent>
    </Dialog>
  );
}

// ── L'aperçu de l'en-tête ─────────────────────────────────────────────────────

/**
 * L'image de tête : une vidéo qui joue pour une cinématique, une vignette haute définition pour
 * un épisode.
 *
 * Un épisode n'a PAS d'aperçu jouable : sa vidéo est hébergée par la chaîne officielle, et
 * l'intégrer en boucle silencieuse dans un en-tête consommerait une lecture comptabilisée pour
 * une image de décor. La vignette suffit — c'est d'ailleurs ce que la plateforme sert elle-même.
 */
function ApercuTitre({ element }: { element: ElementCinema }) {
  const film = element.film;
  const videoRef = useRef<HTMLVideoElement | null>(null);
  const [affiche, setAffiche] = useState<string | null>(() =>
    film ? afficheConnue(film.chemin) : null,
  );
  const [imageKo, setImageKo] = useState(false);

  const capturer = () => {
    const v = videoRef.current;
    if (!v || !film || afficheConnue(film.chemin) || v.videoWidth === 0) return;
    const canvas = document.createElement("canvas");
    canvas.width = 640;
    canvas.height = Math.round((640 * v.videoHeight) / v.videoWidth);
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.drawImage(v, 0, 0, canvas.width, canvas.height);
    try {
      const url = canvas.toDataURL("image/jpeg", 0.72);
      poserAffiche(film.chemin, url);
      setAffiche(url);
    } catch {
      // Canvas teinté : l'en-tête garde son fond typographique.
    }
  };

  if (element.episode) {
    const source = imageKo ? element.vignette : vignetteDe(element.episode);
    if (!source) return <FondTypographique titre={element.titre} />;
    return (
      <img
        src={source}
        alt=""
        onError={() => setImageKo(true)}
        className="h-full w-full object-cover"
        draggable={false}
      />
    );
  }

  if (!film) return <FondTypographique titre={element.titre} />;

  // Un conteneur que la webview ne sait pas décoder ne doit même pas être demandé : la balise
  // échouerait en silence et l'en-tête resterait noir, sans rien dire.
  if (film.lisible === false) {
    return affiche ? (
      <img src={affiche} alt="" className="h-full w-full object-cover" draggable={false} />
    ) : (
      <FondTypographique titre={film.nom} />
    );
  }

  return (
    <>
      {affiche && (
        <img src={affiche} alt="" className="absolute inset-0 h-full w-full object-cover" draggable={false} />
      )}
      {/* eslint-disable-next-line jsx-a11y/media-has-caption -- aperçu muet, sans dialogue. */}
      <video
        ref={videoRef}
        src={urlVideo(film.chemin)}
        crossOrigin="anonymous"
        muted
        autoPlay
        loop
        playsInline
        className="relative h-full w-full object-cover"
        onLoadedMetadata={(e) => {
          const v = e.currentTarget;
          if (v.duration > 2) v.currentTime = v.duration * INSTANT_AFFICHE;
        }}
        onSeeked={capturer}
      />
    </>
  );
}

function FondTypographique({ titre }: { titre: string }) {
  return (
    <div className="flex h-full w-full flex-col items-center justify-center gap-2 bg-gradient-to-br from-app-box to-app-dark-box">
      <Icon name="movie" size={48} className="text-ink-faint/40" />
      <span className="px-6 text-center font-mono text-xs text-ink-faint">{titre}</span>
    </div>
  );
}

// ── Une ligne de la liste d'épisodes ──────────────────────────────────────────

function LigneEpisode({
  element,
  rang,
  courant,
  vu,
  reprise,
  onOuvrir,
  onLire,
}: {
  element: ElementCinema;
  rang: number;
  courant: boolean;
  vu: boolean;
  reprise?: { position: number; duree: number };
  onOuvrir: () => void;
  onLire: () => void;
}) {
  const [imageKo, setImageKo] = useState(false);
  const film = element.film;
  const vignette = element.episode
    ? imageKo
      ? element.vignette
      : vignetteDe(element.episode)
    : film
      ? afficheConnue(film.chemin)
      : null;
  const progression = reprise && reprise.duree > 0 ? (reprise.position / reprise.duree) * 100 : 0;

  return (
    <div
      className={cn(
        "group/ligne flex cursor-pointer items-start gap-3 rounded-md px-2 py-2.5 transition-colors hover:bg-app-hover",
        courant && "bg-app-box",
      )}
      role="button"
      tabIndex={0}
      onClick={onOuvrir}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onOuvrir();
        }
      }}
    >
      <span className="w-5 shrink-0 pt-6 text-center text-sm tabular-nums text-ink-faint">{rang}</span>
      <div className="relative aspect-video w-36 shrink-0 overflow-hidden rounded bg-app-dark-box">
        {vignette ? (
          <img
            src={vignette}
            alt=""
            loading="lazy"
            onError={() => setImageKo(true)}
            className="h-full w-full object-cover"
          />
        ) : (
          <div className="flex h-full items-center justify-center text-ink-faint">
            <Icon name="movie" size={20} />
          </div>
        )}
        <button
          type="button"
          aria-label={`Lire ${element.titre}`}
          title="Lire"
          onClick={(e) => {
            e.stopPropagation();
            onLire();
          }}
          className="absolute inset-0 flex items-center justify-center bg-black/40 opacity-0 transition-opacity group-hover/ligne:opacity-100"
        >
          <Icon name="play_circle" size={30} className="text-white" />
        </button>
        {progression > 0 && (
          <div className="absolute inset-x-0 bottom-0 h-0.5 bg-white/25">
            <div className="h-full bg-accent" style={{ width: `${Math.min(100, progression)}%` }} />
          </div>
        )}
      </div>
      <div className="min-w-0 flex-1">
        <div className="flex items-baseline gap-2">
          <span className="truncate text-sm font-medium text-ink">{element.titre}</span>
          {vu && <Icon name="check_circle" size={14} className="shrink-0 text-emerald-400" />}
          <div className="flex-1" />
          {film?.duree != null && (
            <span className="shrink-0 font-mono text-tiny text-ink-faint">{formaterDuree(film.duree)}</span>
          )}
        </div>
        {element.episode?.description ? (
          <p className="mt-0.5 line-clamp-2 text-xs leading-relaxed text-ink-dull">
            {element.episode.description}
          </p>
        ) : (
          element.sousTitre && <p className="mt-0.5 text-xs text-ink-faint">{element.sousTitre}</p>
        )}
      </div>
    </div>
  );
}

// ── L'onglet « Détails » ──────────────────────────────────────────────────────

function FicheTechnique({ element, technique }: { element: ElementCinema; technique: boolean }) {
  const film = element.film;
  const episode = element.episode;

  const lignes: [string, string][] = [];
  if (episode) {
    lignes.push(["Saison", String(episode.saison)]);
    if (episode.episode !== null) lignes.push(["Épisode", String(episode.episode)]);
    if (episode.publie) lignes.push(["Diffusion", new Date(episode.publie).toLocaleDateString("fr-FR")]);
    if (episode.titreJp) lignes.push(["Titre original", episode.titreJp]);
    if (episode.romaji) lignes.push(["Transcription", episode.romaji]);
    if (episode.langue) lignes.push(["Langue", episode.langue]);
    if (technique) lignes.push(["Source", `YouTube · ${episode.videoId}`]);
  }
  if (film) {
    lignes.push(["Rubrique", film.rubrique]);
    if (film.duree != null) lignes.push(["Durée", formaterDuree(film.duree)]);
    if (film.largeur) lignes.push(["Définition", `${film.largeur}×${film.hauteur}`]);
    if (film.cadence) lignes.push(["Cadence", `${film.cadence.toFixed(3)} i/s`]);
    if (film.codec) lignes.push(["Codec", film.codec.toUpperCase()]);
    // La bande-son d'un film n'est presque jamais dans son conteneur : elle vient de la banque
    // `anime_stream`, et `source` dit laquelle des deux voies a répondu.
    const piste = film.audio[0];
    lignes.push([
      "Bande-son",
      piste
        ? `${piste.codec.toUpperCase()}${piste.frequence ? ` · ${Math.round(piste.frequence / 1000)} kHz` : ""}${
            piste.source !== "conteneur" ? ` · cue ${piste.source}` : ""
          }`
        : "aucune",
    ]);
    if (technique) {
      lignes.push(["Taille", formaterOctets(film.octets)]);
      if (film.nom_origine) lignes.push(["Nom d'origine", film.nom_origine]);
      if (film.chiffre !== null) lignes.push(["Enveloppe CRI", film.chiffre ? "XOR" : "aucune"]);
      if (film.bgm) lignes.push(["BGM déclarée", film.bgm]);
      if (film.sous_titres) lignes.push(["Sous-titres", film.sous_titres]);
      lignes.push(["Chemin VFS", film.chemin]);
    }
  }

  return (
    <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1.5 text-xs">
      {lignes.map(([cle, valeur]) => (
        <div key={cle} className="contents">
          <dt className="text-ink-faint">{cle}</dt>
          <dd className="min-w-0 break-all text-ink">{valeur}</dd>
        </div>
      ))}
    </dl>
  );
}

// ── Bouton rond de l'en-tête ──────────────────────────────────────────────────

function BoutonRond({
  icone,
  titre,
  actif,
  onClick,
}: {
  icone: string;
  titre: string;
  actif?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      title={titre}
      aria-label={titre}
      aria-pressed={actif}
      className={cn(
        "flex size-9 items-center justify-center rounded-full border transition-colors",
        actif
          ? "border-accent bg-accent/15 text-accent"
          : "border-ink/25 bg-app/60 text-ink-dull hover:border-ink/60 hover:text-ink",
      )}
    >
      <Icon name={icone} size={18} />
    </button>
  );
}
