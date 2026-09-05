"use client";

import { ChevronDown, Flame, Ghost, Info, Pin, Sparkles } from "lucide-react";
import { useState } from "react";
import { Icon } from "@/components/ui/Icon";
import { cn } from "@/lib/utils";

export interface ObtainSource {
	type: "player-universe" | "chronicle" | "event" | "gacha";
	name: string;
	details?: string[];
}

export interface SpiritExchangeEntry {
	shop: string;
	tier: string;
	cost: number;
}

export interface HowToObtainProps {
	constellation?: {
		index: number;
		names: {
			fr?: string;
			en?: string;
			ja?: string;
		};
	};
	heroConstellations?: Array<{ name: string; index: number }>;
	spiritExchange?: SpiritExchangeEntry[];
	sources?: ObtainSource[];
	className?: string;
}

/**
 * How to Obtain section - displays where a character can be acquired
 */
export function HowToObtain({
	constellation,
	heroConstellations,
	spiritExchange,
	sources = [],
	className,
}: HowToObtainProps) {
	const [isExpanded, setIsExpanded] = useState(true);

	const hasSpiritExchange = spiritExchange && spiritExchange.length > 0;

	// If no constellation and no sources and no spirit exchange, don't render
	if (
		!constellation &&
		(!heroConstellations || heroConstellations.length === 0) &&
		!hasSpiritExchange &&
		sources.length === 0
	) {
		return null;
	}

	return (
		<div className={cn("w-full max-w-4xl mx-auto mt-4", className)}>
			{/* Header */}
			<button
				onClick={() => setIsExpanded(!isExpanded)}
				className="w-full flex items-center justify-between bg-amber-400 dark:bg-amber-500 px-4 py-2 rounded-t-xl hover:bg-amber-500 dark:hover:bg-amber-600 transition-colors"
			>
				<div className="flex items-center gap-2">
					<Info size={20} className="text-amber-900" aria-hidden="true" />
					<h3 className="text-sm font-bold text-amber-900 uppercase tracking-wide">
						Comment l&apos;obtenir
					</h3>
				</div>
				<ChevronDown
					size={20}
					className="text-amber-900 transition-transform duration-200"
					style={{ transform: isExpanded ? "rotate(180deg)" : "rotate(0deg)" }}
					aria-hidden="true"
				/>
			</button>

			{/* Content */}
			{isExpanded && (
				<div className="bg-white dark:bg-slate-900 border border-t-0 border-slate-200 dark:border-slate-700 rounded-b-xl p-4 space-y-4">
					{/* Player Universe Section */}
					{(constellation || (heroConstellations && heroConstellations.length > 0)) && (
						<div className="space-y-2">
							<div className="flex items-center gap-2">
								<Sparkles size={18} className="text-purple-500" aria-hidden="true" />
								<h4 className="text-sm font-semibold text-slate-700 dark:text-slate-300">
									Univers du joueur
								</h4>
							</div>
							<div className="ml-7 flex flex-wrap gap-2">
								{heroConstellations && heroConstellations.length > 0 ? (
									heroConstellations.map((hc) => (
										<div
											key={hc.name}
											className="inline-flex items-center gap-2 px-3 py-1.5 bg-linear-to-r from-purple-100 to-indigo-100 dark:from-purple-900/30 dark:to-indigo-900/30 rounded-full border border-purple-200 dark:border-purple-700"
										>
											<Pin size={14} className="text-purple-500" aria-hidden="true" />
											<span className="text-sm font-medium text-purple-700 dark:text-purple-300">
												{hc.name}
											</span>
										</div>
									))
								) : constellation ? (
									<div className="inline-flex items-center gap-2 px-3 py-1.5 bg-linear-to-r from-purple-100 to-indigo-100 dark:from-purple-900/30 dark:to-indigo-900/30 rounded-full border border-purple-200 dark:border-purple-700">
										<Pin size={14} className="text-purple-500" aria-hidden="true" />
										<span className="text-sm font-medium text-purple-700 dark:text-purple-300">
											{constellation.names.fr ||
												constellation.names.en ||
												constellation.names.ja ||
												`Constellation ${constellation.index + 1}`}
										</span>
									</div>
								) : null}
							</div>
						</div>
					)}

					{/* Spirit Exchange Section */}
					{hasSpiritExchange && <SpiritExchangeSection entries={spiritExchange!} />}

					{/* Additional Sources */}
					{sources.map((source, idx) => (
						<SourceSection key={idx} source={source} />
					))}

					{/* Empty state for characters without constellation */}
					{!constellation && !hasSpiritExchange && sources.length === 0 && (
						<p className="text-sm text-slate-500 dark:text-slate-400 italic">
							Methode d&apos;acquisition inconnue
						</p>
					)}
				</div>
			)}
		</div>
	);
}

interface SourceSectionProps {
	source: ObtainSource;
}

function SourceSection({ source }: SourceSectionProps) {
	const [isOpen, setIsOpen] = useState(false);

	const icons: Record<ObtainSource["type"], string> = {
		chronicle: "menu_book",
		event: "celebration",
		gacha: "casino",
		"player-universe": "auto_awesome",
	};

	const colors: Record<ObtainSource["type"], string> = {
		chronicle: "text-blue-500",
		event: "text-amber-500",
		gacha: "text-pink-500",
		"player-universe": "text-purple-500",
	};

	return (
		<div className="space-y-2">
			<button
				onClick={() => source.details?.length && setIsOpen(!isOpen)}
				className="flex items-center gap-2 w-full text-left"
				disabled={!source.details?.length}
			>
				<Icon name={icons[source.type]} size={18} className={colors[source.type]} />
				<h4 className="text-sm font-semibold text-slate-700 dark:text-slate-300 flex-1">
					{source.name}
				</h4>
				{source.details && source.details.length > 0 && (
					<ChevronDown
						size={18}
						className="text-slate-400 transition-transform duration-200"
						style={{ transform: isOpen ? "rotate(180deg)" : "rotate(0deg)" }}
						aria-hidden="true"
					/>
				)}
			</button>

			{isOpen && source.details && (
				<ul className="ml-7 space-y-1">
					{source.details.map((detail, i) => (
						<li
							key={i}
							className="text-sm text-slate-600 dark:text-slate-400 flex items-center gap-2"
						>
							<span className="w-1.5 h-1.5 rounded-full bg-slate-300 dark:bg-slate-600" />
							{detail}
						</li>
					))}
				</ul>
			)}
		</div>
	);
}

const TIER_STYLES: Record<string, { bg: string; text: string; border: string }> = {
	BASARA: {
		bg: "bg-red-100 dark:bg-red-900/30",
		border: "border-red-200 dark:border-red-700",
		text: "text-red-700 dark:text-red-300",
	},
	"En progression": {
		bg: "bg-green-100 dark:bg-green-900/30",
		border: "border-green-200 dark:border-green-700",
		text: "text-green-700 dark:text-green-300",
	},
	Expérimenté: {
		bg: "bg-blue-100 dark:bg-blue-900/30",
		border: "border-blue-200 dark:border-blue-700",
		text: "text-blue-700 dark:text-blue-300",
	},
	Héros: {
		bg: "bg-amber-100 dark:bg-amber-900/30",
		border: "border-amber-200 dark:border-amber-700",
		text: "text-amber-700 dark:text-amber-300",
	},
	Normal: {
		bg: "bg-slate-100 dark:bg-slate-800",
		border: "border-slate-200 dark:border-slate-700",
		text: "text-slate-700 dark:text-slate-300",
	},
	Émérite: {
		bg: "bg-purple-100 dark:bg-purple-900/30",
		border: "border-purple-200 dark:border-purple-700",
		text: "text-purple-700 dark:text-purple-300",
	},
};

function SpiritExchangeSection({ entries }: { entries: SpiritExchangeEntry[] }) {
	// Group by shop
	const byShop = new Map<string, SpiritExchangeEntry[]>();
	for (const entry of entries) {
		if (!byShop.has(entry.shop)) {
			byShop.set(entry.shop, []);
		}
		byShop.get(entry.shop)!.push(entry);
	}

	return (
		<div className="space-y-2">
			{[...byShop.entries()].map(([shop, shopEntries]) => {
				const isBasara = shop === "Repaire des indomptables";
				return (
					<div key={shop} className="space-y-2">
						<div className="flex items-center gap-2">
							{isBasara ? (
								<Flame size={18} className="text-red-500" aria-hidden="true" />
							) : (
								<Ghost size={18} className="text-indigo-500" aria-hidden="true" />
							)}
							<h4 className="text-sm font-semibold text-slate-700 dark:text-slate-300">{shop}</h4>
						</div>
						<div className="ml-7 flex flex-wrap gap-2">
							{shopEntries.map((entry, i) => {
								const style = TIER_STYLES[entry.tier] || TIER_STYLES.Normal;
								return (
									<div
										key={i}
										className={cn(
											"inline-flex items-center gap-1.5 px-3 py-1.5 rounded-full border text-sm font-medium",
											style.bg,
											style.text,
											style.border
										)}
									>
										<span>{entry.tier}</span>
										<span className="opacity-60">
											{entry.cost > 0 ? `\u00D7${entry.cost} esprits` : ""}
										</span>
									</div>
								);
							})}
						</div>
					</div>
				);
			})}
		</div>
	);
}
