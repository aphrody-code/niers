"use client";

import { useEffect, useRef, useState } from "react";
import { Modele3D, CurseurAtelier } from "./Modele3D";
import { Button } from "@rosegriffon/ui/button";
import { Input } from "@rosegriffon/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@rosegriffon/ui/select";
import "./atelier.css";
import { grille, importerLocal, type ImportLocal, type Planche, type Region } from "./import-local";
import { telecharger } from "./projet";


/** Imports éphémères : aucun fichier envoyé au serveur, la recette courante reste intacte. */
export function ImportAvatar({ retour }: { retour(): void }) {
	const [asset, setAsset] = useState<ImportLocal | null>(null);
	const courant = useRef<ImportLocal | null>(null);
	const requete = useRef(0);
	const [occupe, setOccupe] = useState(false);
	const [erreur, setErreur] = useState("");
	const [transformation, setTransformation] = useState({ rotation: 0, echelle: 1 });
	useEffect(() => () => { requete.current++; courant.current?.liberer(); courant.current = null; }, []);
	async function charger(fichiers: File[]) {
		if (!fichiers.length) return;
		const id = ++requete.current;
		setOccupe(true); setErreur("");
		try {
			const suivant = await importerLocal(fichiers);
			if (requete.current !== id) { suivant.liberer(); return; }
			const ancien = courant.current;
			courant.current = suivant; setAsset(suivant); setTransformation({ rotation: 0, echelle: 1 });
			ancien?.liberer();
		} catch (e) { if (requete.current === id) setErreur(e instanceof Error ? e.message : "Import impossible."); }
		finally { if (requete.current === id) setOccupe(false); }
	}
	return <section className="nie-atelier" aria-label="Import avatar local">
		<header className="atelier-entete">
			<div className="atelier-titre"><h1>Importer un avatar</h1><p>Images, planches et modèles 3D.</p></div>
			<Button variant="outline" className="text-[#172c48]" onClick={retour}>Revenir au catalogue</Button>
		</header>
		<div className="atelier-import-corps">
		<p>Import local, sans envoi de fichiers. La recette du catalogue est conservée. Les imports ne sont pas enregistrés dans son JSON : exportez le résultat avant de quitter.</p>
		<div className="atelier-depot" onDragOver={e => { e.preventDefault(); }} onDrop={e => {
			e.preventDefault(); void charger(Array.from(e.dataTransfer.files));
		}}>
			<label className="atelier-champ">Fichiers de l’avatar (ou glisser-déposer)
				<Input className="mt-2 block max-w-full" type="file" multiple aria-label="Importer des fichiers avatar"
					accept=".png,.jpg,.jpeg,.webp,.g4tx,.json,.glb,.gltf,.vrm,.bin,.ktx2,.g4md,.g4mg,.gz"
					onChange={e => { const fichiers = Array.from(e.target.files ?? []); e.target.value = ""; void charger(fichiers); }} />
			</label>
			<p className="mt-3 text-sm">2D : PNG, JPEG, WebP, G4TX ; atlas JSON NIE optionnel. 3D : GLB, glTF avec textures et .bin, VRM (aperçu), G4MD + G4MG (géométrie). Gzip : .glb.gz, .gltf.gz, images .gz. Jusqu’à 200 fichiers, 64 Mio chacun / 128 Mio au total.</p>
			<p className="text-sm">GLB compressés Draco/Meshopt : décodeurs du visualiseur. ZIP, FBX, OBJ, .blend : convertir en GLB avant import. Aucun transfert automatique de squelette ou de pièces.</p>
		</div>
		<p role="status">{occupe ? "Lecture et validation…" : erreur || (asset ? `${asset.nom} — ${asset.remarque}` : "Choisissez les fichiers ; pour glTF, sélectionnez aussi toutes ses ressources.")}</p>
		{asset?.type === "3d" && <div className="atelier-import-scene">
   <div className="atelier-scene"><Modele3D key={asset.url} url={asset.url} transformation={transformation} edition /></div>
   <aside className="atelier-panneau" aria-label="Transformations du modèle importé"><h2>Objet complet</h2>
    <CurseurAtelier label="Rotation du modèle importé" valeur={transformation.rotation} unite="°" min={-180} max={180} step={5} changer={rotation => setTransformation(t => ({ ...t, rotation }))} />
    <CurseurAtelier label="Échelle du modèle importé" valeur={transformation.echelle} unite="×" min={0.25} max={4} step={0.05} changer={echelle => setTransformation(t => ({ ...t, echelle }))} />
    <Button variant="outline" className="atelier-bouton-large" onClick={() => setTransformation({ rotation: 0, echelle: 1 })}>Réinitialiser les transformations</Button>
    <p className="atelier-note">Glissez pour orbiter. Exportez votre modèle avant de revenir au catalogue.</p>
   </aside>
  </div>}
		{asset?.type === "2d" && <Planches key={asset.planches[0].url} planches={asset.planches} />}
	</div>
	</section>;
}

function Planches({ planches }: { planches: Planche[] }) {
	const [index, setIndex] = useState(0);
	return <div className="space-y-4">
		{planches.length > 1 && <label className="atelier-champ">Planche <Select value={String(index)} onValueChange={value => setIndex(Number(value))}>
   <SelectTrigger aria-label="Planche"><SelectValue /></SelectTrigger><SelectContent className="atelier-menu">
   {planches.map((p, i) => <SelectItem key={p.url} value={String(i)}>{i + 1}. {p.nom}</SelectItem>)}
   </SelectContent>
  </Select></label>}
		<Planche2D key={planches[index].url} planche={planches[index]} />
	</div>;
}

function Planche2D({ planche }: { planche: Planche }) {
	const [colonnes, setColonnes] = useState(1), [lignes, setLignes] = useState(1);
	const [atlas, setAtlas] = useState(planche.regions.length > 0);
	const [index, setIndex] = useState(0), [lecture, setLecture] = useState(false), [fps, setFps] = useState(8);
	const [pret, setPret] = useState(false), [erreurImage, setErreurImage] = useState("");
	const canvas = useRef<HTMLCanvasElement>(null);
	const image = useRef<HTMLImageElement | null>(null);
	let regions: Region[] = [], erreur = "";
	try { regions = atlas ? planche.regions : grille(planche.largeur, planche.hauteur, colonnes, lignes); }
	catch (e) { erreur = (e as Error).message; }
	const region = regions[Math.min(index, regions.length - 1)];
	useEffect(() => {
		let actif = true;
		const img = new Image(); img.src = planche.url;
		void img.decode().then(() => { if (actif) { image.current = img; setPret(true); } }, () => { if (actif) setErreurImage("Image indécodable."); });
		return () => { actif = false; image.current = null; };
	}, [planche.url]);
	useEffect(() => {
		const c = canvas.current, img = image.current;
		if (!c || !img || !region) return;
		c.width = region.largeur; c.height = region.hauteur;
		const ctx = c.getContext("2d");
		if (!ctx) return;
		ctx.imageSmoothingEnabled = false;
		ctx.clearRect(0, 0, c.width, c.height);
		ctx.drawImage(img, region.x, region.y, region.largeur, region.hauteur, 0, 0, c.width, c.height);
	}, [pret, region?.x, region?.y, region?.largeur, region?.hauteur]);
	useEffect(() => {
		if (!lecture || regions.length < 2) return;
		const id = window.setInterval(() => setIndex(i => (i + 1) % regions.length), 1000 / fps);
		return () => clearInterval(id);
	}, [lecture, regions.length, fps]);
	function configurer(c: number, l: number) { setColonnes(c); setLignes(l); setAtlas(false); setIndex(0); setLecture(false); }
	return <div className="space-y-4" aria-label="Édition de planche 2D">
		<p>{planche.nom} · {planche.largeur} × {planche.hauteur} px · {regions.length} régions</p>
		<div className="flex flex-wrap items-center gap-3">
			<Button variant="outline" onClick={() => configurer(1, 1)}>Image entière</Button>
			<Button variant="outline" onClick={() => configurer(3, 4)}>Chara 3 × 4</Button>
			<Button variant="outline" onClick={() => configurer(4, 4)}>Chara 4 × 4</Button>
			{planche.regions.length > 0 && <Button variant="outline" onClick={() => { setAtlas(true); setIndex(0); setLecture(false); }}>Régions de l’atlas</Button>}
			<label>Colonnes <Input className="w-24" type="number" min={1} max={4096} value={colonnes} onChange={e => configurer(Number(e.target.value), lignes)} /></label>
			<label>Lignes <Input className="w-24" type="number" min={1} max={4096} value={lignes} onChange={e => configurer(colonnes, Number(e.target.value))} /></label>
		</div>
		{(erreur || erreurImage) && <p role="alert">{erreur || erreurImage}</p>}
		{region && <>
			<div className="flex flex-wrap items-center gap-3">
				<label className="atelier-champ">Région <Select value={String(Math.min(index, regions.length - 1))} onValueChange={value => { setIndex(Number(value)); setLecture(false); }}>
     <SelectTrigger aria-label="Région"><SelectValue /></SelectTrigger><SelectContent className="atelier-menu">
     {regions.map((r, i) => <SelectItem key={i} value={String(i)}>{r.nom}</SelectItem>)}
     </SelectContent>
    </Select></label>
				<Button variant="outline" disabled={!pret || regions.length < 2} onClick={() => setLecture(v => !v)}>{lecture ? "Pause" : "Animer la planche"}</Button>
				<label>Images/s <Input className="w-24" type="number" min={1} max={60} value={fps} onChange={e => setFps(Math.max(1, Math.min(60, Number(e.target.value) || 1)))} /></label>
			</div>
			<canvas ref={canvas} role="img" aria-label={`Aperçu : ${region.nom}`} style={{ maxWidth: "100%", maxHeight: 480, imageRendering: "pixelated", background: "repeating-conic-gradient(#ddd 0% 25%, #fff 0% 50%) 0 / 20px 20px" }} />
			<div className="flex flex-wrap gap-3">
				<Button variant="outline" disabled={!pret} onClick={() => { canvas.current?.toBlob(blob => { if (blob) telecharger(blob, "avatar-sprite.png"); }, "image/png"); }}>Exporter le sprite PNG</Button>
				<Button variant="outline" onClick={() => telecharger(new Blob([JSON.stringify({ nom: planche.nom, largeur: planche.largeur, hauteur: planche.hauteur, sprites: regions }, null, 2)], { type: "application/json" }), "avatar-atlas.json")}>Exporter l’atlas JSON</Button>
			</div>
			<p className="text-sm">Le JSON décrit les rectangles ; conservez aussi l’image source pour le réimport. Les préréglages Chara sont des grilles explicites, pas une détection de format du jeu.</p>
		</>}
	</div>;
}
