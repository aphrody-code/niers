"use client";

/**
 * Lecteur de la VOIX japonaise du personnage (1 piste par forme/ère).
 *
 * Le CDN `/audio/<acb>` décode le conteneur CRI live en WAV → lisible directement par
 * `<audio src>` (aucun décodage client). Sélecteur de forme + bouton de téléchargement
 * (même URL). Frontière stricte : aucun import server-only.
 */
import { Download } from "lucide-react";
import { useState } from "react";
import type { DemoFormData } from "./types";

export function DemoVoicePlayer({ forms }: { forms: DemoFormData[] }) {
	const [idx, setIdx] = useState(0);
	const form = forms[idx] ?? forms[0];
	if (!form) return null;

	return (
		<div className="space-y-3 rounded-2xl border border-outline-variant/20 bg-surface-container p-4">
			<h3 className="text-sm font-bold text-on-surface">Voix japonaise</h3>
			<div className="flex flex-wrap gap-1.5">
				{forms.map((f, i) => (
					<button
						key={f.code}
						type="button"
						onClick={() => setIdx(i)}
						className={`h-8 rounded-full px-3 text-xs font-medium transition-colors ${
							i === idx
								? "bg-primary text-on-primary"
								: "bg-surface-container-high text-on-surface-variant hover:bg-surface-container-highest"
						}`}
					>
						{f.label}
					</button>
				))}
			</div>
			{/* `key` force le remontage de la balise audio au changement de forme. */}
			<audio key={form.code} controls preload="none" src={form.voiceUrl} className="w-full">
				<track kind="captions" />
			</audio>
			<a
				href={form.voiceUrl}
				download={`${form.code}.wav`}
				className="inline-flex items-center gap-1 rounded-full bg-surface-container-high px-2.5 py-1 text-xs font-medium text-on-surface-variant hover:bg-surface-container-highest"
			>
				<Download className="size-3" /> Télécharger (WAV)
			</a>
		</div>
	);
}
