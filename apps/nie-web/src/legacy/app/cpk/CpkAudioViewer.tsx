"use client";

/**
 * Lecteur audio CRI décodé **nativement dans le navigateur** : récupère les octets bruts
 * (`/raw`) et les décode en WAV via wasm (`audioToWav` — HCA/ADX + conteneurs AWB/ACB,
 * mêmes parseurs Rust que le jeu), joué via un blob URL. Pour un `.acb` sans AWB embarqué,
 * récupère le `.awb` sibling. Repli sur le décodage serveur (`/audio`) si le wasm échoue.
 */
import { useEffect, useState } from "react";
import { Icon } from "@/components/ui/Icon";
import { cpkAudioUrl, cpkRawUrl } from "@rosegriffon/azalee/cpk/shared";
import { audioToWav } from "@/lib/cpk-wasm";

export function CpkAudioViewer({ path }: { path: string }) {
	const [url, setUrl] = useState<string | null>(null);
	const [failed, setFailed] = useState(false);

	useEffect(() => {
		let cancelled = false;
		let objectUrl: string | null = null;
		setUrl(null);
		setFailed(false);

		(async () => {
			try {
				const res = await fetch(cpkRawUrl(path));
				if (!res.ok) throw new Error(String(res.status));
				const bytes = new Uint8Array(await res.arrayBuffer());
				let wav: Uint8Array;
				try {
					wav = await audioToWav(bytes);
				} catch (e) {
					// `.acb` sans AWB embarqué → l'audio est dans le `.awb` sibling.
					if (path.endsWith(".acb")) {
						const awbPath = `${path.slice(0, -4)}.awb`;
						const awbRes = await fetch(cpkRawUrl(awbPath));
						if (!awbRes.ok) throw e;
						wav = await audioToWav(new Uint8Array(await awbRes.arrayBuffer()));
					} else {
						throw e;
					}
				}
				if (cancelled) return;
				objectUrl = URL.createObjectURL(new Blob([wav.slice()], { type: "audio/wav" }));
				setUrl(objectUrl);
			} catch {
				if (!cancelled) setFailed(true);
			}
		})();

		return () => {
			cancelled = true;
			if (objectUrl) URL.revokeObjectURL(objectUrl);
		};
	}, [path]);

	const src = failed ? cpkAudioUrl(path) : url;

	return (
		<div className="flex flex-col items-center gap-3 p-8 text-center">
			<Icon name="graphic_eq" size={36} className="text-on-surface-variant/50" />
			<p className="text-xs text-on-surface-variant">
				{failed
					? "Audio CRI décodé par le serveur (HCA/ADX → WAV)"
					: "Audio CRI décodé in-browser (wasm, HCA/ADX → WAV)"}
			</p>
			{src ? (
				// eslint-disable-next-line jsx-a11y/media-has-caption
				<audio controls className="w-full max-w-md" src={src} />
			) : (
				<div className="animate-spin rounded-full size-6 border-b-2 border-primary" />
			)}
		</div>
	);
}
