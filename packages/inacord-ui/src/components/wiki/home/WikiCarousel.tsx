import { Link } from "../../../compat/next";
import { AnimatedCounter } from "./AnimatedCounter";
import { Icon } from "../../../components/wiki/ui/Icon";
import { CommonSpriteIcon } from "../../../components/wiki/ui/CommonSpriteIcon";
import type { SpriteCommonKey } from "../../../config/sprites-common";

export interface WikiSectionStats {
	characters: number;
	skills: number;
	items: number;
	keshins: number;
	souls: number;
	passives: number;
}

const WIKI_SECTIONS: Array<{
	title: string;
	description: string;
	href: string;
	sprite?: string;
	icon?: string;
	gradientColors: string;
	statKey?: keyof WikiSectionStats;
}> = [
	{
		description: "Stats, techniques et recrutement",
		gradientColors: "bg-linear-to-r from-blue-600 to-cyan-500",
		href: "/chara",
		sprite: "chara",
		statKey: "characters",
		title: "Joueurs",
	},
	{
		description: "Tirs, dribbles et blocages",
		gradientColors: "bg-linear-to-r from-red-600 to-orange-500",
		href: "/skill",
		sprite: "skill",
		statKey: "skills",
		title: "Techniques",
	},
	{
		description: "Keshins, totems, miximax et plus",
		gradientColors: "bg-linear-to-r from-violet-600 to-purple-500",
		href: "/aura",
		sprite: "keshin",
		statKey: "keshins",
		title: "Auras",
	},
	{
		description: "Équipements et consommables",
		gradientColors: "bg-linear-to-r from-emerald-600 to-green-500",
		href: "/item",
		icon: "backpack",
		statKey: "items",
		title: "Objets",
	},
	{
		description: "Effets des talents innés",
		gradientColors: "bg-linear-to-r from-pink-600 to-rose-500",
		href: "/passive",
		sprite: "tension",
		statKey: "passives",
		title: "Passifs",
	},
	{
		description: "Comparez deux joueurs côte à côte",
		gradientColors: "bg-linear-to-r from-indigo-600 to-blue-500",
		href: "/tools/compare",
		icon: "compare_arrows",
		title: "Comparateur",
	},
	{
		description: "Composez votre équipe idéale",
		gradientColors: "bg-linear-to-r from-teal-600 to-cyan-500",
		href: "/tools/my-team",
		icon: "stadium",
		title: "Mon Équipe",
	},
	{
		description: "News et mises à jour",
		gradientColors: "bg-linear-to-r from-slate-600 to-gray-500",
		href: "/news",
		icon: "newspaper",
		title: "Actualités",
	},
];

interface WikiCarouselProps {
	stats?: WikiSectionStats;
}

export function WikiCarousel({ stats }: WikiCarouselProps) {
	return (
		<section className="w-full">
			<div className="mb-5 md:mb-6 px-0 md:px-8 lg:px-12">
				<h2 className="text-xl md:text-3xl font-black text-on-surface font-expressive-title flex items-center gap-3">
					<span className="w-2 h-8 rounded-full bg-secondary" />
					Encyclopédie
				</h2>
			</div>

			{/* Single responsive render: horizontal scroll on mobile, grid on desktop */}
			<div className="flex overflow-x-auto snap-x snap-proximity gap-3 -mx-4 px-4 pb-1 scrollbar-hide md:grid md:grid-cols-2 lg:grid-cols-4 md:gap-4 lg:gap-6 md:mx-0 md:px-8 lg:px-12 md:overflow-visible md:snap-none md:pb-0">
				{WIKI_SECTIONS.map((section) => {
					const count = section.statKey && stats ? stats[section.statKey] : undefined;

					return (
						<Link
							key={section.href}
							href={section.href}
							className="shrink-0 min-w-[140px] snap-start md:min-w-0 md:snap-align-none group flex flex-col rounded-2xl bg-surface-container overflow-hidden transition-colors active:bg-surface-container-high md:transition-all md:duration-200 md:hover:bg-surface-container-high md:hover:shadow-md md:hover:-translate-y-1 relative"
						>
							<div className={`h-[3px] w-full ${section.gradientColors}`} />
							<div className="p-3 md:p-5 flex flex-col gap-1 md:gap-1.5 flex-1 relative z-10">
								<h3 className="text-xs md:text-sm lg:text-base font-bold text-on-surface leading-tight md:group-hover:text-primary md:transition-colors">
									{section.title}
								</h3>
								{count != null && count > 0 && (
									<AnimatedCounter
										value={count}
										className="text-lg md:text-[28px] md:leading-9 font-black text-primary tabular-nums"
									/>
								)}
								<p className="text-[10px] md:text-xs text-on-surface-variant/70 md:text-on-surface-variant leading-snug line-clamp-1 md:line-clamp-none md:opacity-70 md:group-hover:opacity-100 md:transition-opacity">
									{section.description}
								</p>
							</div>

							{/* Floating subtle icon/sprite watermark in bottom-right corner */}
							<div className="absolute bottom-2 right-2 opacity-5 dark:opacity-10 group-hover:opacity-15 group-hover:scale-110 transition-all duration-300 pointer-events-none select-none z-0">
								{section.sprite ? (
									<CommonSpriteIcon
										name={section.sprite as SpriteCommonKey}
										scale={0.7}
										className="filter grayscale contrast-125 brightness-75 dark:brightness-125"
									/>
								) : section.icon ? (
									<Icon name={section.icon} size={42} className="text-on-surface" />
								) : null}
							</div>
						</Link>
					);
				})}
			</div>
		</section>
	);
}
