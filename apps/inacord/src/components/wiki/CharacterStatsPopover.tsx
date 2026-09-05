"use client";

import type { GameCharacterStats } from "@/lib/wikiTypes";
import { BarChart3 } from "lucide-react";
// Cf. la note d'AuraCard : jamais `@rosegriffon/ui` ici — `process.env` → page blanche.
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { cn } from "@/lib/utils";

interface CharacterStatsPopoverProps {
	stats: GameCharacterStats;
	name: string;
}

const STAT_LABELS = {
	agility: { color: "bg-cyan-500", fr: "Vitesse" },
	control: { color: "bg-blue-500", fr: "Maîtrise" },
	intelligence: { color: "bg-indigo-500", fr: "Intelligence" },
	kick: { color: "bg-red-500", fr: "Frappe" },
	physical: { color: "bg-orange-500", fr: "Physique" },
	pressure: { color: "bg-purple-500", fr: "Défense" }, // "Block" / Pressure
	technique: { color: "bg-green-500", fr: "Technique" },
};

const MAX_STAT = 250; // Estimated max for lv50 visual scaling

export function CharacterStatsPopover({ stats, name }: CharacterStatsPopoverProps) {
	if (!stats) {
		return null;
	}

	return (
		<Popover>
			{/* Le `PopoverTrigger` local (base-ui) REND déjà un `<button>` : il n'a pas de prop
			 * `asChild` (c'est l'API Radix du web), et l'y emboîter produirait un bouton dans un
			 * bouton. Les classes du bouton fantôme sont donc portées directement ici. */}
			<PopoverTrigger className="inline-flex size-11 items-center justify-center rounded-md text-on-surface-variant transition-colors hover:bg-app-hover hover:text-primary sm:size-8">
				<BarChart3 className="size-4" />
				<span className="sr-only">Stats de {name}</span>
			</PopoverTrigger>
			<PopoverContent
				className="w-[calc(100vw-2rem)] max-w-sm sm:w-80 p-4 bg-surface-container-high border-outline-variant"
				align="center"
			>
				<div className="space-y-4">
					<h4 className="font-medium text-on-surface flex items-center justify-between">
						<span>Stats (Niv. 50)</span>
						<span className="text-xs font-normal text-on-surface-variant">Max estimé</span>
					</h4>

					<div className="space-y-3">
						{Object.entries(stats).map(([key, value]) => {
							const config = STAT_LABELS[key as keyof typeof STAT_LABELS];
							if (!config) {
								return null;
							}

							const percentage = Math.min((value / MAX_STAT) * 100, 100);

							return (
								<div key={key} className="space-y-1">
									<div className="flex justify-between text-xs">
										<span className="text-on-surface-variant">{config.fr}</span>
										<span className="font-medium text-on-surface font-mono">{value}</span>
									</div>
									<div className="h-1.5 w-full bg-surface-container-highest rounded-full overflow-hidden">
										<div
											className={cn(
												"h-full rounded-full transition-all duration-500",
												config.color
											)}
											style={{ width: `${percentage}%` }}
										/>
									</div>
								</div>
							);
						})}
					</div>

					<div className="pt-2 border-t border-outline-variant/30 flex justify-between items-center text-xs text-on-surface-variant">
						<span>Total</span>
						<span className="font-medium text-on-surface font-mono text-sm">
							{Object.values(stats).reduce((a, b) => a + b, 0)}
						</span>
					</div>
				</div>
			</PopoverContent>
		</Popover>
	);
}
