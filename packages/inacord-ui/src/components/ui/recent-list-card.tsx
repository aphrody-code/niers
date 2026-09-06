import { Badge } from "./badge";
import { Card, CardContent, CardHeader, CardTitle } from "./card";
import { format } from "date-fns";
import { fr } from "date-fns/locale";
import { ArrowRight, type LucideIcon } from "lucide-react";
import { Link } from "../../compat/next";
import { cn } from "../../lib/utils";

interface RecentItem {
	id: string;
	title: string;
	subtitle?: string;
	date?: string | Date;
	status?: string;
	meta?: React.ReactNode;
}

interface RecentListCardProps {
	title: string;
	items: RecentItem[];
	icon: LucideIcon;
	href: string;
	accent?: string;
	emptyText?: string;
}

/**
 * Reusable Card for "Recent Activities" (Users, Articles, Events)
 */
export function RecentListCard({
	title,
	items,
	icon: Icon,
	href,
	accent = "bg-primary/10 text-primary",
	emptyText = "Aucun élément trouvé.",
}: RecentListCardProps) {
	return (
		<Card>
			<CardHeader className="flex flex-row items-center justify-between space-y-0">
				<CardTitle className="text-base">{title}</CardTitle>
				<Link
					href={href}
					className="flex items-center gap-1 text-xs font-medium text-muted-foreground transition-colors hover:text-primary"
				>
					Tout voir
					<ArrowRight className="size-3" aria-hidden="true" />
				</Link>
			</CardHeader>
			<CardContent>
				{items.length === 0 ? (
					<p className="text-sm text-muted-foreground">{emptyText}</p>
				) : (
					<ul className="divide-y divide-border">
						{items.map((item) => (
							<li key={item.id} className="py-3 first:pt-0 last:pb-0">
								<div className="flex items-start gap-3">
									<div
										className={cn(
											"flex size-9 shrink-0 items-center justify-center rounded-lg",
											accent
										)}
										aria-hidden="true"
									>
										<Icon className="size-4" />
									</div>
									<div className="min-w-0 flex-1">
										<div className="flex items-start justify-between gap-2">
											<p className="line-clamp-1 text-sm font-medium text-foreground">
												{item.title}
											</p>
											{item.status && (
												<Badge variant="outline" className="shrink-0 text-[10px] uppercase">
													{item.status}
												</Badge>
											)}
										</div>
										<div className="mt-0.5 flex items-center gap-3 text-xs text-muted-foreground">
											{item.date && (
												<span>
													{format(new Date(item.date), "d MMM yyyy", {
														locale: fr,
													})}
												</span>
											)}
											{item.subtitle && <span className="truncate">{item.subtitle}</span>}
											{item.meta}
										</div>
									</div>
								</div>
							</li>
						))}
					</ul>
				)}
			</CardContent>
		</Card>
	);
}
