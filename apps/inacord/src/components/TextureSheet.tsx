import { useEffect, useMemo, useRef, useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { toast } from "sonner";
import { api, type Texture } from "@/lib/api";
import { useSettings } from "@/lib/settings";
import { humanSize } from "@/lib/bytes";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";

/** Vignettes décodées en parallèle. 4 : un décodage BC7 est CPU-bound côté Rust, et chaque appel
 * y consomme un thread à pile de 16 Mio — en lancer 80 d'un coup fait tomber la fenêtre dans
 * exactement le mur que la vignette existe pour éviter. */
const CONCURRENCE = 4;

/** Sous-textures montées d'emblée ; le reste vient par tranches, à la demande. */
const PAGE = 60;

/**
 * Planche de contact d'un conteneur `.g4tx` : TOUTES ses textures nommées, pas seulement celle
 * que son nom de fichier désigne.
 *
 * Un conteneur IEVR n'est pas une image. `icon_item05.g4tx` porte 80 payloads DDS 256×256 nommés
 * (`eq_ac0100101`…), et les atlas spatiaux portent des régions nommées à rogner. L'aperçu du
 * panneau de détail n'en montre qu'une — la principale — et les 79 autres étaient jusqu'ici
 * invisibles depuis l'application, alors même que le décodeur savait les lire.
 *
 * Le catalogue ([`api.textureList`]) ne décode rien : c'est un parse d'en-tête. Seules les
 * vignettes réellement affichées sont décodées, et le plein format uniquement au clic.
 */
export function TextureSheet({ path }: { path: string }) {
  const settings = useSettings();
  const [textures, setTextures] = useState<Texture[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [filtre, setFiltre] = useState("");
  const [visibles, setVisibles] = useState(PAGE);
  /** Vignette par nom de texture, `""` = échec (ne pas retenter en boucle). */
  const [vignettes, setVignettes] = useState<Record<string, string>>({});
  /** Texture ouverte en plein format, `null` = aucune. */
  const [ouverte, setOuverte] = useState<{ nom: string; url: string } | null>(null);

  useEffect(() => {
    setTextures(null);
    setError(null);
    setVignettes({});
    setOuverte(null);
    setFiltre("");
    setVisibles(PAGE);
    setLoading(true);
    let vivant = true;
    api
      .textureList(path, settings.gameDir)
      .then((t) => vivant && setTextures(t))
      .catch((e) => vivant && setError(String(e)))
      .finally(() => vivant && setLoading(false));
    return () => {
      vivant = false;
    };
  }, [path, settings.gameDir]);

  const filtrees = useMemo(() => {
    const q = filtre.trim().toLowerCase();
    const t = textures ?? [];
    return q ? t.filter((x) => x.name.toLowerCase().includes(q)) : t;
  }, [textures, filtre]);
  const affichees = useMemo(() => filtrees.slice(0, visibles), [filtrees, visibles]);

  // Décodage des vignettes affichées, par vagues bornées. `demandees` empêche qu'un re-rendu
  // relance un décodage déjà en cours : sans lui, chaque arrivée de vignette rerelance les autres.
  const demandees = useRef<Set<string>>(new Set());
  useEffect(() => {
    demandees.current = new Set();
  }, [path]);
  useEffect(() => {
    let vivant = true;
    const file = affichees.map((t) => t.name).filter((n) => !demandees.current.has(n));
    if (file.length === 0) return;
    file.forEach((n) => demandees.current.add(n));

    (async () => {
      // `vivant` EST modifié — par la fonction de nettoyage de l'effet, quand la sélection change
      // pendant le décodage ; et l'attente est le mécanisme même du bornage de concurrence.
      // eslint-disable-next-line no-unmodified-loop-condition
      for (let i = 0; i < file.length && vivant; i += CONCURRENCE) {
        const lot = file.slice(i, i + CONCURRENCE);
        // eslint-disable-next-line no-await-in-loop
        const rendus = await Promise.all(
          lot.map(async (nom) => {
            try {
              const b64 = await api.textureNamedThumbB64(path, nom, 96, settings.gameDir);
              return [nom, `data:image/png;base64,${b64}`] as const;
            } catch {
              // Un payload non décodable (format DDS exotique) ne doit pas arrêter la planche :
              // la case reste vide, les autres s'affichent.
              return [nom, ""] as const;
            }
          }),
        );
        if (!vivant) return;
        setVignettes((prec) => ({ ...prec, ...Object.fromEntries(rendus) }));
      }
    })();
    return () => {
      vivant = false;
    };
  }, [affichees, path, settings.gameDir]);

  async function ouvrir(nom: string) {
    try {
      const b64 = await api.textureNamedPngB64(path, nom, settings.gameDir);
      setOuverte({ nom, url: `data:image/png;base64,${b64}` });
    } catch (e) {
      toast.error(String(e));
    }
  }

  async function exporter(nom: string) {
    try {
      const dest = await save({ defaultPath: `${nom}.png`, filters: [{ name: "PNG", extensions: ["png"] }] });
      if (!dest) return;
      const b64 = await api.textureNamedPngB64(path, nom, settings.gameDir);
      await api.saveBytesB64(dest, b64);
      toast.success(`${nom}.png écrit`);
    } catch (e) {
      toast.error(String(e));
    }
  }

  if (loading) return <p className="type-body-small text-on-surface-variant">Lecture du conteneur…</p>;
  if (error) return <p className="type-body-small text-error">{error}</p>;
  if (!textures) return null;
  if (textures.length === 0)
    return <p className="type-body-small text-on-surface-variant">Ce conteneur ne déclare aucune texture.</p>;

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center gap-2">
        <Badge variant="secondary">
          {textures.length.toLocaleString("fr-FR")} texture{textures.length > 1 ? "s" : ""}
        </Badge>
        {textures.length > 8 && (
          <Input
            value={filtre}
            onChange={(e) => {
              setFiltre(e.target.value);
              setVisibles(PAGE);
            }}
            placeholder="Filtrer par nom…"
            className="h-7 flex-1"
          />
        )}
      </div>

      {ouverte && (
        <div className="flex flex-col gap-1 rounded-lg border border-app-line bg-app-dark-box p-2">
          <div className="flex items-center justify-between gap-2">
            <span className="truncate type-label-small text-on-surface">{ouverte.nom}</span>
            <div className="flex gap-1">
              <Button size="sm" variant="ghost" onClick={() => exporter(ouverte.nom)}>
                Exporter PNG…
              </Button>
              <Button size="sm" variant="ghost" onClick={() => setOuverte(null)}>
                Fermer
              </Button>
            </div>
          </div>
          <img src={ouverte.url} alt={ouverte.nom} className="max-h-64 max-w-full self-center" />
        </div>
      )}

      <div className="grid gap-2" style={{ gridTemplateColumns: "repeat(auto-fill,minmax(84px,1fr))" }}>
        {affichees.map((t) => {
          const vignette = vignettes[t.name];
          return (
            <button
              key={`${t.id}-${t.name}`}
              type="button"
              className={`state-layer flex flex-col items-center gap-1 rounded-lg border p-1 text-center ${
                ouverte?.nom === t.name ? "border-primary" : "border-app-line"
              }`}
              onClick={() => ouvrir(t.name)}
              title={`${t.name} — ${t.width}×${t.height}, ${humanSize(t.size)}${
                t.regions > 0 ? `, ${t.regions} région(s)` : ""
              }`}
            >
              <div className="flex h-16 w-full items-center justify-center overflow-hidden rounded bg-app-dark-box">
                {vignette ? (
                  <img src={vignette} alt={t.name} className="max-h-16 max-w-full" />
                ) : vignette === "" ? (
                  <span className="type-label-small text-on-surface-variant">✕</span>
                ) : (
                  <span className="type-label-small text-on-surface-variant">…</span>
                )}
              </div>
              <span className="w-full truncate type-label-small text-on-surface">{t.name}</span>
              <span className="type-label-small text-on-surface-variant">
                {t.width}×{t.height}
              </span>
            </button>
          );
        })}
      </div>

      {filtrees.length > visibles && (
        <Button size="sm" variant="outline" onClick={() => setVisibles((n) => n + PAGE)}>
          Afficher {Math.min(PAGE, filtrees.length - visibles)} de plus ({filtrees.length - visibles} restantes)
        </Button>
      )}
    </div>
  );
}
