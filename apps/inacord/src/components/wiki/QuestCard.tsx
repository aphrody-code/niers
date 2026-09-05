import { Flag, ScrollText } from "lucide-react";
import { Link } from "@/components/ui/link";
import type { Quest } from "@/lib/wikiTypes";
import { cn } from "@/lib/utils";

export interface QuestCardProps {
	quest: Quest;
	className?: string;
}

/** Libellé court de zone annexe (dérivé de `area`, pas de table de noms en DB). */
function areaLabel(area: number | null): string {
	return area ? `Zone ${area}` : "Annexe";
}

/**
 * Carte de quête. Les images de quête (`data.image`) ne sont PAS servies par le CDN
 * (toutes les variantes testées → 404) : on n'affiche donc PAS d'`<img>` cassée, mais
 * une icône typée (drapeau = histoire, parchemin = annexe) + le badge phase/zone.
 * 100% des champs rendus proviennent du miroir réel.
 */
export function QuestCard({ quest, className }: QuestCardProps) {
	const isMain = quest.kind === "main";
	const Icon = isMain ? Flag : ScrollText;

	return (
		<Link
			href={`/quete/${quest.id}`}
			className={cn(
				"group flex h-full items-start gap-3 rounded-2xl p-3",
				"border border-outline-variant/30 bg-surface-container-low",
				"transition-all duration-200 hover:-translate-y-0.5 hover:bg-surface-container hover:shadow-lg",
				className
			)}
		>
			<div
				className={cn(
					"flex size-11 shrink-0 items-center justify-center rounded-xl",
					isMain ? "bg-primary/10 text-primary" : "bg-tertiary/10 text-tertiary"
				)}
			>
				<Icon size={20} aria-hidden="true" />
			</div>

			<div className="min-w-0 flex-1">
				<div className="flex items-center gap-2">
					<span
						className={cn(
							"rounded-full px-2 py-0.5 text-[10px] font-bold uppercase tracking-wider",
							isMain
								? "bg-primary/10 text-primary"
								: "bg-tertiary/10 text-tertiary"
						)}
					>
						{isMain ? "Histoire" : areaLabel(quest.area)}
					</span>
					<span className="text-[10px] font-medium text-on-surface-variant/60">
						{isMain ? `Chapitre ${quest.phase}` : `#${quest.phase}`}
					</span>
				</div>
				<h3 className="mt-1 line-clamp-2 text-sm font-bold leading-snug text-on-surface transition-colors group-hover:text-primary">
					{quest.title}
				</h3>
				{quest.titles.en && quest.titles.en !== quest.title && (
					<p className="mt-0.5 line-clamp-1 text-xs text-on-surface-variant/70">
						{quest.titles.en}
					</p>
				)}
			</div>
		</Link>
	);
}
