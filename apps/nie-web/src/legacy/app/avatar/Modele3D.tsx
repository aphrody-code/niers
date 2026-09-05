"use client";

import { useEffect, useRef, useState } from "react";
import { Button } from "@rosegriffon/ui/button";
import { Slider } from "@rosegriffon/ui/slider";
import { loadModelViewer } from "../../lib/model-viewer-loader";
import { telecharger, type Projet } from "./projet";
import "./atelier.css";

type Viewer = HTMLElement & {
 exportScene: () => Promise<Blob>;
 /** Capture la vue courante. `idealAspect: false` garde le cadrage affiché à l'écran. */
 toBlob: (options?: { mimeType?: string; qualityArgument?: number; idealAspect?: boolean }) => Promise<Blob>;
 updateFraming: () => void;
 updateComplete: Promise<boolean>;
};
const IDENTITE = { rotation: 0, echelle: 1 };

/** Un seul viewer : les changements de recette préservent la caméra. */
export function Modele3D({ url, transformation = IDENTITE, edition = false }: {
 url: string; transformation?: Projet["transformation"]; edition?: boolean;
}) {
 const hote = useRef<HTMLDivElement>(null);
 const viewer = useRef<Viewer | null>(null);
 const [instance, setInstance] = useState<Viewer | null>(null);
 const revision = useRef(0);
 const exportToken = useRef(0);
 const [charge, setCharge] = useState(false);
 const [erreur, setErreur] = useState("");
 const [tentative, setTentative] = useState(0);
 const [exporte, setExporte] = useState(false);
 useEffect(() => {
  let annule = false;
  let element: Viewer | null = null;
  const loaded = () => { if (!annule) { setCharge(true); setErreur(""); } };
  const failed = () => { if (!annule) { setCharge(false); setErreur("Assemblage 3D indisponible. Réessayez ou choisissez une autre pièce."); } };
  setCharge(false);
  setExporte(false);
  setErreur("");
  loadModelViewer().then(() => {
   if (annule || !hote.current) return;
   const mv = document.createElement("model-viewer") as Viewer;
   element = mv;
   for (const [nom, valeur] of Object.entries({ alt: "Avatar NIE éditable en 3D", "camera-controls": "", "touch-action": "pan-y", "camera-orbit": "0deg 82deg auto", "interaction-prompt": "none", exposure: "1", "shadow-intensity": "0" })) mv.setAttribute(nom, valeur);
   mv.style.cssText = "width:100%;height:100%;min-height:28rem;flex:1";
   mv.addEventListener("load", loaded);
   mv.addEventListener("error", failed);
   hote.current.appendChild(mv); viewer.current = mv; setInstance(mv);
  }).catch(e => { if (!annule) setErreur(e instanceof Error ? e.message : "Visualiseur indisponible."); });
  return () => {
   annule = true; revision.current++; exportToken.current++;
   element?.removeEventListener("load", loaded);
   element?.removeEventListener("error", failed);
   element?.remove();
   if (viewer.current === element) viewer.current = null;
  };
 }, [tentative]);
 useEffect(() => {
  if (!instance || instance !== viewer.current) return;
  revision.current++;
  setCharge(false); setErreur(""); instance.setAttribute("src", url);
 }, [url, instance]);
 useEffect(() => {
  if (!instance || instance !== viewer.current) return;
  revision.current++;
  instance.setAttribute("orientation", `0deg 0deg ${transformation.rotation}deg`);
  instance.setAttribute("scale", `${transformation.echelle} ${transformation.echelle} ${transformation.echelle}`);
 }, [instance, transformation.rotation, transformation.echelle]);
 return <div className="atelier-visualiseur">
  <div ref={hote} className="atelier-visualiseur-hote" />
  <div className="atelier-visualiseur-outils">
   <p role="status" className="mr-auto text-sm">{erreur || (charge ? "Modèle prêt" : "Chargement du modèle…")}</p>
   {erreur && <Button variant="outline" onClick={() => { setCharge(false); setTentative(t => t + 1); }}>Réessayer</Button>}
   {edition && <>
    <Button variant="outline" disabled={!charge} onClick={() => { viewer.current?.setAttribute("camera-orbit", "0deg 82deg auto"); viewer.current?.updateFraming(); }}>Recadrer</Button>
    <Button disabled={!charge || exporte} onClick={async () => {
     const mv = viewer.current;
     if (!mv) return;
     const version = revision.current;
     const token = ++exportToken.current;
     setExporte(true);
     try {
      await mv.updateComplete;
      if (viewer.current !== mv || revision.current !== version) return;
      const blob = await mv.exportScene();
      if (viewer.current === mv && revision.current === version) telecharger(blob, "avatar.glb");
     }
     catch { if (viewer.current === mv) setErreur("Export GLB impossible. Réessayez après le chargement complet."); }
     finally { if (viewer.current === mv && exportToken.current === token) setExporte(false); }
    }}>{exporte ? "Export en cours…" : "Exporter le GLB"}</Button>
    {/*
      L'image de la vue courante. Le GLB s'ouvre dans un logiciel 3D ; une planche de
      référence, une fiche, un message se collent avec un PNG. `idealAspect: false` capture
      exactement le cadrage à l'écran, pas un cadrage recalculé.
    */}
    <Button variant="outline" disabled={!charge} onClick={async () => {
     const mv = viewer.current;
     if (!mv) return;
     const version = revision.current;
     try {
      await mv.updateComplete;
      if (viewer.current !== mv || revision.current !== version) return;
      const image = await mv.toBlob({ mimeType: "image/png", idealAspect: false });
      if (viewer.current === mv && revision.current === version) telecharger(image, "avatar.png");
     } catch { if (viewer.current === mv) setErreur("Capture impossible. Réessayez après le chargement complet."); }
    }}>Capturer en PNG</Button>
   </>}
  </div>
 </div>;
}

/** Le kit expose le root ; le nom accessible est aussi posé sur son thumb interactif. */
export function CurseurAtelier({ label, valeur, unite, min, max, step, changer }: {
 label: string; valeur: number; unite: string; min: number; max: number; step: number; changer: (valeur: number) => void;
}) {
 return <div className="atelier-curseur">
  <div><span>{label}</span><output>{Number(valeur.toFixed(2))}{unite}</output></div>
  <Slider aria-label={label} min={min} max={max} step={step} value={[valeur]}
   ref={element => { element?.querySelector('[role="slider"]')?.setAttribute("aria-label", label); }}
   onValueChange={values => { if (values[0] !== undefined) changer(values[0]); }} />
 </div>;
}
