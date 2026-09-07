import { useEffect, useMemo, useState } from "react";
import { Note, TitreVue } from "./Ecran";

type Part = { id: string; itemNo: number; resource: string; modeles?: string[]; modeles2?: string[]; icone?: string | null };
type Category = { faceSettingType: number; prefixe: string; parts: Part[]; couleurs?: string[] };
type Catalogue = { source: string; categories: Category[]; couleursRgb?: Record<string, { rgb: string; alpha: number }>; modelesDeBase: { morphologies: string[] }; presets?: unknown[] };
type Legacy = { parts?: unknown[]; colors?: unknown[]; voices?: unknown[] };
type Famille<T> = { donnees: T; octets: number; chemin: string };

const CDN = "/assets";
const NOMS: Record<number, string> = {
  1: "Visages prédéfinis", 2: "Forme du visage", 3: "Peau", 4: "Coiffure", 5: "Frange",
  6: "Yeux", 7: "Pupilles", 8: "Reflets", 9: "Nez", 10: "Bouche", 11: "Sourcils",
  12: "Oreilles", 13: "Marques du visage", 14: "Accessoires", 16: "Genre", 17: "Morphologie",
  18: "Poitrine", 19: "Col", 20: "Manches", 21: "Ourlet",
};

/** Atelier issu de l'ancien éditeur Azalée, réadmis sur les contrats de nie-site. */
export function Avatar() {
  const [catalogue, setCatalogue] = useState<Catalogue | null>(null);
  const [legacy, setLegacy] = useState<Famille<Legacy> | null>(null);
  const [categorie, setCategorie] = useState(1);
  const [choix, setChoix] = useState<Record<number, string>>({});
  const [morphologie, setMorphologie] = useState(0);
  const [taille, setTaille] = useState(7);
  const [erreur, setErreur] = useState(false);

  useEffect(() => {
    const ac = new AbortController();
    lire<Catalogue>(CDN + "/avatar/catalog.json", ac.signal)
      .then(setCatalogue)
      .catch(() => lire<Famille<Legacy>>("/api/v1/donnees/famille/chara_edit", ac.signal)
        .then(setLegacy)
        .catch(() => { if (!ac.signal.aborted) setErreur(true); }));
    return () => ac.abort();
  }, []);

  const categories = catalogue?.categories ?? [];
  const active = categories.find((c) => c.faceSettingType === categorie) ?? categories[0];
  const url = useMemo(() => catalogue ? composerUrl(catalogue, choix, morphologie, taille) : null, [catalogue, choix, morphologie, taille]);

  if (erreur) return <section><TitreVue>Éditeur d’avatar</TitreVue><Note ton="alerte">Les données de l’atelier ne sont pas disponibles pour le moment.</Note></section>;
  if (!catalogue && !legacy) return <section><TitreVue>Éditeur d’avatar</TitreVue><Note>Chargement du catalogue de pièces…</Note></section>;
  if (!catalogue) return <LegacyView data={legacy!} />;

  return (
    <section aria-labelledby="titre-avatar">
      <TitreVue appoint={totalParts(categories) + " pièces · " + totalColors(categories) + " couleurs"}><span id="titre-avatar">Éditeur d’avatar</span></TitreVue>
      <p style={{ margin: "0 0 var(--jeu-espace-l)", fontWeight: 700 }}>
        Catalogue résolu depuis les fichiers du jeu : {catalogue.presets?.length ?? 0} visages prédéfinis et {catalogue.modelesDeBase.morphologies.length} morphologies.
      </p>
      <div style={layout}>
        <div>
          <div role="tablist" aria-label="Familles de l’avatar" style={tabs}>
            {categories.map((c) => <button key={c.faceSettingType} type="button" role="tab" aria-selected={c.faceSettingType === active?.faceSettingType} onClick={() => setCategorie(c.faceSettingType)} style={bouton(c.faceSettingType === active?.faceSettingType)}>{NOMS[c.faceSettingType] ?? ("Pièces " + c.faceSettingType)} ({c.parts.length})</button>)}
          </div>
          <div style={panneau}>
            <h3 style={titrePanneau}>{NOMS[active?.faceSettingType ?? 0] ?? active?.prefixe}</h3>
            <div style={grille}>
              {(active?.parts ?? []).slice(0, 120).map((part, i) => {
                const choisi = choix[active!.faceSettingType] === part.id;
                const image = vignette(part.icone);
                return <button key={part.id} type="button" aria-pressed={choisi} onClick={() => setChoix((v) => ({ ...v, [active!.faceSettingType]: part.id }))} style={{ ...tuile, borderColor: choisi ? "var(--jeu-accent-azur)" : "transparent" }}>
                  {image ? <img src={image} alt="" loading="lazy" width={120} height={120} style={{ width: "100%", aspectRatio: "1", objectFit: "contain" }} /> : <span style={placeholder}>{colorFor(catalogue, active!, part) ?? "—"}</span>}
                  <span style={caption}>{part.resource !== "0xFFFFFFFF" ? part.resource : ("Variante " + (i + 1))}</span>
                </button>;
              })}
            </div>
            {(active?.parts.length ?? 0) > 120 ? <p style={{ fontWeight: 700 }}>Affichage des 120 premières pièces sur {active?.parts.length}.</p> : null}
          </div>
        </div>
        <aside style={panneau} aria-label="Réglages et aperçu">
          <h3 style={titrePanneau}>Silhouette</h3>
          <label style={champ}>Morphologie<select value={morphologie} onChange={(e) => setMorphologie(Number(e.target.value))}>{catalogue.modelesDeBase.morphologies.map((m, i) => <option key={m} value={i}>{m}</option>)}</select></label>
          <label style={champ}>Taille <output>{taille}</output><input type="range" min="0" max="14" value={taille} onChange={(e) => setTaille(Number(e.target.value))} /></label>
          <h3 style={titrePanneau}>Assemblage</h3>
          {url ? <><a href={url} target="_blank" rel="noreferrer" style={lien}>Ouvrir le GLB assemblé</a><p style={meta}>Pièces, textures faciales et modèle de corps résolus par le serveur.</p></> : <Note>Sélectionnez une pièce de modèle pour préparer un GLB.</Note>}
          <p style={meta}>Source : {catalogue.source}</p>
        </aside>
      </div>
    </section>
  );
}

function LegacyView({ data }: { data: Famille<Legacy> }) {
  return <section><TitreVue appoint={(data.donnees.parts?.length ?? 0) + " pièces · " + (data.donnees.colors?.length ?? 0) + " couleurs"}>Éditeur d’avatar</TitreVue><Note>Le catalogue résolu est temporairement indisponible. Les tables chara_edit restent consultables.</Note><pre style={ligneStyle}>{JSON.stringify(data.donnees, null, 2)}</pre></section>;
}

function composerUrl(catalogue: Catalogue, choix: Record<number, string>, morphologie: number, taille: number): string | null {
  const pieces = catalogue.categories.flatMap((c) => {
    const p = c.parts.find((part) => part.id === choix[c.faceSettingType]) ?? c.parts[0];
    return p ? [...(p.modeles ?? []), ...(p.modeles2 ?? [])].filter((x) => x.includes("/20_EDIT/") && x.endsWith(".g4md")).map((x) => x.split("/20_EDIT/")[1]?.replace(/\.g4md$/, "")).filter((x): x is string => Boolean(x)) : [];
  });
  const uniques = [...new Set(pieces)];
  if (!uniques.length) return null;
  const morpho = catalogue.modelesDeBase.morphologies[morphologie] ?? catalogue.modelesDeBase.morphologies[0] ?? "male";
  return CDN + "/model-avatar/" + uniques.join("+") + ".glb?morpho=" + encodeURIComponent(morpho) + "&taille=" + taille;
}
function vignette(icone?: string | null): string | null {
  if (!icone || !/^[A-Za-z0-9_]+_[0-9]+$/.test(icone)) return null;
  const i = icone.lastIndexOf("_");
  return CDN + "/tex/dx11/menu/200_icon/21_icon_avatar/" + icone.slice(0, i) + ".g4tx/" + icone + ".png";
}
function colorFor(catalogue: Catalogue, category: Category, part: Part): string | null {
  const id = category.couleurs?.[part.itemNo];
  return id ? catalogue.couleursRgb?.[id]?.rgb ?? null : null;
}
function totalParts(categories: Category[]) { return categories.reduce((n, c) => n + c.parts.length, 0); }
function totalColors(categories: Category[]) { return categories.reduce((n, c) => n + (c.couleurs?.length ?? 0), 0); }
async function lire<T>(url: string, signal: AbortSignal): Promise<T> { const r = await fetch(url, { signal, headers: { accept: "application/json" } }); if (!r.ok) throw new Error(url + " a répondu " + r.status); return (await r.json()) as T; }

const layout = { display: "grid", gridTemplateColumns: "minmax(0, 1fr) minmax(260px, .4fr)", gap: 18, alignItems: "start" } as const;
const tabs = { display: "flex", flexWrap: "wrap", gap: 8, marginBottom: 18 } as const;
const panneau = { background: "rgb(255 255 255 / 78%)", padding: "var(--jeu-espace-l)", boxShadow: "var(--jeu-ombre-panneau)" } as const;
const titrePanneau = { margin: "0 0 12px", color: "var(--jeu-nuit-profonde)" } as const;
const meta = { overflowWrap: "anywhere", fontSize: 12, fontWeight: 700 } as const;
const ligneStyle = { margin: 0, padding: 10, overflow: "auto", maxHeight: 420, background: "var(--jeu-surface-craie)", borderLeft: "3px solid var(--jeu-accent-azur)", fontSize: 11 } as const;
const grille = { display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(130px, 1fr))", gap: 10 } as const;
const tuile = { border: "2px solid transparent", background: "var(--jeu-surface-glace)", padding: 8, cursor: "pointer", textAlign: "left" } as const;
const caption = { display: "block", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", fontSize: 11, fontWeight: 800 } as const;
const placeholder = { display: "grid", placeItems: "center", aspectRatio: "1", fontSize: 28, fontWeight: 900 } as const;
const champ = { display: "grid", gap: 6, marginBottom: 18, fontWeight: 800 } as const;
const lien = { display: "inline-block", padding: "10px 14px", background: "var(--jeu-tuile-active-bas)", color: "var(--jeu-texte-vif)", fontWeight: 800, textDecoration: "none" } as const;
function bouton(actif: boolean) { return { border: 0, padding: "9px 13px", cursor: "pointer", fontWeight: 800, color: actif ? "var(--jeu-texte-vif)" : "var(--jeu-nuit-profonde)", background: actif ? "var(--jeu-tuile-active-bas)" : "var(--jeu-surface-glace)" } as const; }
