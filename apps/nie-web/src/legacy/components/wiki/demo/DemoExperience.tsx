"use client";

/**
 * ORCHESTRATEUR de la démo 3D (`/demo`). PIÈCE CENTRALE = une scène 3D COMPOSÉE qui montre le
 * JOUEUR (`model-full`) ET l'EFFET 3D de sa super-technique (`waza`) réunis dans la MÊME scène
 * (le personnage entouré de son hissatsu), avec caméra cinématique, telop, Bloom, voix, et un
 * bouton « Générer la vidéo » qui exporte la scène en MP4.
 *
 * CONTRAINTE (vérifiée dans `CutinScene.fitObject`) : ce composant-là normalise CHAQUE GLB à ~2 u
 * INDIVIDUELLEMENT → impossible d'y composer joueur+effet. La scène centrale utilise donc le
 * NOUVEAU `ComposedHissatsuScene` à PLACEMENT EXPLICITE (joueur pieds au sol, effet à l'échelle
 * relative autour du torse). Le reste de la démo (formes/stats/voix/textures) est réutilisé tel quel.
 *
 * Frontière stricte : que des libs PURES/client-safe ; la scène R3F est chargée via
 * `next/dynamic { ssr:false }` et montée À LA DEMANDE (lazy).
 */
import { Box, Download, Film, Loader2, Pause, Play, RotateCcw, Sparkles } from "lucide-react";
import dynamic from "next/dynamic";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { defaultCutinShots, type CutinStoryboard } from "@/lib/cutin/camera";
import {
	downloadBlob,
	exportCutinVideo,
	isVideoExportSupported,
	type CutinAudioSource,
} from "@/lib/cutin/export";
import { fetchCutinManifest } from "@/lib/cutin/listing";
import { loadCutinStoryboard } from "@/lib/cutin/decode";
import { createTheatreSequencer } from "@/lib/cutin/theatre-sequencer";
import { getSkillCutin } from "@rosegriffon/azalee/game/skills-cutin";
import type { CutinSceneHandle } from "./ComposedHissatsuScene";
import { DemoModelSelector } from "./DemoModelSelector";
import { DemoSkillsSection } from "./DemoSkillsSection";
import { DemoStatsPanel } from "./DemoStatsPanel";
import { DemoTextureGallery } from "./DemoTextureGallery";
import { DemoVoicePlayer } from "./DemoVoicePlayer";
import type { DemoData, DemoModelOption, DemoSkill } from "./types";

// La scène composée n'est chargée QUE côté client (chunk WebGL/wasm lourd) — jamais en SSR.
const ComposedHissatsuScene = dynamic(() => import("./ComposedHissatsuScene"), {
	ssr: false,
	loading: () => <SceneSkeleton />,
});

/** Squelette pendant le chargement du chunk R3F. */
function SceneSkeleton(): React.JSX.Element {
	return (
		<div className="flex aspect-square w-full items-center justify-center rounded-xl bg-surface-container-high">
			<Loader2 className="size-8 animate-spin text-primary" />
		</div>
	);
}

/** Attend la prochaine peinture (laisse React appliquer scrub/orbit off avant l'export). */
function nextPaint(): Promise<void> {
	return new Promise((resolve) => {
		requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
	});
}

export function DemoExperience({ data }: { data: DemoData }): React.JSX.Element {
	// --- Registre d'options : formes (mesh normal) + cut-ins 3D servis (mesh émissif). ---
	const options = useMemo<DemoModelOption[]>(() => {
		const forms: DemoModelOption[] = data.forms.map((f) => ({
			id: `form:${f.code}`,
			label: f.label,
			kind: "form",
			code: f.code,
			glbUrl: f.modelGlbUrl,
			fromEffect: false,
			voiceUrl: f.voiceUrl,
			iconUrl: f.iconUrl,
			eventIdName: null,
		}));
		const wazas: DemoModelOption[] = data.skills
			.filter((s) => s.has3d)
			.map((s) => ({
				id: `waza:${s.eventIdName}`,
				label: `${s.elementName.fr} ${s.categoryName.fr}`,
				kind: "waza",
				code: s.eventIdName,
				glbUrl: s.wazaGlbUrl,
				fromEffect: true,
				voiceUrl: null,
				iconUrl: s.textureUrl,
				eventIdName: s.eventIdName,
			}));
		return [...forms, ...wazas];
	}, [data.forms, data.skills]);

	const optionById = useMemo(() => new Map(options.map((o) => [o.id, o])), [options]);

	// --- État scène : DEUX sélections indépendantes (joueur + technique) composées ensemble. ---
	const [mounted, setMounted] = useState(false);
	const [playerCode, setPlayerCode] = useState<string>(data.forms[0]?.code ?? "c01001900");
	const [skillEvent, setSkillEvent] = useState<string>(
		data.skills.find((s) => s.eventIdName === "ev60_00340" && s.has3d)?.eventIdName ??
			data.skills.find((s) => s.has3d)?.eventIdName ??
			"",
	);
	// `selectedId` ne sert qu'au surlignage du sélecteur (dernière vignette cliquée).
	const [selectedId, setSelectedId] = useState<string>(`form:${data.forms[0]?.code ?? ""}`);
	const [storyboard, setStoryboard] = useState<CutinStoryboard | null>(null);
	const [activeTelop, setActiveTelop] = useState<string | null>(null);

	const [bloom, setBloom] = useState(true);
	const [orbit, setOrbit] = useState(false);
	const [playing, setPlaying] = useState(true);
	const [scrubSec, setScrubSec] = useState<number | null>(null);
	const [displayPos, setDisplayPos] = useState(0);
	const [shotName, setShotName] = useState("");

	const [exporting, setExporting] = useState(false);
	const [exportPct, setExportPct] = useState(0);
	const canExport = useMemo(() => isVideoExportSupported(), []);

	const handleRef = useRef<CutinSceneHandle | null>(null);

	// --- Dérivés : forme (joueur) + technique (effet) couramment composés. ---
	const currentForm = useMemo(
		() => data.forms.find((f) => f.code === playerCode) ?? data.forms[0],
		[data.forms, playerCode],
	);
	const currentSkill = useMemo(
		() => data.skills.find((s) => s.eventIdName === skillEvent) ?? null,
		[data.skills, skillEvent],
	);
	const playerUrl = currentForm?.modelGlbUrl ?? "";
	const effectUrl = currentSkill?.wazaGlbUrl ?? "";

	// --- Chargement du storyboard caméra (clé = technique courante). ---
	useEffect(() => {
		if (!mounted) return;
		const skill = data.skills.find((s) => s.eventIdName === skillEvent) ?? null;
		if (!skill) {
			setStoryboard(defaultCutinShots());
			setActiveTelop(null);
			return;
		}
		const ac = new AbortController();
		(async () => {
			try {
				const cutin = getSkillCutin(skill.skillIdStr)?.cutin ?? null;
				let story: CutinStoryboard;
				if (cutin) {
					// On ne lit le manifeste QUE pour fournir le storyboard caméra (g4cm) — les g4pk
					// ne sont PAS décodés (la géométrie de l'effet vient du GLB waza assemblé).
					const m = await fetchCutinManifest(cutin, ac.signal);
					story = await loadCutinStoryboard(m, ac.signal);
				} else {
					story = defaultCutinShots();
				}
				if (ac.signal.aborted) return;
				setStoryboard(story);
				setActiveTelop(skill.telopUrl ?? null);
				setPlaying(true);
				setScrubSec(null);
			} catch (err) {
				if (err instanceof DOMException && err.name === "AbortError") return;
				setStoryboard(defaultCutinShots());
				setActiveTelop(skill.telopUrl ?? null);
			}
		})();
		return () => ac.abort();
	}, [mounted, skillEvent, data.skills]);

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

	// Sélection d'une vignette : forme → change le JOUEUR, waza → change la TECHNIQUE composée.
	const onSelect = useCallback(
		(id: string) => {
			setMounted(true);
			setSelectedId(id);
			const o = optionById.get(id);
			if (!o) return;
			if (o.kind === "form") setPlayerCode(o.code);
			else setSkillEvent(o.code);
		},
		[optionById],
	);

	// Lancement d'une technique depuis la section Techniques → change l'effet composé dans la scène.
	const launchCutin = useCallback((skill: DemoSkill) => {
		setMounted(true);
		setSkillEvent(skill.eventIdName);
		setSelectedId(`waza:${skill.eventIdName}`);
		// Remonte vers la scène pour voir le résultat.
		if (typeof window !== "undefined") window.scrollTo({ top: 0, behavior: "smooth" });
	}, []);

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

	// Génération de la vidéo MP4 de la scène composée + piste audio = voix de la forme courante.
	const handleExport = useCallback(async () => {
		const h = handleRef.current;
		if (!h || !storyboard || exporting) return;
		setExporting(true);
		setExportPct(0);
		setOrbit(false);
		setPlaying(false);
		setScrubSec(null);
		await nextPaint();
		h.setFrameloop("never");
		try {
			let audio: CutinAudioSource | undefined;
			const voiceUrl = currentForm?.voiceUrl;
			if (voiceUrl) {
				const r = await fetch(voiceUrl);
				if (r.ok) audio = { wav: new Uint8Array(await r.arrayBuffer()) }; // `/audio` renvoie du WAV
			}
			const renderAt = createTheatreSequencer({ sequence: h.sequence, r3f: h });
			const blob = await exportCutinVideo({
				canvas: h.gl.domElement,
				durationSec: storyboard.duration,
				fps: storyboard.fps,
				renderAt,
				audio,
				onProgress: (done, total) => setExportPct(done / total),
			});
			downloadBlob(blob, `aphrodi_${currentSkill?.eventIdName ?? "demo"}`);
		} catch {
			/* échec d'encodage : on retombe sur l'état interactif sans bloquer l'UI */
		} finally {
			h.setFrameloop("always");
			setExporting(false);
			setPlaying(true);
		}
	}, [storyboard, exporting, currentForm, currentSkill]);

	const duration = storyboard?.duration ?? 0;
	const fps = storyboard?.fps ?? 60;
	const poster = data.forms[0]?.iconUrl ?? data.forms[0]?.faceTextureUrl ?? null;

	return (
		<div className="space-y-6">
			<div className="grid grid-cols-1 gap-6 lg:grid-cols-2">
				{/* Colonne scène. */}
				<div className="space-y-2">
					<div className="relative aspect-square w-full overflow-hidden rounded-xl bg-surface-container-high">
						{!mounted ? (
							<button
								type="button"
								onClick={() => setMounted(true)}
								className="group absolute inset-0 size-full"
							>
								{poster && (
									// eslint-disable-next-line @next/next/no-img-element
									<img
										src={poster}
										alt="Aperçu"
										className="absolute inset-0 size-full object-cover opacity-30 blur-sm [image-rendering:pixelated]"
									/>
								)}
								<span className="absolute inset-0 flex flex-col items-center justify-center gap-2 text-on-surface">
									<Sparkles className="size-10 text-primary transition-transform group-hover:scale-110" />
									<span className="text-sm font-semibold">Charger la scène 3D</span>
									<span className="text-xs text-on-surface-variant">
										joueur + super-technique · caméra · export MP4
									</span>
								</span>
							</button>
						) : storyboard && playerUrl && effectUrl ? (
							<ComposedHissatsuScene
								playerUrl={playerUrl}
								effectUrl={effectUrl}
								storyboard={storyboard}
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
						{mounted && shotName && (
							<span className="pointer-events-none absolute left-2 top-2 rounded bg-surface/60 px-2 py-0.5 font-mono text-xs text-on-surface-variant backdrop-blur">
								{shotName}
							</span>
						)}
						{/* Telop du cut-in courant. */}
						{mounted && activeTelop && (
							// eslint-disable-next-line @next/next/no-img-element
							<img
								src={activeTelop}
								alt="Telop"
								className="pointer-events-none absolute inset-x-3 bottom-3 mx-auto aspect-[24/5] w-3/4 object-contain drop-shadow"
							/>
						)}
						{/* Progression d'export. */}
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
					{mounted && (
						<>
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

							{/* Actions : bloom, orbit, reset, download GLB, export. */}
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
								{currentForm && (
									<a
										href={currentForm.modelGlbUrl}
										download={`${currentForm.code}.glb`}
										className="inline-flex items-center gap-1 rounded-full bg-surface-container-high px-2.5 py-1 text-xs font-medium text-on-surface-variant hover:bg-surface-container-highest"
										title="Télécharger le modèle du joueur (GLB)"
									>
										<Download className="size-3" /> Joueur
									</a>
								)}
								{currentSkill && (
									<a
										href={currentSkill.wazaGlbUrl}
										download={`${currentSkill.eventIdName}.glb`}
										className="inline-flex items-center gap-1 rounded-full bg-surface-container-high px-2.5 py-1 text-xs font-medium text-on-surface-variant hover:bg-surface-container-highest"
										title="Télécharger l'effet de la technique (GLB)"
									>
										<Download className="size-3" /> Effet
									</a>
								)}
								{canExport && (
									<button
										type="button"
										onClick={() => void handleExport()}
										disabled={exporting}
										className="inline-flex items-center gap-1 rounded-full bg-primary px-2.5 py-1 text-xs font-medium text-on-primary hover:opacity-90 disabled:opacity-50"
										title="Générer la vidéo MP4 de la scène (avec voix)"
									>
										{exporting ? (
											<Loader2 className="size-3 animate-spin" />
										) : (
											<Film className="size-3" />
										)}
										{exporting
											? `Vidéo ${Math.round(exportPct * 100)}%`
											: "Générer la vidéo"}
									</button>
								)}
							</div>
						</>
					)}
				</div>

				{/* Colonne sélecteur + voix. */}
				<div className="space-y-4">
					<div className="space-y-2">
						<h2 className="text-sm font-bold text-on-surface">
							Modèles ({options.length}) — joueur + technique
						</h2>
						<DemoModelSelector options={options} selectedId={selectedId} onSelect={onSelect} />
						<p className="text-[11px] text-on-surface-variant/70">
							Joueur + sa super-technique composés dans la même scène : une vignette « forme »
							change le joueur, une vignette « cut-in » change l'effet autour de lui.
						</p>
					</div>
					<DemoVoicePlayer forms={data.forms} />
				</div>
			</div>

			{/* Statistiques (moteur niers). */}
			<DemoStatsPanel forms={data.forms} precomputed={data.precomputedStats} />

			{/* Techniques (changent l'effet composé dans la scène). */}
			<DemoSkillsSection
				skills={data.skills}
				activeEventId={skillEvent || null}
				onPlayCutin={launchCutin}
			/>

			{/* Textures téléchargeables. */}
			<DemoTextureGallery textures={data.textures} />
		</div>
	);
}
