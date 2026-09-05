// Le bandeau de tête — un titre mis en avant, qui tourne.
//
// ## Ce qu'il coûte, et pourquoi il ne coûte rien
//
// Le bandeau des plateformes de référence joue une bande-annonce. Ici, jouer un fond signifierait
// démultiplexer un conteneur `.usm` — jusqu'à 300 Mo — pour une image que personne ne regarde
// plus après trois secondes, et cela dès l'ouverture de la vue. Le carrousel ne lit donc AUCUN
// octet du jeu : il affiche la vignette distante d'un épisode, ou l'affiche d'une cinématique
// **déjà capturée** au survol d'une carte (cache `affiches`). Un film jamais survolé garde son
// fond typographique — c'est honnête, et cela reste cohérent avec la règle qui gouverne toute la
// vue : le catalogue s'ouvre sans ouvrir les conteneurs.
//
// ## La rotation
//
// Neuf secondes par titre, suspendue au survol et à la première interaction avec les pastilles —
// un carrousel qui change de titre pendant qu'on vise son bouton est une porte qui se déplace.
// `prefers-reduced-motion` la désactive entièrement : le premier titre reste, les pastilles
// fonctionnent toujours.
import { useEffect, useRef, useState } from "react";

import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/Icon";
import { formaterDuree } from "@/components/VideoPlayer";
import { afficheConnue } from "@/lib/affiches";
import { formaterOctets, vignetteDe, type ElementCinema } from "@/lib/cinema";
import { cn } from "@/lib/utils";

/** Durée d'affichage d'un titre, en millisecondes. */
const ROTATION = 9000;

export function HerosCarrousel({
  elements,
  liste,
  technique,
  onLire,
  onOuvrir,
  onBasculerListe,
}: {
  elements: ElementCinema[];
  liste: ReadonlySet<string>;
  technique: boolean;
  onLire: (el: ElementCinema) => void;
  onOuvrir: (el: ElementCinema) => void;
  onBasculerListe: (cle: string) => void;
}) {
  const [rang, setRang] = useState(0);
  const [fige, setFige] = useState(false);
  const minuterie = useRef<number | null>(null);

  // Le catalogue arrive en deux temps (jeu puis série) : un rang calculé sur une liste courte
  // pointerait hors de la liste longue une seconde plus tard.
  const index = elements.length === 0 ? 0 : rang % elements.length;
  const courant = elements[index];

  useEffect(() => {
    if (fige || elements.length < 2) return;
    if (window.matchMedia?.("(prefers-reduced-motion: reduce)").matches) return;
    minuterie.current = window.setTimeout(() => setRang((r) => r + 1), ROTATION);
    return () => {
      if (minuterie.current) window.clearTimeout(minuterie.current);
    };
  }, [rang, fige, elements.length]);

  if (!courant) return null;

  const dansListe = liste.has(courant.cle);
  const episode = courant.episode;
  const film = courant.film;
  const fond = episode
    ? vignetteDe(episode)
    : film
      ? afficheConnue(film.chemin)
      : null;

  const badges = [
    episode?.publie ? new Date(episode.publie).getFullYear().toString() : null,
    episode?.episode ? `Épisode ${episode.episode}` : (film?.rubrique ?? null),
    film?.duree != null ? formaterDuree(film.duree) : null,
    film?.largeur ? `${film.largeur}×${film.hauteur}` : null,
    technique && film ? formaterOctets(film.octets) : null,
    courant.source === "jeu" ? "Cinématique du jeu" : null,
  ].filter((x): x is string => Boolean(x));

  return (
    <div
      className="relative mx-4 mt-3 overflow-hidden rounded-xl border border-app-line bg-app-dark-box"
      onMouseEnter={() => setFige(true)}
      onMouseLeave={() => setFige(false)}
    >
      <div className="relative aspect-[21/8] max-h-[46vh] w-full">
        {fond ? (
          <img
            key={courant.cle}
            src={fond}
            alt=""
            className="absolute inset-0 h-full w-full object-cover animate-in fade-in duration-500"
            draggable={false}
          />
        ) : (
          <div className="absolute inset-0 bg-gradient-to-br from-app-box to-app-dark-box" />
        )}
        <div className="pointer-events-none absolute inset-0 bg-gradient-to-r from-app via-app/70 to-transparent" />
        <div className="pointer-events-none absolute inset-x-0 bottom-0 h-24 bg-gradient-to-t from-app to-transparent" />

        <div className="absolute inset-y-0 left-0 flex max-w-xl flex-col justify-center gap-3 p-6">
          <div className="text-tiny uppercase tracking-[0.2em] text-accent">
            {courant.source === "jeu" ? "Victory Road" : (courant.sousTitre ?? "Série")}
          </div>
          <h2 className="line-clamp-2 text-3xl font-semibold leading-tight text-ink drop-shadow">
            {courant.titre}
          </h2>
          {episode?.description && (
            <p className="line-clamp-2 max-w-lg text-sm text-ink-dull">{episode.description}</p>
          )}
          <div className="flex flex-wrap items-center gap-1.5 text-tiny text-ink-dull">
            {badges.map((b) => (
              <span key={b} className="rounded bg-app-line/70 px-1.5 py-0.5">
                {b}
              </span>
            ))}
          </div>
          <div className="mt-1 flex items-center gap-2">
            <Button onClick={() => onLire(courant)}>
              <Icon name="play_arrow" size={16} />
              Lecture
            </Button>
            <button
              type="button"
              onClick={() => onBasculerListe(courant.cle)}
              title={dansListe ? "Retirer de ma liste" : "Ajouter à ma liste"}
              aria-label={dansListe ? "Retirer de ma liste" : "Ajouter à ma liste"}
              aria-pressed={dansListe}
              className={cn(
                "flex size-9 items-center justify-center rounded-full border transition-colors",
                dansListe
                  ? "border-accent bg-accent/15 text-accent"
                  : "border-ink/25 bg-app/60 text-ink-dull hover:border-ink/60 hover:text-ink",
              )}
            >
              <Icon name={dansListe ? "check" : "add"} size={18} />
            </button>
            <Button variant="outline" onClick={() => onOuvrir(courant)}>
              Plus d'infos
            </Button>
          </div>
        </div>
      </div>

      {elements.length > 1 && (
        <div className="absolute inset-x-0 bottom-2 flex items-center justify-center gap-1.5">
          {elements.map((el, i) => (
            <button
              key={el.cle}
              type="button"
              aria-label={`Titre ${i + 1} : ${el.titre}`}
              aria-current={i === index}
              onClick={() => {
                setRang(i);
                setFige(true);
              }}
              className={cn(
                "h-1.5 rounded-full transition-all",
                i === index ? "w-5 bg-ink" : "w-1.5 bg-ink/35 hover:bg-ink/60",
              )}
            />
          ))}
        </div>
      )}
    </div>
  );
}
