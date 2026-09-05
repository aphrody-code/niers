// Le PNG vient de `nie-model-serve::menu::render_menu` : sprites, ordre et coordonnées du layout
// du jeu. Ce panneau ne dessine aucune approximation CSS du menu.
import { useState } from "react";
import { api } from "@/lib/api";

export function MenuPipelinePanel({ baseUrl }: { baseUrl: string }) {
  const [screen, setScreen] = useState("");
  const [image, setImage] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const render = () => {
    setLoading(true); setError(null);
    api.modelServiceMenuPngB64(baseUrl, screen).then((png) => setImage(png)).catch((e) => setError(String(e))).finally(() => setLoading(false));
  };
  return (
    <section className="border-b border-app-line bg-app-dark-box px-2 py-1.5" aria-label="Rendu de menu">
      <div className="flex items-center gap-2">
        <strong className="shrink-0 text-tiny text-ink">Menu réel</strong>
        <input className="h-7 min-w-0 flex-1 rounded border border-app-line bg-app-box px-2 text-tiny text-ink" value={screen}
          placeholder="écran dont le layout a été exporté"
          aria-label="Nom de l'écran de menu" onChange={(e) => setScreen(e.target.value)} />
        <button type="button" className="h-7 rounded bg-accent px-2 text-tiny font-medium text-white disabled:opacity-50" disabled={loading || !screen.trim()} onClick={render}>
          {loading ? "Rendu…" : "Rendre"}
        </button>
        {image && <button type="button" className="h-7 rounded border border-app-line px-2 text-tiny text-ink" onClick={() => setImage(null)}>Fermer</button>}
      </div>
      {!image && !error && <p className="mt-1 text-tiny text-ink-faint">Le rendu utilise uniquement un layout exporté du jeu ; aucun écran par défaut n'est inventé.</p>}
      {error && <p className="mt-1 text-tiny text-status-error">{error}</p>}
      {image && <img className="mt-2 max-h-64 max-w-full rounded border border-app-line object-contain" src={`data:image/png;base64,${image}`} alt={`Rendu du menu ${screen}`} />}
    </section>
  );
}
