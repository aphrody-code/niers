"use client";

/**
 * Section des TECHNIQUES (hissatsu) du personnage. Pour chaque skill : nom (élément/catégorie
 * localisés), puissance min-max, TP, niveau d'apprentissage, bannière telop (nom rendu) et glyphes
 * d'évolution (`SkillEvolutionGlyphs`, gaiji). Le bouton « Jouer dans la scène » n'apparaît que si
 * le cut-in 3D est réellement servi (`has3d`, gate anti-404) et pilote la scène héro via
 * `onPlayCutin`. Présentationnel (n'importe que GaijiGlyph, pur).
 */
import { Play } from "lucide-react";
import { SkillEvolutionGlyphs } from "@/components/wiki/GaijiGlyph";
import type { DemoSkill } from "./types";

export function DemoSkillsSection({
	skills,
	activeEventId,
	onPlayCutin,
}: {
	skills: DemoSkill[];
	/** event_id_name du cut-in actuellement chargé dans la scène (surlignage). */
	activeEventId: string | null;
	onPlayCutin: (skill: DemoSkill) => void;
}) {
	return (
		<section className="space-y-3">
			<h2 className="text-fluid-headline-sm font-bold text-on-surface">
				Techniques ({skills.length})
			</h2>
			<div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
				{skills.map((sk) => {
					const active = sk.has3d && sk.eventIdName === activeEventId;
					return (
						<article
							key={sk.skillIdStr}
							className={`space-y-2 rounded-2xl border bg-surface-container-low/40 p-4 ${
								active ? "border-primary" : "border-outline-variant/20"
							}`}
						>
							{/* Telop = bannière large (1728×352 ~4.9:1), jamais écrasée en carré. */}
							{sk.telopUrl && (
								// eslint-disable-next-line @next/next/no-img-element
								<img
									src={sk.telopUrl}
									alt={`${sk.elementName.fr} ${sk.categoryName.fr}`}
									loading="lazy"
									className="aspect-[24/5] w-full rounded-md object-contain"
								/>
							)}
							<div className="flex items-center justify-between gap-2">
								<div className="min-w-0">
									<p className="truncate text-sm font-semibold text-on-surface">
										{sk.elementName.fr} · {sk.categoryName.fr}
									</p>
									<p className="font-mono text-[11px] text-on-surface-variant/70">
										{sk.skillIdStr} · {sk.eventIdName}
									</p>
								</div>
								<SkillEvolutionGlyphs growthType={sk.growthType} size={22} />
							</div>
							<dl className="flex flex-wrap gap-x-4 gap-y-1 text-xs text-on-surface-variant">
								<div>
									<dt className="inline font-medium">Puissance </dt>
									<dd className="inline tabular-nums text-on-surface">
										{sk.powerMin}–{sk.powerMax}
									</dd>
								</div>
								<div>
									<dt className="inline font-medium">TP </dt>
									<dd className="inline tabular-nums text-on-surface">{sk.consumeTp}</dd>
								</div>
								{sk.learnLevel > 0 && (
									<div>
										<dt className="inline font-medium">Niv. </dt>
										<dd className="inline tabular-nums text-on-surface">{sk.learnLevel}</dd>
									</div>
								)}
							</dl>
							{sk.has3d ? (
								<button
									type="button"
									onClick={() => onPlayCutin(sk)}
									className="inline-flex items-center gap-1 rounded-full bg-primary px-3 py-1 text-xs font-medium text-on-primary hover:opacity-90"
								>
									<Play className="size-3" /> {active ? "Dans la scène" : "Jouer dans la scène"}
								</button>
							) : (
								<p className="text-[11px] text-on-surface-variant/60">
									Cut-in 3D non disponible (telop + métadonnées seuls).
								</p>
							)}
						</article>
					);
				})}
			</div>
		</section>
	);
}
