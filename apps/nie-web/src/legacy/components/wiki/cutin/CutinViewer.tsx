"use client";

/**
 * Viewer de cut-in moderne (fiche skill). Wrapper `"use client"` qui :
 *  - monte la scène R3F LAZY (bouton « Charger la scène 3D » → évite de payer le coût
 *    WebGL/wasm sur les 142 fiches) ; la scène elle-même est chargée via
 *    `next/dynamic { ssr:false }` ;
 *  - construit le manifeste (`fetchCutinManifest`) + le storyboard caméra, puis décode
 *    les couches d'effet g4pk **à la demande** (1 décode par toggle, jamais les 48 d'un
 *    coup) ;
 *  - expose une timeline (scrub/play), des toggles de couches, un Bloom, un reset
 *    caméra, et un export MP4 (WebCodecs) rendu frame-par-frame ;
 *  - retombe sur `<CharacterModelViewer>` (model-viewer) si les données échouent.
 *
 * Frontière stricte : ce module n'importe que des libs PURES/client-safe (cpk-wasm,
 * cpk/shared, lib/cutin/*) — aucun `server-only`/`bun:sqlite`.
 */
import { Box, Download, Film, Loader2, Pause, Play, RotateCcw, Sparkles } from "lucide-react";
import dynamic from "next/dynamic";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import CharacterModelViewer from "@/components/wiki/CharacterModelViewer";
import { cpkAssetUrl } from "@rosegriffon/azalee/cpk/shared";
import { downloadName } from "@rosegriffon/azalee/text/download-filename";
import type { CutinStoryboard } from "@/lib/cutin/camera";
import { decodeEffectLayer, loadCutinStoryboard, mainModelLayer } from "@/lib/cutin/decode";
import {
	downloadBlob,
	exportCutinVideo,
	isVideoExportSupported,
} from "@/lib/cutin/export";
import { fetchCutinManifest } from "@/lib/cutin/listing";
import { createTheatreSequencer } from "@/lib/cutin/theatre-sequencer";
import type { CutinGlbLayer, CutinManifest } from "@/lib/cutin/types";
import type { SkillCutin } from "@rosegriffon/azalee/game/skills-cutin";
import type { CutinSceneHandle } from "./CutinScene";

const CutinScene = dynamic(() => import("@/components/wiki/cutin/CutinScene"), {
	ssr: false,
	loading: () => <SceneSkeleton />,
});

export interface CutinViewerProps {
	/** Entrée cut-in (déjà disponible dans `SkillCutinSection`). */
	cutin: SkillCutin;
	skillName: string;
}

/** Squelette pendant le chargement du chunk R3F. */
function SceneSkeleton(): React.JSX.Element {
	return (
		<div className="aspect-square w-full rounded-xl bg-surface-container-high flex items-center justify-center">
			<Loader2 className="size-8 animate-spin text-primary" />
		</div>
	);
}

/** Statut de décodage d'une couche d'effet. */
type LayerStatus = "idle" | "loading" | "ready" | "none";

/** Attend la prochaine peinture (laisse React appliquer scrubSec=null/orbit off avant l'export). */
function nextPaint(): Promise<void> {
	return new Promise((resolve) => {
		requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
	});
}

export function CutinViewer({ cutin, skillName }: CutinViewerProps): React.JSX.Element {
	const [mounted, setMounted] = useState(false);
	const [manifest, setManifest] = useState<CutinManifest | null>(null);
	const [storyboard, setStoryboard] = useState<CutinStoryboard | null>(null);
	const [mainLayer, setMainLayer] = useState<CutinGlbLayer | null>(null);
	const [loadError, setLoadError] = useState(false);

	// Couches d'effet décodées (id → GLB) + leur statut.
	const [decoded, setDecoded] = useState<Record<string, CutinGlbLayer>>({});
	const [status, setStatus] = useState<Record<string, LayerStatus>>({});
	const [visibility, setVisibility] = useState<Record<string, boolean>>({ main: true });

	// Contrôles de scène.
	const [bloom, setBloom] = useState(true);
	const [orbit, setOrbit] = useState(false);
	const [playing, setPlaying] = useState(true);
	const [scrubSec, setScrubSec] = useState<number | null>(null);
	const [displayPos, setDisplayPos] = useState(0);
	const [shotName, setShotName] = useState("");

	// Export.
	const [exporting, setExporting] = useState(false);
	const [exportPct, setExportPct] = useState(0);
	const canExport = useMemo(() => isVideoExportSupported(), []);

	const handleRef = useRef<CutinSceneHandle | null>(null);
	const blobUrlsRef = useRef<string[]>([]);

	const modelUrl = `https://cdn.rosegriffon.fr/model-chr/waza/${cutin.event_id_name}.glb`;
	const textureUrl = cpkAssetUrl(cutin.texture_g4tx);

	// Chargement des données une fois la scène montée (AbortController au démontage).
	useEffect(() => {
		if (!mounted) return;
		const ac = new AbortController();
		(async () => {
			try {
				const m = await fetchCutinManifest(cutin, ac.signal);
				const story = await loadCutinStoryboard(m, ac.signal);
				if (ac.signal.aborted) return;
				setManifest(m);
				setStoryboard(story);
				setMainLayer(mainModelLayer(m));
				// Effets : tous « idle » (décodables à la demande), invisibles par défaut.
				setStatus(Object.fromEntries(m.layers.map((l) => [l.id, "idle" as LayerStatus])));
			} catch (err) {
				if (err instanceof DOMException && err.name === "AbortError") return;
				setLoadError(true);
			}
		})();
		return () => ac.abort();
	}, [mounted, cutin]);

	// Révoque les blob URLs créés (couches d'effet) au démontage.
	useEffect(() => {
		const urls = blobUrlsRef.current;
		return () => {
			for (const u of urls) URL.revokeObjectURL(u);
		};
	}, []);

	// Slider/overlay : suit la position de la timeline pendant la lecture.
	useEffect(() => {
		if (!playing || scrubSec != null) return;
		let raf = 0;
		const tick = () => {
			const h = handleRef.current;
			const dur = storyboard?.duration ?? 0;
			if (h && dur > 0) setDisplayPos(h.sequence.position % dur);
			raf = requestAnimationFrame(tick);
		};
		raf = requestAnimationFrame(tick);
		return () => cancelAnimationFrame(raf);
	}, [playing, scrubSec, storyboard]);

	const onReady = useCallback((h: CutinSceneHandle) => {
		handleRef.current = h;
	}, []);

	// Couches GLB montées dans la scène : waza + effets décodés.
	const glbLayers = useMemo<CutinGlbLayer[]>(() => {
		const arr: CutinGlbLayer[] = [];
		if (mainLayer) arr.push(mainLayer);
		for (const l of Object.values(decoded)) arr.push(l);
		return arr;
	}, [mainLayer, decoded]);

	// Toggle d'une couche : décode à la demande (1er clic d'un effet renderable).
	const toggleLayer = useCallback(
		async (id: string) => {
			if (id === "main") {
				setVisibility((v) => ({ ...v, main: v.main === false }));
				return;
			}
			const st = status[id] ?? "idle";
			if (st === "ready") {
				setVisibility((v) => ({ ...v, [id]: v[id] === false }));
				return;
			}
			if (st !== "idle") return; // loading / none → no-op
			const layerDesc = manifest?.layers.find((l) => l.id === id);
			if (!layerDesc) return;
			setStatus((s) => ({ ...s, [id]: "loading" }));
			try {
				const glb = await decodeEffectLayer(layerDesc);
				if (glb) {
					blobUrlsRef.current.push(glb.blobUrl);
					setDecoded((d) => ({ ...d, [id]: glb }));
					setStatus((s) => ({ ...s, [id]: "ready" }));
					setVisibility((v) => ({ ...v, [id]: true }));
				} else {
					// Effet pur non-géométrique (G4MT/G4MA/G4VS) : pas de mesh à monter.
					setStatus((s) => ({ ...s, [id]: "none" }));
				}
			} catch {
				setStatus((s) => ({ ...s, [id]: "none" }));
			}
		},
		[status, manifest],
	);

	const resetCamera = useCallback(() => {
		setOrbit(false);
		setScrubSec(null);
		setPlaying(true);
	}, []);

	const onScrub = useCallback((value: number) => {
		setPlaying(false);
		setScrubSec(value);
		setDisplayPos(value);
	}, []);

	const togglePlay = useCallback(() => {
		setPlaying((p) => {
			const next = !p;
			if (next) setScrubSec(null);
			return next;
		});
	}, []);

	const handleExport = useCallback(async () => {
		const h = handleRef.current;
		if (!h || !storyboard || exporting) return;
		setExporting(true);
		setExportPct(0);
		// Fige l'état déterministe (pas d'orbit, pas de scrub manuel, pas de playhead auto).
		setOrbit(false);
		setPlaying(false);
		setScrubSec(null);
		await nextPaint();
		h.setFrameloop("never");
		try {
			const renderAt = createTheatreSequencer({ sequence: h.sequence, r3f: h });
			const blob = await exportCutinVideo({
				canvas: h.gl.domElement,
				durationSec: storyboard.duration,
				fps: storyboard.fps,
				renderAt,
				onProgress: (done, total) => setExportPct(done / total),
			});
			downloadBlob(blob, `${downloadName(skillName, "cutin")}_cutin`);
		} catch {
			/* échec d'encodage : on retombe sur l'état interactif sans bloquer l'UI */
		} finally {
			h.setFrameloop("always");
			setExporting(false);
			setPlaying(true);
		}
	}, [storyboard, exporting, skillName]);

	// --- Repli garanti si les données échouent (model-viewer mono-modèle). ---
	if (loadError) {
		return <CharacterModelViewer glbUrl={modelUrl} name={skillName} inline />;
	}

	// --- Avant montage : vignette + bouton de chargement (lazy). ---
	if (!mounted) {
		return (
			<button
				type="button"
				onClick={() => setMounted(true)}
				className="group relative aspect-square w-full overflow-hidden rounded-xl bg-surface-container-high"
			>
				{textureUrl ? (
					// eslint-disable-next-line @next/next/no-img-element
					<img
						src={textureUrl}
						alt={`Aperçu ${skillName}`}
						loading="lazy"
						className="absolute inset-0 size-full object-cover opacity-30 blur-sm [image-rendering:pixelated]"
					/>
				) : null}
				<span className="absolute inset-0 flex flex-col items-center justify-center gap-2 text-on-surface">
					<Sparkles className="size-9 text-primary transition-transform group-hover:scale-110" />
					<span className="text-sm font-semibold">Charger la scène 3D</span>
					<span className="text-xs text-on-surface-variant">modèle · timeline · export MP4</span>
				</span>
			</button>
		);
	}

	const duration = storyboard?.duration ?? 0;
	const fps = storyboard?.fps ?? 60;

	// Toggles affichés : waza principal + couches d'effet de l'event.
	const toggles = [
		{ id: "main", label: "main", available: true, status: "ready" as LayerStatus },
		...(manifest?.layers ?? []).map((l) => ({
			id: l.id,
			label: l.plan ? `${l.subject} · ${l.plan}` : l.subject,
			available: (status[l.id] ?? "idle") !== "none",
			status: status[l.id] ?? "idle",
		})),
	];

	return (
		<div className="space-y-2">
			{/* Canvas R3F (composé). */}
			<div className="relative aspect-square w-full overflow-hidden rounded-xl bg-surface-container-high">
				{storyboard && manifest ? (
					<CutinScene
						manifest={manifest}
						storyboard={storyboard}
						glbLayers={glbLayers}
						visibility={visibility}
						bloom={bloom}
						orbit={orbit && !exporting}
						playing={playing}
						scrubSec={scrubSec}
						onShot={setShotName}
						onReady={onReady}
					/>
				) : (
					<SceneSkeleton />
				)}
				{/* Overlay plan courant. */}
				{shotName && (
					<span className="pointer-events-none absolute left-2 top-2 rounded bg-surface/60 px-2 py-0.5 text-xs font-mono text-on-surface-variant backdrop-blur">
						{shotName}
					</span>
				)}
				{/* Barre de progression d'export. */}
				{exporting && (
					<div className="absolute inset-x-0 bottom-0 h-1.5 bg-surface/40">
						<div
							className="h-full bg-primary transition-[width]"
							style={{ width: `${Math.round(exportPct * 100)}%` }}
						/>
					</div>
				)}
			</div>

			{/* Timeline : play/pause + scrub. */}
			<div className="flex items-center gap-2">
				<button
					type="button"
					onClick={togglePlay}
					className="inline-flex size-8 items-center justify-center rounded-full bg-surface-container-high text-on-surface hover:bg-surface-container-highest"
					title={playing ? "Pause" : "Lecture"}
				>
					{playing ? <Pause className="size-4" /> : <Play className="size-4" />}
				</button>
				<input
					type="range"
					min={0}
					max={duration || 1}
					step={1 / fps}
					value={scrubSec ?? displayPos}
					onChange={(e) => onScrub(Number(e.target.value))}
					className="h-1 flex-1 accent-primary"
					aria-label="Position de la timeline"
				/>
				<span className="w-12 text-right font-mono text-xs text-on-surface-variant">
					{(scrubSec ?? displayPos).toFixed(1)}s
				</span>
			</div>

			{/* Toggles de couches. */}
			<div className="flex flex-wrap gap-1.5">
				{toggles.map((t) => {
					const visible = t.id === "main" ? visibility.main !== false : visibility[t.id] === true;
					const isLoading = t.status === "loading";
					return (
						<button
							key={t.id}
							type="button"
							disabled={!t.available || isLoading}
							onClick={() => void toggleLayer(t.id)}
							title={t.available ? t.label : `${t.label} — effet (non géométrique)`}
							className={`inline-flex items-center gap-1 rounded-full px-2.5 py-1 text-xs font-medium transition-colors ${
								visible
									? "bg-primary text-on-primary"
									: "bg-surface-container-high text-on-surface-variant hover:bg-surface-container-highest"
							} ${!t.available ? "opacity-40" : ""}`}
						>
							{isLoading && <Loader2 className="size-3 animate-spin" />}
							{t.label}
							{!t.available && " · effet"}
						</button>
					);
				})}
			</div>

			{/* Actions : bloom, reset caméra, export. */}
			<div className="flex flex-wrap items-center gap-1.5">
				<button
					type="button"
					onClick={() => setBloom((b) => !b)}
					className={`inline-flex items-center gap-1 rounded-full px-2.5 py-1 text-xs font-medium ${
						bloom
							? "bg-primary text-on-primary"
							: "bg-surface-container-high text-on-surface-variant hover:bg-surface-container-highest"
					}`}
				>
					<Sparkles className="size-3" /> Bloom
				</button>
				<button
					type="button"
					onClick={() => setOrbit((o) => !o)}
					className={`inline-flex items-center gap-1 rounded-full px-2.5 py-1 text-xs font-medium ${
						orbit
							? "bg-primary text-on-primary"
							: "bg-surface-container-high text-on-surface-variant hover:bg-surface-container-highest"
					}`}
					title="Caméra libre (orbit)"
				>
					<Box className="size-3" /> Orbit
				</button>
				<button
					type="button"
					onClick={resetCamera}
					className="inline-flex items-center gap-1 rounded-full bg-surface-container-high px-2.5 py-1 text-xs font-medium text-on-surface-variant hover:bg-surface-container-highest"
					title="Réinitialiser la caméra"
				>
					<RotateCcw className="size-3" /> Reset
				</button>
				<a
					href={modelUrl}
					download={`${downloadName(skillName, "cutin")}.glb`}
					className="inline-flex items-center gap-1 rounded-full bg-surface-container-high px-2.5 py-1 text-xs font-medium text-on-surface-variant hover:bg-surface-container-highest"
					title="Télécharger le modèle waza (GLB)"
				>
					<Download className="size-3" /> GLB
				</a>
				{canExport && (
					<button
						type="button"
						onClick={() => void handleExport()}
						disabled={exporting}
						className="inline-flex items-center gap-1 rounded-full bg-primary px-2.5 py-1 text-xs font-medium text-on-primary hover:opacity-90 disabled:opacity-50"
						title="Exporter le cut-in en MP4 (muet)"
					>
						{exporting ? <Loader2 className="size-3 animate-spin" /> : <Film className="size-3" />}
						{exporting ? `Export ${Math.round(exportPct * 100)}%` : "Exporter MP4"}
					</button>
				)}
			</div>
		</div>
	);
}
