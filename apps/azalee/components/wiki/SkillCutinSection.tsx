"use client";

/**
 * Section "Cut-in" d'une fiche hissatsu : affiche les assets du cut-in servis live par
 * niers — le **telop** (nom rendu, g4tx→png), le **modèle 3D** du cut-in (`chr/_waza`, assemblé
 * serveur depuis g4pkm→g4md + g4mg + texture g4tx embarquée) et la **texture** (`dx11/chr/_waza`,
 * g4tx→png). Données : `skills-cutin.json`.
 *
 * La **scène 3D** (R3F + timeline + export MP4) a quitté le wiki au lot J2 : elle lisait des
 * fichiers du jeu en local, ce qu'une page servie sans serveur ne peut plus faire. Elle vit
 * désormais dans Aphrody, et cette section n'en garde qu'un lien — actif seulement si
 * `NEXT_PUBLIC_TOOLS_ORIGIN` est renseignée, pour ne jamais pointer dans le vide.
 */
import { useState } from "react";
import { Icon } from "@/components/ui/Icon";
import { cpkAssetUrl } from "@rosegriffon/azalee/cpk/shared";
import { getSkillTelopUrl } from "@rosegriffon/azalee/images";
import { downloadName } from "@rosegriffon/azalee/text/download-filename";
import type { SkillCutin } from "@rosegriffon/azalee/game/skills-cutin";

/** Chip de téléchargement (texture/telop PNG), réutilisable. */
function DownloadChip({ href, filename }: { href: string; filename: string }) {
	return (
		<a
			href={href}
			download={filename}
			className="inline-flex items-center gap-1 rounded-full bg-surface-container-high px-2.5 py-1 text-xs font-medium text-on-surface-variant hover:text-on-surface hover:bg-surface-container-highest transition-colors"
			title="Télécharger (PNG)"
		>
			<Icon name="download" size={12} /> PNG
		</a>
	);
}

export function SkillCutinSection({
	cutin,
	skillCode,
	skillName,
	assetsAvailable,
}: {
	cutin: SkillCutin;
	/** Code interne de la technique (`whs00030`) — la clé de l'index des telops réels. */
	skillCode: string;
	skillName: string;
	/** Le modèle 3D + la texture `_waza` existent dans cette build (vérifié serveur via l'index
	    CPK) : ~157/992 hissatsu. Faux → on n'affiche ni le viewer ni la texture (sinon 404/broken). */
	assetsAvailable: boolean;
}) {
	const [telopError, setTelopError] = useState(false);
	// Telop : résolu contre l'index des fichiers RÉELLEMENT présents dans les CPK.
	//
	// `skills-cutin.json` annonce un telop dans neuf langues pour TOUTE technique,
	// y compris les 19 `rh*` qui n'en ont aucun — la section forgeait donc une URL
	// sans garde et servait un 404 mesuré (`…/telop_waza/fr/rhd10010.png`, et son
	// équivalent `en`). `getSkillTelopUrl` fait la même résolution fr → en que
	// `getSkillImageUrl`, contre le même index, et rend `null` quand rien n'existe
	// (pas de placeholder : il n'a rien à faire dans une section « assets »).
	const telopUrl = getSkillTelopUrl(skillCode);
	// Vide tant qu'Aphrody n'est pas en ligne : le lien disparaît plutôt que de casser.
	const outilsOrigin = process.env.NEXT_PUBLIC_TOOLS_ORIGIN;
	const textureUrl = cpkAssetUrl(cutin.texture_g4tx);

	return (
		<section className="rounded-3xl border border-outline-variant/20 bg-surface-container-low/40 p-5 space-y-4">
			<h2 className="flex items-center gap-2 text-fluid-title-md font-bold text-on-surface">
				<Icon name="auto_awesome" size={20} className="text-primary" />
				Cut-in
			</h2>

			{telopUrl && !telopError && (
				<div>
					<div className="flex items-center justify-between mb-1">
						<p className="text-xs text-on-surface-variant">Telop (nom rendu)</p>
						<DownloadChip href={telopUrl} filename={`${downloadName(skillName, "technique")}_telop.png`} />
					</div>
					{/* eslint-disable-next-line @next/next/no-img-element */}
					<img
						src={telopUrl}
						alt={`Telop ${skillName}`}
						loading="lazy"
						className="max-h-24 w-auto"
						onError={() => setTelopError(true)}
					/>
				</div>
			)}

			{/* Scène cut-in composée + texture : affichées UNIQUEMENT si l'asset `_waza` existe dans
			    cette build (gate serveur `assetsAvailable`). Sinon (835/992 hissatsu sans cut-in 3D),
			    on évite un viewer « indisponible » + une image cassée. Le viewer gère lui-même le
			    chargement lazy de la scène R3F + le repli model-viewer. */}
			{assetsAvailable && (
				<div className="grid grid-cols-1 md:grid-cols-2 gap-4">
					{outilsOrigin && (
						<div>
							<p className="text-xs text-on-surface-variant mb-1">
								Scène cut-in (modèle 3D composé · timeline · export MP4)
							</p>
							<a
								href={`${outilsOrigin}/cutin/${encodeURIComponent(skillCode)}`}
								className="inline-flex items-center gap-1.5 rounded-full bg-surface-container-high px-3 py-1.5 text-sm font-medium text-on-surface-variant hover:text-on-surface hover:bg-surface-container-highest transition-colors"
								rel="noreferrer"
							>
								<Icon name="open_in_new" size={14} /> Ouvrir la scène dans Aphrody
							</a>
						</div>
					)}
					{textureUrl && (
						<div>
							<div className="flex items-center justify-between mb-1">
								<p className="text-xs text-on-surface-variant">Texture (dx11/chr/_waza)</p>
								<DownloadChip href={textureUrl} filename={`${downloadName(skillName, "technique")}.png`} />
							</div>
							{/* eslint-disable-next-line @next/next/no-img-element */}
							<img
								src={textureUrl}
								alt={`Texture ${skillName}`}
								loading="lazy"
								className="w-full max-w-xs rounded-lg bg-surface-container [image-rendering:pixelated]"
							/>
						</div>
					)}
				</div>
			)}

		</section>
	);
}
