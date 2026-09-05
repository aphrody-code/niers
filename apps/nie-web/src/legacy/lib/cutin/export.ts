/**
 * Export VIDÉO MP4 d'un cut-in (hissatsu) rendu dans un canvas R3F, **dans le navigateur**.
 *
 * Pipeline 100 % client, sans dépendance serveur ni encodage cloud :
 *   1. rendu **déterministe frame-par-frame** : on pilote une position de timeline (Theatre.js
 *      `sequence.position`, cf. {@link createTheatreSequencer}) puis on rend la scène three.js
 *      manuellement (`gl.render(scene, camera)`) — aucune dépendance au `requestAnimationFrame`,
 *      donc reproductible quel que soit le débit machine ;
 *   2. capture : chaque frame du canvas WebGL devient un `VideoFrame(canvas, { timestamp })`
 *      (WebCodecs, API navigateur native — aucune dep npm) ;
 *   3. encodage : `VideoEncoder` (H.264/AVC) produit des `EncodedVideoChunk` ;
 *   4. muxing : `mp4-muxer` (`Muxer` + `ArrayBufferTarget`, `addVideoChunk`) assemble le MP4 ;
 *   5. piste audio optionnelle : un WAV PCM (décodé in-browser par `audioToWav`, cf.
 *      `lib/cpk-wasm.ts`) est ré-encodé en AAC (`AudioEncoder`) et muxé via `addAudioChunk` ;
 *   6. sortie : `ArrayBufferTarget.buffer` → `Blob` MP4 téléchargeable.
 *
 * Client-safe : aucun import `server-only`/`bun:sqlite`/Node. Utilisable depuis un îlot
 * `"use client"` (le canvas R3F doit lui-même être chargé via `next/dynamic { ssr:false }`).
 *
 * (Module `lib/cutin/export.ts` — cohésion du dossier `lib/cutin/*` ; anciennement
 * `lib/cutin/video-export.ts`, contenu inchangé.)
 *
 * CSP (cf. `next.config.ts` `cspHeader`) : rien à ajouter — WebCodecs tourne sur le thread
 * principal (pas de Worker), et le Blob de sortie est consommé via `blob:` déjà autorisé
 * (`media-src 'self' blob:`, `connect-src 'self' blob:`).
 */

import { ArrayBufferTarget, Muxer } from "mp4-muxer";

/** Disponibilité runtime de l'API WebCodecs (à tester avant d'exposer le bouton d'export). */
export function isVideoExportSupported(): boolean {
	return (
		typeof window !== "undefined" &&
		typeof window.VideoEncoder === "function" &&
		typeof window.VideoFrame === "function"
	);
}

/** Source audio optionnelle de l'export : un WAV PCM (sortie de `audioToWav`, `lib/cpk-wasm`). */
export interface CutinAudioSource {
	/** Octets WAV (PCM 8/16/24/32-bit ou float) — typiquement `await audioToWav(rawBytes)`. */
	wav: Uint8Array;
	/**
	 * Décalage (s) du début de l'audio par rapport au début de la vidéo. Permet de caler le SON
	 * du cut-in sur la phase voulue de la timeline. Défaut `0`.
	 */
	startSec?: number;
}

/** Options de l'export vidéo d'un cut-in. */
export interface CutinExportOptions {
	/** Le canvas WebGL de la scène R3F (`gl.domElement`). Source des `VideoFrame`. */
	canvas: HTMLCanvasElement;
	/** Durée totale de la séquence à exporter, en secondes (= `sequence.length` Theatre.js). */
	durationSec: number;
	/** Images par seconde. Défaut `30`. Le rendu génère exactement `round(durationSec*fps)` frames. */
	fps?: number;
	/**
	 * Rendu DÉTERMINISTE d'une frame à l'instant `timeSec` : doit (1) positionner la timeline
	 * (`sequence.position = timeSec`), (2) appliquer les mutations dépendantes du temps, (3) rendre
	 * la scène dans le canvas (`gl.render(scene, camera)`). Peut être async (ex. await d'une frame
	 * de matériau/texture). Voir {@link createTheatreSequencer} pour une implémentation clé-en-main.
	 */
	renderAt: (timeSec: number) => void | Promise<void>;
	/** Débit cible de la vidéo en bits/s. Défaut : auto selon la résolution (≈ 0,1 bit/pixel/frame). */
	bitrate?: number;
	/** Intervalle (en frames) entre deux keyframes H.264. Défaut `fps*2` (une toutes les ~2 s). */
	keyFrameInterval?: number;
	/** Piste audio optionnelle (WAV PCM → AAC). */
	audio?: CutinAudioSource;
	/** Progression de l'encodage : `(framesRendues, framesTotales)`. */
	onProgress?: (done: number, total: number) => void;
	/** Annulation coopérative (libère les encodeurs et jette `AbortError`). */
	signal?: AbortSignal;
}

/** Calcule une chaîne de codec AVC (`avc1.PPCCLL`) valide pour la résolution/fps donnés. */
function avcCodecString(width: number, height: number, fps: number): string {
	// Profil High (0x64). Niveau dérivé du nombre de macroblocs/s (Annexe A H.264), borné aux
	// paliers usuels 3.1→5.2. La plupart des cut-ins tiennent en 3.1/4.0.
	const mbPerSec = Math.ceil(width / 16) * Math.ceil(height / 16) * fps;
	let level = 0x1f; // 3.1
	if (mbPerSec > 245_760) level = 0x20; // 3.2
	if (mbPerSec > 522_240) level = 0x28; // 4.0
	if (mbPerSec > 589_824) level = 0x29; // 4.1
	if (mbPerSec > 983_040) level = 0x2a; // 4.2
	if (mbPerSec > 2_073_600) level = 0x32; // 5.0
	if (mbPerSec > 4_177_920) level = 0x34; // 5.2
	return `avc1.6400${level.toString(16).padStart(2, "0")}`;
}

/** WAV PCM décodé en canaux float32 planaires (in-browser, sans `AudioContext`). */
interface DecodedWav {
	sampleRate: number;
	channels: Float32Array[];
	frames: number;
}

/** Parse un WAV (RIFF/WAVE) PCM int 8/16/24/32 ou IEEE float 32 → float32 planaire. */
function decodeWav(wav: Uint8Array): DecodedWav {
	const view = new DataView(wav.buffer, wav.byteOffset, wav.byteLength);
	if (view.getUint32(0, false) !== 0x52494646 /* "RIFF" */ || view.getUint32(8, false) !== 0x57415645 /* "WAVE" */) {
		throw new Error("WAV invalide (RIFF/WAVE attendu)");
	}
	let fmt: { audioFormat: number; channels: number; sampleRate: number; bits: number } | null = null;
	let dataOffset = 0;
	let dataLength = 0;
	// Parcours des sous-chunks (fmt , data, …) — tolère LIST/fact entre les deux.
	let p = 12;
	while (p + 8 <= wav.byteLength) {
		const id = view.getUint32(p, false);
		const size = view.getUint32(p + 4, true);
		const body = p + 8;
		if (id === 0x666d7420 /* "fmt " */) {
			fmt = {
				audioFormat: view.getUint16(body, true),
				channels: view.getUint16(body + 2, true),
				sampleRate: view.getUint32(body + 4, true),
				bits: view.getUint16(body + 14, true),
			};
		} else if (id === 0x64617461 /* "data" */) {
			dataOffset = body;
			dataLength = size;
		}
		p = body + size + (size & 1); // padding sur taille impaire
	}
	if (!fmt || dataLength === 0) {
		throw new Error("WAV sans chunk fmt/data exploitable");
	}

	const { channels, sampleRate, bits, audioFormat } = fmt;
	const bytesPerSample = bits >> 3;
	const frameSize = bytesPerSample * channels;
	const frames = Math.floor(dataLength / frameSize);
	const out: Float32Array[] = Array.from({ length: channels }, () => new Float32Array(frames));
	const isFloat = audioFormat === 3;

	for (let i = 0; i < frames; i++) {
		for (let c = 0; c < channels; c++) {
			const off = dataOffset + i * frameSize + c * bytesPerSample;
			let sample = 0;
			if (isFloat && bits === 32) {
				sample = view.getFloat32(off, true);
			} else if (bits === 16) {
				sample = view.getInt16(off, true) / 0x8000;
			} else if (bits === 24) {
				const b0 = view.getUint8(off);
				const b1 = view.getUint8(off + 1);
				const b2 = view.getInt8(off + 2);
				sample = ((b2 << 16) | (b1 << 8) | b0) / 0x800000;
			} else if (bits === 32) {
				sample = view.getInt32(off, true) / 0x80000000;
			} else if (bits === 8) {
				sample = (view.getUint8(off) - 128) / 128; // PCM 8-bit = non signé
			}
			out[c]![i] = sample;
		}
	}
	return { sampleRate, channels: out, frames };
}

/** Jette une `AbortError` si le signal est déjà déclenché (helper de coopération). */
function throwIfAborted(signal?: AbortSignal): void {
	if (signal?.aborted) {
		throw new DOMException("Export annulé", "AbortError");
	}
}

/** Laisse respirer le thread principal entre deux frames (évite le freeze UI). */
function nextTick(): Promise<void> {
	return new Promise((resolve) => setTimeout(resolve, 0));
}

/**
 * Encode la piste audio (WAV PCM → AAC) et la muxe dans le MP4. AAC-LC, frames de 1024 samples.
 * Renvoie la config audio du muxer (pour déclarer la piste) — appelée AVANT le muxing vidéo.
 */
async function encodeAudio(
	muxer: Muxer<ArrayBufferTarget>,
	source: CutinAudioSource,
	durationSec: number,
	signal?: AbortSignal
): Promise<void> {
	const decoded = decodeWav(source.wav);
	const { sampleRate, channels } = decoded;
	const numberOfChannels = channels.length;
	const startUs = Math.round((source.startSec ?? 0) * 1_000_000);
	// On ne dépasse pas la durée vidéo : un cut-in audio plus long que la timeline est tronqué.
	const maxFrames = Math.min(decoded.frames, Math.round((durationSec - (source.startSec ?? 0)) * sampleRate));
	if (maxFrames <= 0) {
		return;
	}

	const errors: Error[] = [];
	const encoder = new AudioEncoder({
		output: (chunk, meta) => muxer.addAudioChunk(chunk, meta),
		error: (e) => errors.push(e),
	});
	encoder.configure({
		codec: "mp4a.40.2", // AAC-LC
		sampleRate,
		numberOfChannels,
		bitrate: 128_000,
	});

	const BLOCK = 1024; // taille de trame AAC
	const interleaved = new Float32Array(BLOCK * numberOfChannels);
	for (let start = 0; start < maxFrames; start += BLOCK) {
		throwIfAborted(signal);
		const count = Math.min(BLOCK, maxFrames - start);
		// Entrelacement f32 (AudioData "f32" = interleaved).
		for (let i = 0; i < count; i++) {
			for (let c = 0; c < numberOfChannels; c++) {
				interleaved[i * numberOfChannels + c] = channels[c]![start + i]!;
			}
		}
		const data = new AudioData({
			format: "f32",
			sampleRate,
			numberOfFrames: count,
			numberOfChannels,
			timestamp: startUs + Math.round((start / sampleRate) * 1_000_000),
			data: count === BLOCK ? interleaved : interleaved.subarray(0, count * numberOfChannels),
		});
		encoder.encode(data);
		data.close();
		// Backpressure : on évite de saturer la file de l'encodeur audio.
		if (encoder.encodeQueueSize > 16) {
			await nextTick();
		}
	}

	await encoder.flush();
	encoder.close();
	if (errors.length > 0) {
		throw errors[0];
	}
}

/**
 * Exporte un cut-in en **Blob MP4** (H.264 + AAC optionnel), rendu frame-par-frame de façon
 * déterministe. À `await` depuis un handler de bouton ; déclencher ensuite le téléchargement via
 * {@link downloadBlob}.
 *
 * @throws `DOMException("AbortError")` si `signal` est déclenché ; `Error` si WebCodecs indispo,
 *   si la config H.264/AAC n'est pas supportée, ou si l'encodage échoue.
 */
export async function exportCutinVideo(options: CutinExportOptions): Promise<Blob> {
	if (!isVideoExportSupported()) {
		throw new Error("WebCodecs (VideoEncoder/VideoFrame) indisponible dans ce navigateur");
	}
	const {
		canvas,
		durationSec,
		fps = 30,
		renderAt,
		audio,
		onProgress,
		signal,
	} = options;

	// Dimensions paires (H.264 exige des dimensions divisibles par 2).
	const width = canvas.width - (canvas.width % 2);
	const height = canvas.height - (canvas.height % 2);
	if (width === 0 || height === 0) {
		throw new Error("Canvas de taille nulle — le rendu R3F doit être monté et dimensionné");
	}

	const totalFrames = Math.max(1, Math.round(durationSec * fps));
	const frameDurationUs = 1_000_000 / fps;
	const keyFrameInterval = options.keyFrameInterval ?? fps * 2;
	const bitrate = options.bitrate ?? Math.round(width * height * fps * 0.1);

	const target = new ArrayBufferTarget();
	const muxer = new Muxer({
		target,
		fastStart: "in-memory", // metadata en tête → lecture/seek immédiats du MP4 téléchargé
		video: { codec: "avc", width, height, frameRate: fps },
		...(audio
			? {
					audio: (() => {
						const d = decodeWav(audio.wav);
						return { codec: "aac" as const, numberOfChannels: d.channels.length, sampleRate: d.sampleRate };
					})(),
				}
			: {}),
		firstTimestampBehavior: "offset",
	});

	const errors: Error[] = [];
	const encoder = new VideoEncoder({
		output: (chunk, meta) => muxer.addVideoChunk(chunk, meta),
		error: (e) => errors.push(e),
	});
	const config: VideoEncoderConfig = {
		codec: avcCodecString(width, height, fps),
		width,
		height,
		bitrate,
		framerate: fps,
		// Hint navigateur : on privilégie le temps réel n'a pas d'intérêt ici, on veut la qualité.
		latencyMode: "quality",
	};
	// Vérifie le support avant de configurer (certaines combinaisons profil/niveau sont refusées).
	const supportCheck = await VideoEncoder.isConfigSupported(config);
	if (!supportCheck.supported) {
		throw new Error(`Config H.264 non supportée: ${config.codec} ${width}x${height}`);
	}
	encoder.configure(config);

	try {
		// 1) Audio d'abord (indépendant du rendu vidéo, peut s'encoder en parallèle logique).
		if (audio) {
			await encodeAudio(muxer, audio, durationSec, signal);
		}

		// 2) Vidéo : boucle déterministe frame-par-frame.
		for (let i = 0; i < totalFrames; i++) {
			throwIfAborted(signal);
			const timeSec = i / fps;
			// Rendu déterministe : positionne la timeline + rend la scène dans le canvas.
			await renderAt(timeSec);

			const frame = new VideoFrame(canvas, {
				timestamp: Math.round(i * frameDurationUs),
				duration: Math.round(frameDurationUs),
			});
			encoder.encode(frame, { keyFrame: i % keyFrameInterval === 0 });
			frame.close();

			onProgress?.(i + 1, totalFrames);

			// Backpressure : si la file d'encodage gonfle, on attend qu'elle se vide un peu.
			if (encoder.encodeQueueSize > 8) {
				await nextTick();
			}
		}

		await encoder.flush();
		encoder.close();
		if (errors.length > 0) {
			throw errors[0];
		}

		muxer.finalize();
		return new Blob([target.buffer], { type: "video/mp4" });
	} catch (err) {
		// Libère les ressources WebCodecs en cas d'échec/annulation.
		try {
			if (encoder.state !== "closed") {
				encoder.close();
			}
		} catch {
			/* déjà fermé */
		}
		throw err;
	}
}

/** Déclenche le téléchargement d'un Blob MP4 (révoque l'URL objet après le clic). */
export function downloadBlob(blob: Blob, filename: string): void {
	const url = URL.createObjectURL(blob);
	const a = document.createElement("a");
	a.href = url;
	a.download = filename.endsWith(".mp4") ? filename : `${filename}.mp4`;
	document.body.appendChild(a);
	a.click();
	a.remove();
	// Laisse le temps au navigateur de démarrer le download avant de révoquer.
	setTimeout(() => URL.revokeObjectURL(url), 10_000);
}
