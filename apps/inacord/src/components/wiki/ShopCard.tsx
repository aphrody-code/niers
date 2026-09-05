import { Link } from "@/components/ui/link";
import { Icon } from "@/components/ui/Icon";
import { SHOP_CATEGORY_FR } from "@/lib/wikiTypes";
import { cn } from "@/lib/utils";

/** Icône Material par catégorie d'objet (réutilise le mapping ItemCard). */
const CATEGORY_ICON: Record<string, string> = {
	accessory: "diamond",
	animal: "pets",
	consume: "local_pharmacy",
	costume: "apparel",
	craft_obj: "construction",
	emblem: "shield",
	fashion: "styler",
	important: "priority_high",
	kizuna_link: "link",
	misanga: "wrist_band",
	name_plate: "id_card",
	shoes: "steps",
	special: "star",
	special_tactics: "strategy",
	super_tactics: "military_tech",
	title: "badge",
};

export interface ShopCardProps {
	shopId: number;
	name: string;
	nameJa?: string | null;
	itemCount: number;
	categories: { category: string; count: number }[];
	className?: string;
}

export function ShopCard({ shopId, name, nameJa, itemCount, categories, className }: ShopCardProps) {
	const topCategories = categories.slice(0, 4);

	return (
		<Link
			href={`/boutique/${shopId}`}
			className={cn(
				"block h-full transition-all duration-200 relative overflow-hidden group",
				"hover:shadow-lg hover:-translate-y-0.5",
				"border border-outline-variant/30 bg-surface-container-low hover:bg-surface-container",
				"rounded-2xl",
				className
			)}
		>
			<div className="p-4 flex gap-4">
				{/* Icône boutique */}
				<div className="size-14 shrink-0 rounded-xl bg-surface-container-high flex items-center justify-center border border-outline-variant/20">
					<Icon name="storefront" size={26} className="text-primary" />
				</div>

				{/* Contenu */}
				<div className="flex-1 min-w-0">
					<h3 className="font-bold text-sm text-on-surface leading-tight group-hover:text-primary transition-colors line-clamp-2">
						{name}
					</h3>
					{nameJa ? (
						<p className="text-[11px] text-on-surface-variant/70 truncate mt-0.5">{nameJa}</p>
					) : null}

					<div className="flex items-center gap-1.5 mt-1.5">
						<span className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded bg-primary/10 text-[10px] font-bold text-primary uppercase tracking-wider">
							<Icon name="inventory_2" size={12} />
							{itemCount} objets
						</span>
					</div>

					{topCategories.length > 0 ? (
						<div className="flex flex-wrap items-center gap-1.5 mt-2">
							{topCategories.map(({ category, count }) => (
								<span
									key={category}
									className="inline-flex items-center gap-0.5 px-1.5 py-0.5 rounded bg-surface-container-highest/50 text-[10px] font-medium text-on-surface-variant"
								>
									<Icon name={CATEGORY_ICON[category] || "category"} size={12} />
									{SHOP_CATEGORY_FR[category] || category}
									<span className="opacity-60">·{count}</span>
								</span>
							))}
						</div>
					) : null}
				</div>
			</div>
		</Link>
	);
}
