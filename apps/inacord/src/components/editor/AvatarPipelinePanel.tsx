// Atelier de convergence : il ne fabrique ni modèle ni sprite. Le catalogue est l'export de
// `niers avatar export`, le GLB vient de `nie-model-serve`, et le viewport parent charge cet
// artefact exactement comme n'importe quel GLB VFS.
import { useEffect, useMemo, useState } from "react";

import { api } from "@/lib/api";
import { cn } from "@/lib/utils";

type Part = { id: string; resource: string; modeles: string[]; modeles2: string[] };
type Category = { faceSettingType: number; prefixe: string; parts: Part[] };
type Catalogue = {
  categories: Category[];
  modelesDeBase: { morphologies: string[]; visages: { resources: string[] }[] };
};

function isCatalogue(value: unknown): value is Catalogue {
  if (!value || typeof value !== "object") return false;
  const c = value as Partial<Catalogue>;
  return Array.isArray(c.categories) && !!c.modelesDeBase && Array.isArray(c.modelesDeBase.morphologies);
}

/** Reproduit le routage du catalogue : mailles dans l'URL, couches de visage dans `face`. */
function avatarPath(catalogue: Catalogue, choices: Record<number, string>, morpho: number): string | null {
  const pieces: string[] = [];
  const face: string[] = [];
  const morphology = catalogue.modelesDeBase.morphologies[morpho];
  const body = catalogue.categories.find((c) => c.faceSettingType === 17)?.parts.find((p) => p.resource === `edit_body_${morphology}`);
  const skeleton = body?.modeles2.find((p) => p.endsWith(".g4sk"))?.split("/").pop()?.replace(/\.g4sk$/, "");
  if (skeleton) pieces.push(`_bodySK/${skeleton}`);
  for (const category of catalogue.categories) {
    const part = category.parts.find((p) => p.id === choices[category.faceSettingType]) ?? category.parts[0];
    if (!part) continue;
    for (const model of [...part.modeles, ...part.modeles2]) {
      const mesh = model.match(/\/20_EDIT\/([^/]+)\/([^/]+)\.g4md$/);
      if (mesh) pieces.push(`${mesh[1]}/${mesh[2]}`);
      const layer = model.match(/\/_facetex\/(.+)\.g4tx$/);
      if (layer) face.push(layer[1]);
    }
  }
  if (pieces.length === 0) return null;
  const query = [...new Set(face)].sort();
  return `model-avatar/${[...new Set(pieces)].join("+")}.glb${query.length ? `?face=${encodeURIComponent(query.join(","))}` : ""}`;
}

export function AvatarPipelinePanel({
  baseUrl,
  onGlb,
}: {
  baseUrl: string;
  onGlb: (glbB64: string) => void;
}) {
  const [catalogue, setCatalogue] = useState<Catalogue | null>(null);
  const [choices, setChoices] = useState<Record<number, string>>({});
  const [morpho, setMorpho] = useState(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    api.modelServiceAvatarCatalog(baseUrl).then((raw) => {
      if (cancelled) return;
      if (!isCatalogue(raw)) throw new Error("le catalogue avatar ne respecte pas le contrat attendu");
      setCatalogue(raw);
      setChoices(Object.fromEntries(raw.categories.filter((c) => c.parts[0]).map((c) => [c.faceSettingType, c.parts[0].id])));
      setMorpho(0);
    }).catch((e) => !cancelled && setError(String(e))).finally(() => !cancelled && setLoading(false));
    return () => { cancelled = true; };
  }, [baseUrl]);

  const path = useMemo(() => catalogue && avatarPath(catalogue, choices, morpho), [catalogue, choices, morpho]);
  const build = () => {
    if (!path) return;
    setLoading(true);
    setError(null);
    api.modelServiceAvatarGlbB64(baseUrl, path).then(onGlb).catch((e) => setError(String(e))).finally(() => setLoading(false));
  };

  if (loading && !catalogue) return <p className="px-3 py-2 text-tiny text-ink-faint">Chargement du catalogue avatar réel…</p>;
  if (!catalogue) return <p className="px-3 py-2 text-tiny text-status-error">Pipeline avatar indisponible : {error ?? "catalogue absent"}</p>;
  return (
    <section className="flex min-h-0 shrink-0 items-center gap-2 overflow-x-auto border-b border-app-line bg-app-dark-box px-2 py-1.5" aria-label="Assemblage avatar">
      <strong className="shrink-0 text-tiny text-ink">Avatar assemblé</strong>
      <select className="h-7 rounded border border-app-line bg-app-box px-1 text-tiny text-ink" value={morpho} onChange={(e) => setMorpho(Number(e.target.value))}>
        {catalogue.modelesDeBase.morphologies.map((name, index) => <option key={name} value={index}>{name}</option>)}
      </select>
      {catalogue.categories.filter((c) => c.parts.length).slice(0, 10).map((category) => (
        <select key={category.faceSettingType} className="h-7 max-w-28 rounded border border-app-line bg-app-box px-1 text-tiny text-ink" value={choices[category.faceSettingType] ?? ""}
          title={category.prefixe || `catégorie ${category.faceSettingType}`}
          onChange={(e) => setChoices((old) => ({ ...old, [category.faceSettingType]: e.target.value }))}>
          {category.parts.map((part, index) => <option key={part.id} value={part.id}>{part.resource || `${category.prefixe} ${index + 1}`}</option>)}
        </select>
      ))}
      <button type="button" className={cn("h-7 shrink-0 rounded px-2 text-tiny font-medium", path ? "bg-accent text-white hover:brightness-110" : "bg-app-hover text-ink-faint")}
        disabled={!path || loading} onClick={build}>{loading ? "Assemblage…" : "Assembler dans la scène"}</button>
      {error && <span className="max-w-80 truncate text-tiny text-status-error" title={error}>{error}</span>}
    </section>
  );
}
