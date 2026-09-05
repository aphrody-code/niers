"use client";

/**
 * Galerie des TEXTURES du personnage (visages, portraits, cut-ins, telops) — toutes
 * téléchargeables (lien `download` sur le PNG plein format ; vignette WebP `?w=400` pour la grille).
 * Présentationnel pur (aucun import lourd).
 */
import { Download } from "lucide-react";
import type { DemoTexture } from "./types";

const GROUP_LABELS: Record<DemoTexture["group"], string> = {
	face: "Visages",
	icon: "Portraits",
	waza: "Cut-ins",
	telop: "Telops",
};

export function DemoTextureGallery({ textures }: { textures: DemoTexture[] }) {
	if (textures.length === 0) return null;
	const groups = (Object.keys(GROUP_LABELS) as DemoTexture["group"][])
		.map((g) => ({ group: g, items: textures.filter((t) => t.group === g) }))
		.filter((g) => g.items.length > 0);

	return (
		<section className="space-y-4">
			<h2 className="text-fluid-headline-sm font-bold text-on-surface">
				Textures ({textures.length})
			</h2>
			{groups.map(({ group, items }) => (
				<div key={group} className="space-y-2">
					<h3 className="text-sm font-semibold text-on-surface-variant">{GROUP_LABELS[group]}</h3>
					<div className="grid grid-cols-2 gap-3 sm:grid-cols-3 md:grid-cols-4">
						{items.map((t) => (
							<figure
								key={t.url}
								className="overflow-hidden rounded-xl border border-outline-variant/20 bg-surface-container"
							>
								<div className="relative aspect-square bg-surface-container-high">
									{/* eslint-disable-next-line @next/next/no-img-element */}
									<img
										src={t.thumbUrl ?? t.url}
										alt={t.label}
										loading="lazy"
										className="absolute inset-0 size-full object-contain [image-rendering:pixelated]"
									/>
								</div>
								<figcaption className="flex items-center justify-between gap-1 p-2">
									<span className="truncate text-[11px] text-on-surface-variant" title={t.label}>
										{t.label}
									</span>
									<a
										href={t.url}
										download
										title="Télécharger le PNG"
										className="shrink-0 text-on-surface-variant hover:text-primary"
									>
										<Download className="size-3.5" />
									</a>
								</figcaption>
							</figure>
						))}
					</div>
				</div>
			))}
		</section>
	);
}
