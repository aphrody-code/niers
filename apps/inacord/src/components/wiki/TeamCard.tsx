"use client";

import { Image } from "@/components/ui/image";
import { Link } from "@/components/ui/link";
import { useState } from "react";
import { Icon } from "@/components/ui/Icon";
import { SEASON_LABELS, SERIES_LABELS, type TeamListItem } from "@/lib/wikiTypes";
import { cn } from "@/lib/utils";

interface TeamCardProps {
	team: TeamListItem;
	className?: string;
}

export function TeamCard({ team, className }: TeamCardProps) {
	const [imgError, setImgError] = useState(false);
	const hasEmblem = Boolean(team.emblemUrl) && !imgError;

	// Saisons triées par effectif décroissant (les plus représentatives en premier).
	const seasonEntries = Object.entries(team.seasons).sort(([, a], [, b]) => b - a);
	const topSeasons = seasonEntries.slice(0, 3);

	return (
		<Link
			href={`/equipe/${team.id}`}
			className={cn(
				"block h-full transition-all duration-200 relative overflow-hidden group",
				"hover:shadow-lg hover:-translate-y-0.5",
				"border border-outline-variant/30 bg-surface-container-low hover:bg-surface-container",
				"rounded-2xl",
				className
			)}
		>
			<div className="p-4 flex gap-4">
				{/* Emblème */}
				<div className="size-14 shrink-0 rounded-xl bg-surface-container-high flex items-center justify-center border border-outline-variant/20 overflow-hidden">
					{hasEmblem ? (
						<Image
							src={team.emblemUrl as string}
							alt={team.name}
							width={48}
							height={48}
							className="object-contain size-12"
							unoptimized
							onError={() => setImgError(true)}
						/>
					) : (
						<Icon name="shield" size={24} className="text-on-surface-variant/30" />
					)}
				</div>

				{/* Contenu */}
				<div className="flex-1 min-w-0">
					<h3 className="font-bold text-sm text-on-surface leading-tight group-hover:text-primary transition-colors line-clamp-2">
						{team.name}
					</h3>
					{team.nameJa && team.nameJa !== team.name && (
						<p className="text-[11px] text-on-surface-variant/70 truncate mt-0.5">{team.nameJa}</p>
					)}

					{/* Chips méta */}
					<div className="flex flex-wrap items-center gap-1.5 mt-1.5">
						{team.rosterCount > 0 && (
							<span className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded bg-primary/10 text-[10px] font-bold text-primary">
								<Icon name="group" size={12} />
								{team.rosterCount}
							</span>
						)}
						{team.seriesKeys.map((s) => (
							<span
								key={s}
								className="inline-flex items-center px-1.5 py-0.5 rounded bg-surface-container-highest/50 text-[10px] font-bold text-on-surface-variant uppercase tracking-wider"
								title={SERIES_LABELS[s] ?? s}
							>
								{s === "aresOrion" ? "ARES" : s}
							</span>
						))}
					</div>

					{/* Apparitions jeu */}
					{topSeasons.length > 0 && (
						<div className="flex flex-wrap items-center gap-1 mt-2">
							{topSeasons.map(([code]) => (
								<span
									key={code}
									className="inline-flex items-center px-1.5 py-0.5 rounded bg-tertiary/10 text-[10px] font-medium text-tertiary"
									title={SEASON_LABELS[code] ?? code}
								>
									{code}
								</span>
							))}
						</div>
					)}
				</div>
			</div>
		</Link>
	);
}
