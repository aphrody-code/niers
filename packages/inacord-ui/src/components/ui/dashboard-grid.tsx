import { Card, CardContent } from "./card";
import type { LucideIcon } from "lucide-react";
import { Link } from "../../compat/next";
import { cn } from "../../lib/utils";

interface QuickActionProps {
	title: string;
	description: string;
	href: string;
	icon: LucideIcon;
	accent?: string;
}

/**
 * Shared Quick Action card for Dashboards.
 */
export function QuickActionCard({
	title,
	description,
	href,
	icon: Icon,
	accent = "text-primary",
}: QuickActionProps) {
	return (
		<Link href={href} className="group">
			<Card className="h-full transition-all group-hover:border-primary/40 group-hover:-translate-y-0.5 group-hover:shadow-md">
				<CardContent className="flex items-start gap-3 p-4">
					<div className={cn("mt-0.5", accent)} aria-hidden="true">
						<Icon className="size-5" />
					</div>
					<div className="min-w-0 flex-1">
						<p className="font-semibold text-foreground">{title}</p>
						<p className="mt-0.5 text-xs text-muted-foreground">{description}</p>
					</div>
				</CardContent>
			</Card>
		</Link>
	);
}

interface DashboardGridProps {
	children: React.ReactNode;
	title?: string;
	className?: string;
}

/**
 * Responsive grid for Dashboard sections.
 */
export function DashboardGrid({ children, title, className }: DashboardGridProps) {
	return (
		<section className="space-y-3">
			{title && (
				<h2 className="text-sm font-semibold uppercase tracking-wide text-muted-foreground">
					{title}
				</h2>
			)}
			<div className={cn("grid gap-4 sm:grid-cols-2 lg:grid-cols-4", className)}>{children}</div>
		</section>
	);
}
