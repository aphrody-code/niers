import { Link } from "../../../compat/next";
import { Icon } from "../../../components/wiki/ui/Icon";
import { cn } from "../../../lib/utils";

const TOOLS: Array<{
	href: string;
	icon: string;
	title: string;
	description: string;
	container: string;
}> = [
	{
		container: "bg-primary-container text-on-primary-container",
		description: "Stats, techniques et auras de deux joueurs côte à côte.",
		href: "/tools/compare",
		icon: "compare_arrows",
		title: "Comparateur",
	},
	{
		container: "bg-secondary-container text-on-secondary-container",
		description: "11 joueurs, 1 coach, 3 manageuses. Filtrez par élément ou style de jeu.",
		href: "/tools/random-team",
		icon: "casino",
		title: "Équipe aléatoire",
	},
	{
		container: "bg-tertiary-container text-on-tertiary-container",
		description: "Trouvez les noms français, anglais et japonais de toutes les entités du jeu.",
		href: "/tools/translator",
		icon: "translate",
		title: "Traducteur",
	},
	{
		container: "bg-primary-container text-on-primary-container",
		description: "Composez votre équipe idéale en glissant-déposant vos joueurs sur le terrain.",
		href: "/tools/my-team",
		icon: "stadium",
		title: "Mon Équipe",
	},
];

export function ToolsPreview() {
	return (
		<section className="w-full">
			{/* Section header */}
			<div className="mb-5 md:mb-6 px-0 md:px-8 lg:px-12">
				<h2 className="font-[BradBunR] text-xl md:text-3xl text-on-surface tracking-wide flex items-center gap-3">
					<span className="w-2 h-8 rounded-full bg-primary" />
					Outils interactifs
				</h2>
				<p className="mt-2 type-body-medium text-on-surface-variant">
					Comparez, générez, traduisez et composez vos équipes.
				</p>
			</div>

			{/* Responsive grid: 1 col → 2 (sm) → 4 (lg) */}
			<div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4 md:gap-5 px-0 md:px-8 lg:px-12">
				{TOOLS.map((tool) => (
					<Link
						key={tool.href}
						href={tool.href}
						className="group relative flex flex-col rounded-2xl overflow-hidden bg-surface-container elevation-1 state-layer hover:elevation-2 active:scale-[0.98] transition-all duration-200"
					>
						{/* Tonal header with icon */}
						<div
							className={cn(
								"relative h-28 md:h-32 flex items-center justify-center overflow-hidden",
								tool.container
							)}
						>
							<div className="transition-transform duration-300 group-hover:scale-110">
								<Icon name={tool.icon} size={48} className="opacity-90" />
							</div>
						</div>

						{/* Content */}
						<div className="relative z-10 flex-1 p-4 md:p-5 flex flex-col gap-1.5">
							<h3 className="type-title-medium text-on-surface group-hover:text-primary transition-colors">
								{tool.title}
							</h3>
							<p className="type-body-small text-on-surface-variant leading-relaxed">
								{tool.description}
							</p>
							<div className="mt-auto pt-3 flex items-center justify-end">
								<Icon
									name="arrow_forward"
									size={18}
									className="text-primary transition-transform group-hover:translate-x-1"
								/>
							</div>
						</div>
					</Link>
				))}
			</div>
		</section>
	);
}
