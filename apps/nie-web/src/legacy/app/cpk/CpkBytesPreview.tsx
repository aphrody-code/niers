"use client";

/**
 * Prévisualise des **octets en mémoire** (un sous-fichier extrait d'une archive g4pk,
 * pas un chemin) en les décodant via les parseurs wasm natifs selon leur extension :
 *   - g4tx → image (PNG décodé in-browser, blob URL) ;
 *   - bin/objbin/… → config typée (arbre JSON) ;
 *   - g4pk/g4pkm → archive imbriquée (table de sous-fichiers) ;
 *   - acb/awb/hca/adx → audio (WAV, blob URL) ;
 *   - autre → dump hexadécimal des premiers Ko.
 *
 * Réutilisé par `CpkPackageViewer` pour la prévisualisation récursive des sous-fichiers.
 */
import { useEffect, useMemo, useState } from "react";
import { CpkJsonTree } from "@/app/cpk/CpkJsonTree";
import { cpkPreviewKind } from "@rosegriffon/azalee/cpk/shared";
import { audioToWav, cfgbinTyped, g4pkParse, g4txToPng, type G4pkParsed } from "@/lib/cpk-wasm";

function hexDump(bytes: Uint8Array, max = 16 * 64): string {
	const lines: string[] = [];
	const n = Math.min(bytes.length, max);
	for (let off = 0; off < n; off += 16) {
		const slice = bytes.subarray(off, off + 16);
		const hex = Array.from(slice, (b) => b.toString(16).padStart(2, "0")).join(" ");
		const ascii = Array.from(slice, (b) =>
			b >= 0x20 && b < 0x7f ? String.fromCharCode(b) : "."
		).join("");
		lines.push(`${off.toString(16).padStart(8, "0")}  ${hex.padEnd(47)}  ${ascii}`);
	}
	if (bytes.length > n) lines.push(`… (${bytes.length - n} octets restants)`);
	return lines.join("\n");
}

function humanSize(n: number): string {
	if (n < 1024) return `${n} o`;
	if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} Ko`;
	return `${(n / (1024 * 1024)).toFixed(2)} Mo`;
}

type Decoded =
	| { kind: "image"; url: string }
	| { kind: "json"; data: unknown; label: string }
	| { kind: "package"; pkg: G4pkParsed }
	| { kind: "audio"; url: string }
	| { kind: "hex"; text: string }
	| { kind: "loading" };

export function CpkBytesPreview({ bytes, name }: { bytes: Uint8Array; name: string }) {
	const [state, setState] = useState<Decoded>({ kind: "loading" });
	const ext = useMemo(() => name.split(".").pop()?.toLowerCase() ?? "", [name]);

	useEffect(() => {
		let cancelled = false;
		let objectUrl: string | null = null;
		setState({ kind: "loading" });

		(async () => {
			try {
				const previewKind = cpkPreviewKind(ext);
				if (previewKind === "image") {
					const png = await g4txToPng(bytes);
					objectUrl = URL.createObjectURL(new Blob([png.slice()], { type: "image/png" }));
					if (!cancelled) setState({ kind: "image", url: objectUrl });
				} else if (previewKind === "config") {
					const body = await cfgbinTyped(bytes, name);
					if (!cancelled) {
						setState({
							kind: "json",
							data: body.family ? body.data : body.generic,
							label: body.family ?? "config",
						});
					}
				} else if (previewKind === "package") {
					const pkg = await g4pkParse(bytes);
					if (!cancelled) setState({ kind: "package", pkg });
				} else if (previewKind === "sound") {
					const wav = await audioToWav(bytes);
					objectUrl = URL.createObjectURL(new Blob([wav.slice()], { type: "audio/wav" }));
					if (!cancelled) setState({ kind: "audio", url: objectUrl });
				} else {
					if (!cancelled) setState({ kind: "hex", text: hexDump(bytes) });
				}
			} catch {
				if (!cancelled) setState({ kind: "hex", text: hexDump(bytes) });
			}
		})();

		return () => {
			cancelled = true;
			if (objectUrl) URL.revokeObjectURL(objectUrl);
		};
	}, [bytes, name, ext]);

	if (state.kind === "loading") {
		return (
			<div className="flex items-center justify-center p-4">
				<div className="animate-spin rounded-full size-5 border-b-2 border-primary" />
			</div>
		);
	}
	if (state.kind === "image") {
		return (
			<div
				className="p-2"
				style={{
					background:
						"repeating-conic-gradient(rgba(120,120,120,.18) 0% 25%, transparent 0% 50%) 50% / 16px 16px",
				}}
			>
				{/* eslint-disable-next-line @next/next/no-img-element */}
				<img
					src={state.url}
					alt={name}
					className="mx-auto max-h-[40vh] w-auto object-contain"
					style={{ imageRendering: "pixelated" }}
				/>
			</div>
		);
	}
	if (state.kind === "audio") {
		// eslint-disable-next-line jsx-a11y/media-has-caption
		return <audio controls className="w-full p-2" src={state.url} />;
	}
	if (state.kind === "json") {
		return <CpkJsonTree data={state.data} rootLabel={state.label} />;
	}
	if (state.kind === "package") {
		return (
			<div className="p-3 text-xs font-mono">
				<p className="mb-1 text-on-surface-variant">
					archive imbriquée · {state.pkg.files.length} sous-fichiers
				</p>
				{state.pkg.files.map((f, i) => (
					<div
						key={`${f.name}-${i}`}
						className="flex justify-between border-t border-outline-variant/10 py-0.5"
					>
						<span className="break-all">{f.name || "(sans nom)"}</span>
						<span className="text-on-surface-variant tabular-nums">{humanSize(f.size)}</span>
					</div>
				))}
			</div>
		);
	}
	return (
		<pre className="text-[11px] text-on-surface bg-surface-container-lowest rounded-lg p-2 overflow-auto max-h-[40vh] font-mono leading-tight whitespace-pre">
			{state.text}
		</pre>
	);
}
