// Vue **Galerie** — les illustrations du jeu, listées depuis le VFS.
//
// Portée depuis `apps/azalee/app/gallery/` (`GalleryGrid`, `GalleryLightbox`,
// `filters/GalleryFilterBar`, `wikiService.getGalleryList`). La migration ne déplace pas la page :
// elle change de source. Le wiki compose deux fonds qui ne se rejoignent jamais — la table
// `inagle_gallery` (360 lignes) et un manifeste statique de 3 579 entrées — d'où sa pastille
// « Toutes » qui annonce 3 939 items pour une liste qui n'en rend que 360.
//
// Ici, la source est le VFS monté : `data/dx11/menu/220_img/` porte **17 085 `.g4tx`**, les
// catégories SONT les sous-dossiers réels (`api.ls`), et chaque compte affiché est un compte
// relevé (`api.findPaged` rend le total avant pagination, cf. `nie_explore::listing::find_paged`).
// `gallery_config` (`api.gameDataGallery`) n'énumère plus : elle enrichit — condition de
// déblocage et épisode d'histoire, là où elle en connaît.
//
// Les vignettes passent par `lib/thumbs.ts`, source UNIQUE de l'application : décodage borné à
// 128 px côté Rust, cache LRU, file de décodage. Une grille qui appellerait `api.texturePngB64`
// ferait entrer 8 Mo de bitmap par image dans le processus de rendu (les `gallery_img2` pèsent
// 8 294 752 octets pièce) — c'est exactement l'accident que `thumbs.ts` documente.
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { toast } from "sonner";

import { api, type VfsDir } from "@/lib/api";
import { humanSize } from "@/lib/bytes";
import {
  EXT_GALERIE,
  LANGUES,
  RACINE_GALERIE,
  construireIllustrations,
  filtrerIllustrations,
  libelleCategorie,
  libelleSousDossier,
  prefixeCategorie,
  type EnrichissementGalerie,
  type Illustration,
} from "@/lib/galerie";
import { useSettings } from "@/lib/settings";
import { useThumbnail } from "@/lib/thumbs";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Icon } from "@/components/ui/Icon";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";

/** Illustrations affichées d'un coup — au-delà, un bouton « en afficher plus ». */
const PAR_PAGE = 60;

/**
 * Images PLEINE RÉSOLUTION gardées par la visionneuse.
 *
 * Volontairement minuscule : une planche du jeu pèse plusieurs mégaoctets une fois encodée en
 * base64, et la garder est autrement plus cher qu'une vignette de 128 px. Trois entrées
 * couvrent exactement ce que sert le préchargement — l'image courante et ses deux voisines —
 * sans retenir un album entier.
 */
const MAX_PLEINES = 3;

/** Cache LRU des images pleine résolution : `Map` = ordre d'insertion. */
const cachePleines = new Map<string, string>();

/** Lit le cache en marquant l'entrée comme récemment utilisée. */
function pleineDuCache(chemin: string): string | undefined {
	const trouve = cachePleines.get(chemin);
	if (trouve === undefined) return undefined;
	cachePleines.delete(chemin);
	cachePleines.set(chemin, trouve);
	return trouve;
}

/** Range une image et évince la plus ancienne au-delà de `MAX_PLEINES`. */
function rangerPleine(chemin: string, src: string) {
	cachePleines.delete(chemin);
	cachePleines.set(chemin, src);
	while (cachePleines.size > MAX_PLEINES) {
		const plusAncien = cachePleines.keys().next();
		if (plusAncien.done) break;
		cachePleines.delete(plusAncien.value);
	}
}

/**
 * Charge une image pleine résolution, en passant par le cache.
 *
 * Les demandes concurrentes du même chemin sont partagées : sans cela, afficher une image
 * pendant que son préchargement est en vol la décoderait deux fois.
 */
const enVol = new Map<string, Promise<string>>();

function chargerPleine(chemin: string, gameDir?: string): Promise<string> {
	const connu = pleineDuCache(chemin);
	if (connu !== undefined) return Promise.resolve(connu);

	const dejaEnVol = enVol.get(chemin);
	if (dejaEnVol) return dejaEnVol;

	const promesse = api
		.texturePngB64(chemin, gameDir)
		.then((b64) => {
			const src = `data:image/png;base64,${b64}`;
			rangerPleine(chemin, src);
			return src;
		})
		.finally(() => enVol.delete(chemin));

	enVol.set(chemin, promesse);
	return promesse;
}

/** Plafond de listage d'une catégorie. `telop_waza` en porte 12 460 (neuf langues) : le plus gros
 * dossier de la galerie tient largement en dessous, et la borne protège d'un dossier inattendu. */
const MAX_PAR_CATEGORIE = 30000;

/** Vignette d'une illustration — même fabrique que l'Explorateur et l'Éditeur. */
function Vignette({ chemin, gameDir }: { chemin: string; gameDir?: string }) {
  const { ref, src } = useThumbnail(chemin, EXT_GALERIE, gameDir);
  return (
    <div
      ref={ref}
      className="flex aspect-video w-full items-center justify-center overflow-hidden rounded-lg bg-surface-container-highest"
    >
      {src ? (
        // Aperçu local décodé par le backend : pas de balise image optimisée ici (app Tauri).
        <img src={src} alt="" className="h-full w-full object-contain" loading="lazy" />
      ) : (
        <Icon name="image" size={22} className="text-on-surface-variant/50" />
      )}
    </div>
  );
}

/**
 * Visionneuse plein cadre. Contrairement à la grille, elle demande la texture PLEINE
 * RÉSOLUTION (`api.texturePngB64`) — une seule à la fois, à la demande : c'est le seul endroit où
 * ce coût est justifié.
 */
function Visionneuse({
  liste,
  index,
  onIndex,
  onFermer,
  onOuvrirDansExplorateur,
  gameDir,
}: {
  liste: Illustration[];
  index: number;
  onIndex: (i: number) => void;
  onFermer: () => void;
  onOuvrirDansExplorateur?: (chemin: string) => void;
  gameDir?: string;
}) {
  const item = liste[index];
  const [src, setSrc] = useState<string | null>(null);
  const [erreur, setErreur] = useState<string | null>(null);

  useEffect(() => {
    if (!item) return;
    let annule = false;

    // Une image déjà en cache s'affiche SANS repasser par `null` : sinon chaque flèche fait
    // clignoter la visionneuse alors que l'image est déjà là, ce qui annule tout le bénéfice
    // du préchargement.
    const connu = pleineDuCache(item.chemin);
    setSrc(connu ?? null);
    setErreur(null);

    if (connu === undefined) {
      chargerPleine(item.chemin, gameDir)
        .then((src) => (annule ? null : setSrc(src)))
        .catch((e) => {
          if (!annule) setErreur(String(e));
        });
    }

    // Préchargement des voisines : c'est la navigation aux flèches qui en profite. Les échecs
    // sont ignorés — une voisine illisible ne doit pas parasiter l'image qu'on regarde ; elle
    // signalera son erreur quand on arrivera dessus.
    for (const voisin of [liste[index + 1], liste[index - 1]]) {
      if (voisin) void chargerPleine(voisin.chemin, gameDir).catch(() => undefined);
    }

    return () => {
      annule = true;
    };
  }, [item, index, liste, gameDir]);

  useEffect(() => {
    function onTouche(e: KeyboardEvent) {
      if (e.key === "Escape") onFermer();
      else if (e.key === "ArrowRight") onIndex(Math.min(liste.length - 1, index + 1));
      else if (e.key === "ArrowLeft") onIndex(Math.max(0, index - 1));
    }
    window.addEventListener("keydown", onTouche);
    return () => window.removeEventListener("keydown", onTouche);
  }, [index, liste.length, onIndex, onFermer]);

  const exporter = useCallback(async () => {
    if (!item) return;
    try {
      const nom = await api.exportDefaultName(item.chemin, "png");
      const dest = await save({ defaultPath: nom });
      if (!dest) return;
      const ecrits = await api.exportAs(item.chemin, dest, "png", gameDir);
      toast.success(`${humanSize(ecrits)} écrits → ${dest}`);
    } catch (e) {
      toast.error(String(e));
    }
  }, [item, gameDir]);

  if (!item) return null;

  return (
    <div className="absolute inset-0 z-50 flex flex-col bg-app/95 backdrop-blur-sm">
      <div className="flex items-center gap-3 border-b border-app-line px-4 py-2">
        <div className="min-w-0 flex-1">
          <p className="truncate type-title-small text-on-surface">{item.titre}</p>
          <p className="truncate type-label-small text-on-surface-variant">{item.chemin}</p>
        </div>
        <Badge variant="outline">{humanSize(item.octets)}</Badge>
        {item.deblocage && <Badge variant="secondary">{item.deblocage}</Badge>}
        {item.episode !== null && <Badge variant="outline">épisode {item.episode}</Badge>}
        <button
          type="button"
          className="state-layer rounded-md px-2 py-1 type-label-medium text-on-surface-variant"
          onClick={exporter}
        >
          <Icon name="download" size={16} /> Exporter en PNG…
        </button>
        {onOuvrirDansExplorateur && (
          <button
            type="button"
            className="state-layer rounded-md px-2 py-1 type-label-medium text-on-surface-variant"
            onClick={() => onOuvrirDansExplorateur(item.chemin)}
          >
            <Icon name="folder_open" size={16} /> Ouvrir
          </button>
        )}
        <button
          type="button"
          aria-label="Fermer"
          className="state-layer rounded-md px-2 py-1 text-on-surface-variant"
          onClick={onFermer}
        >
          <Icon name="close" size={18} />
        </button>
      </div>

      <div className="relative flex min-h-0 flex-1 items-center justify-center p-4">
        <button
          type="button"
          aria-label="Précédente"
          disabled={index === 0}
          className="state-layer absolute left-2 rounded-full p-2 text-on-surface disabled:opacity-30"
          onClick={() => onIndex(index - 1)}
        >
          <Icon name="chevron_left" size={28} />
        </button>
        {erreur ? (
          <Alert variant="destructive" className="max-w-lg">
            <AlertTitle>Décodage impossible</AlertTitle>
            <AlertDescription>{erreur}</AlertDescription>
          </Alert>
        ) : src ? (
          <img src={src} alt={item.titre} className="max-h-full max-w-full object-contain" />
        ) : (
          <p className="type-body-medium text-on-surface-variant">décodage…</p>
        )}
        <button
          type="button"
          aria-label="Suivante"
          disabled={index >= liste.length - 1}
          className="state-layer absolute right-2 rounded-full p-2 text-on-surface disabled:opacity-30"
          onClick={() => onIndex(index + 1)}
        >
          <Icon name="chevron_right" size={28} />
        </button>
      </div>

      <div className="border-t border-app-line px-4 py-1.5 text-center type-label-small text-on-surface-variant">
        {index + 1} / {liste.length.toLocaleString("fr-FR")} — ← → pour naviguer, Échap pour fermer
      </div>
    </div>
  );
}

export function GalleryView({ onOpenFile }: { onOpenFile?: (path: string) => void }) {
  const settings = useSettings();
  const [categories, setCategories] = useState<VfsDir[]>([]);
  const [categorie, setCategorie] = useState<string | null>(null);
  const [sousDossiers, setSousDossiers] = useState<VfsDir[]>([]);
  const [sousDossier, setSousDossier] = useState<string | null>(null);
  const [items, setItems] = useState<Illustration[]>([]);
  const [recherche, setRecherche] = useState("");
  const [visibles, setVisibles] = useState(PAR_PAGE);
  const [chargement, setChargement] = useState(true);
  const [erreur, setErreur] = useState<string | null>(null);
  const [ouvert, setOuvert] = useState<number | null>(null);
  /** `gallery_config` indexé par nom de fichier — chargé une fois, réutilisé par toutes les pages. */
  const [enrichissements, setEnrichissements] = useState<Map<string, EnrichissementGalerie>>(
    new Map(),
  );

  // Catégories = sous-dossiers RÉELS de la racine. Aucune liste écrite d'avance : un dossier
  // ajouté par une mise à jour du jeu apparaît de lui-même.
  useEffect(() => {
    setChargement(true);
    setErreur(null);
    api
      .ls(RACINE_GALERIE, settings.gameDir)
      .then((l) => {
        setCategories(l.dirs);
        setCategorie((c) => c ?? l.dirs[0]?.name ?? null);
        return null;
      })
      .catch((e) => setErreur(String(e)))
      .finally(() => setChargement(false));
  }, [settings.gameDir]);

  // `gallery_config` : ce que le jeu sait des illustrations qu'il expose dans son menu Galerie.
  // Best-effort — la galerie liste le VFS avec ou sans lui.
  useEffect(() => {
    api
      .gameDataGallery(settings.gameDir)
      .then((lignes) => {
        const m = new Map<string, EnrichissementGalerie>();
        for (const g of lignes) {
          if (g.img_path) m.set(g.img_path, { deblocage: g.unlock_kind, episode: g.story_episode });
          if (g.thumb_path)
            m.set(g.thumb_path, { deblocage: g.unlock_kind, episode: g.story_episode });
        }
        setEnrichissements(m);
        return null;
      })
      .catch(() => setEnrichissements(new Map()));
  }, [settings.gameDir]);

  // Sous-dossiers de la catégorie courante (langues de `telop_waza`/`hlp`/`stamp_img`, variantes
  // de `ev_pic`/`ev_telop`).
  useEffect(() => {
    if (!categorie) return;
    setSousDossier(null);
    api
      .ls(`${RACINE_GALERIE}/${categorie}`, settings.gameDir)
      .then((l) => setSousDossiers(l.dirs))
      .catch(() => setSousDossiers([]));
  }, [categorie, settings.gameDir]);

  // Contenu de la catégorie (ou du sous-dossier). Chargé EN ENTIER une fois : la base est un
  // fichier local, et tout garder en mémoire permet de chercher et paginer sans rappeler le VFS
  // à chaque frappe.
  useEffect(() => {
    if (!categorie) return;
    let annule = false;
    setChargement(true);
    setErreur(null);
    setVisibles(PAR_PAGE);
    api
      .findPaged(
        prefixeCategorie(categorie, sousDossier),
        EXT_GALERIE,
        MAX_PAR_CATEGORIE,
        0,
        settings.gameDir,
      )
      .then((page) => {
        if (!annule) setItems(construireIllustrations(page.files, enrichissements));
        return null;
      })
      .catch((e) => {
        if (!annule) setErreur(String(e));
      })
      .finally(() => {
        if (!annule) setChargement(false);
      });
    return () => {
      annule = true;
    };
  }, [categorie, sousDossier, enrichissements, settings.gameDir]);

  const filtres = useMemo(() => filtrerIllustrations(items, recherche), [items, recherche]);
  const affiches = useMemo(() => filtres.slice(0, visibles), [filtres, visibles]);

  /** Sentinelle de fin de grille : sa venue à l'écran déclenche la page suivante. */
  const sentinelle = useRef<HTMLButtonElement | null>(null);
  const reste = visibles < filtres.length;

  // Chargement automatique au défilement. La marge de 300 px déclenche AVANT que la sentinelle
  // n'entre réellement dans le champ : les vignettes suivantes sont donc déjà demandées quand
  // l'utilisateur arrive dessus, au lieu d'apparaître en retard sous ses yeux.
  //
  // La boucle se referme d'elle-même : chaque déclenchement augmente `visibles`, ce qui
  // recalcule `reste` et réattache un observateur ; quand tout est affiché, `reste` passe à
  // faux et l'effet ne se remonte plus. `setVisibles` est appelé sous forme fonctionnelle pour
  // que deux déclenchements rapprochés s'additionnent au lieu de s'écraser.
  useEffect(() => {
    if (!reste) return;
    const el = sentinelle.current;
    if (!el) return;
    const obs = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) {
          setVisibles((v) => Math.min(v + PAR_PAGE, filtres.length));
        }
      },
      { rootMargin: "300px" },
    );
    obs.observe(el);
    return () => obs.disconnect();
  }, [reste, filtres.length]);
  const total = useMemo(
    () => categories.reduce((somme, d) => somme + d.count, 0),
    [categories],
  );

  return (
    <div className="relative flex h-full min-h-0 flex-col gap-3 p-3">
      <div className="flex flex-wrap items-center gap-2">
        <h2 className="type-title-small text-on-surface">Galerie</h2>
        <Badge variant="secondary">{total.toLocaleString("fr-FR")} illustrations</Badge>
        <Input
          className="ml-auto w-64"
          placeholder="Rechercher une illustration…"
          value={recherche}
          onChange={(e) => setRecherche(e.target.value)}
        />
      </div>

      {erreur && (
        <Alert variant="destructive">
          <AlertTitle>Galerie indisponible</AlertTitle>
          <AlertDescription>{erreur}</AlertDescription>
        </Alert>
      )}

      <div className="grid min-h-0 flex-1 grid-cols-[minmax(180px,220px)_1fr] gap-3">
        <ScrollArea className="min-h-0 rounded-2xl border border-app-line bg-app-dark-box">
          <div className="divide-y divide-app-line">
            {categories.map((c) => (
              <button
                key={c.name}
                type="button"
                className={`state-layer flex w-full items-center justify-between gap-2 px-3 py-2 text-left type-body-medium ${
                  categorie === c.name
                    ? "bg-secondary-container text-on-secondary-container"
                    : "text-on-surface"
                }`}
                onClick={() => setCategorie(c.name)}
              >
                <span className="min-w-0 flex-1 truncate">{libelleCategorie(c.name)}</span>
                <span className="tabular-nums type-label-small text-on-surface-variant">
                  {c.count.toLocaleString("fr-FR")}
                </span>
              </button>
            ))}
            {categories.length === 0 && !chargement && (
              <p className="p-4 type-body-small text-on-surface-variant">
                Aucun dossier sous {RACINE_GALERIE} — le VFS est-il monté ?
              </p>
            )}
          </div>
        </ScrollArea>

        <div className="flex min-h-0 flex-col gap-2">
          {sousDossiers.length > 0 && (
            <div className="flex flex-wrap gap-1.5">
              <button
                type="button"
                className={`rounded-full px-3 py-1 type-label-medium ${
                  sousDossier === null
                    ? "bg-primary text-on-primary"
                    : "bg-surface-container text-on-surface-variant"
                }`}
                onClick={() => setSousDossier(null)}
              >
                Tout ({items.length.toLocaleString("fr-FR")})
              </button>
              {sousDossiers.map((d) => (
                <button
                  key={d.name}
                  type="button"
                  title={LANGUES.has(d.name) ? "Variante de langue" : undefined}
                  className={`rounded-full px-3 py-1 type-label-medium ${
                    sousDossier === d.name
                      ? "bg-primary text-on-primary"
                      : "bg-surface-container text-on-surface-variant"
                  }`}
                  onClick={() => setSousDossier(d.name)}
                >
                  {libelleSousDossier(d.name)} ({d.count.toLocaleString("fr-FR")})
                </button>
              ))}
            </div>
          )}

          <div className="flex items-center gap-2 type-label-small text-on-surface-variant">
            {chargement
              ? "chargement…"
              : `${filtres.length.toLocaleString("fr-FR")} illustration(s)${
                  recherche.trim() ? ` sur ${items.length.toLocaleString("fr-FR")}` : ""
                }`}
          </div>

          <ScrollArea className="min-h-0 flex-1 rounded-2xl border border-app-line bg-app-dark-box p-2">
            <div className="grid grid-cols-[repeat(auto-fill,minmax(150px,1fr))] gap-2">
              {affiches.map((it, i) => (
                <button
                  key={it.chemin}
                  type="button"
                  className="state-layer flex flex-col gap-1 rounded-lg p-1.5 text-left"
                  onClick={() => setOuvert(i)}
                  onDoubleClick={() => onOpenFile?.(it.chemin)}
                >
                  <Vignette chemin={it.cheminVignette} gameDir={settings.gameDir} />
                  <span className="truncate type-label-small text-on-surface" title={it.titre}>
                    {it.titre}
                  </span>
                  <span className="truncate type-label-small text-on-surface-variant">
                    {humanSize(it.octets)}
                    {it.deblocage ? ` · ${it.deblocage}` : ""}
                  </span>
                </button>
              ))}
            </div>
            {reste && (
              // La sentinelle EST le bouton, et c'est délibéré : le défilement la déclenche
              // seul, mais elle reste actionnable au clavier — et sert de repli si un
              // conteneur exotique empêchait l'observateur de se déclencher.
              <button
                ref={sentinelle}
                type="button"
                className="state-layer mt-2 w-full rounded-lg py-2 type-label-medium text-on-surface-variant"
                onClick={() => setVisibles((v) => Math.min(v + PAR_PAGE, filtres.length))}
              >
                Chargement… ({(filtres.length - visibles).toLocaleString("fr-FR")} restantes)
              </button>
            )}
            {!chargement && filtres.length === 0 && (
              <p className="p-4 type-body-small text-on-surface-variant">
                Aucune illustration ne correspond.
              </p>
            )}
          </ScrollArea>
        </div>
      </div>

      {ouvert !== null && (
        <Visionneuse
          liste={affiches}
          index={ouvert}
          onIndex={setOuvert}
          onFermer={() => setOuvert(null)}
          onOuvrirDansExplorateur={onOpenFile}
          gameDir={settings.gameDir}
        />
      )}
    </div>
  );
}
