import type * as React from "react";
import { isValidElement } from "react";
import { Link } from "../../compat/next";
import type { LucideIcon } from "lucide-react";
import { cn } from "../../lib/utils";

interface DashboardStatsCardProps {
	title?: string;
	label?: string; // Alias for Azalee
	value: string | number;
	/** Composant Lucide, ou nœud déjà rendu (ex. une icône applicative). */
	icon: LucideIcon | React.ReactNode;
	description?: string;
	href?: string;
	trend?: {
		value: number;
		label?: string;
	};
	accent?: "primary" | "emerald" | "sky" | "rose" | "amber" | "violet";
	accentColor?: string; // Legacy alias for Azalee
	className?: string;
}

/**
 * Accents des cartes.
 *
 * Ils pointaient sur la palette Tailwind brute (`sky-500`, `violet-500`…) :
 * les cartes gardaient donc les mêmes teintes quel que soit le thème actif, et
 * juraient avec Roy, Gaëlle et Azalée. Les noms de clés sont conservés pour ne
 * pas casser les appelants, mais chacun renvoie désormais vers un token du
 * design system, donc vers une couleur qui suit le thème.
 */
const accentVariants = {
	primary: "bg-primary/10 text-primary",
	emerald: "bg-succes/10 text-succes",
	sky: "bg-info/10 text-info",
	rose: "bg-rg-rose/10 text-rg-rose",
	amber: "bg-alerte/15 text-alerte",
	violet: "bg-secondary/15 text-secondary-foreground",
};

/**
 * Affiche une icône fournie soit comme TYPE de composant, soit déjà rendue.
 *
 * Le discriminant est `isValidElement`, jamais `typeof === "function"` : les
 * icônes `lucide-react` sont des `forwardRef`, donc des objets.
 */
function renderIcone(icone: React.ReactNode | LucideIcon): React.ReactNode {
	if (!icone) {
		return null;
	}
	if (isValidElement(icone)) {
		return icone;
	}
	const Composant = icone as React.ComponentType<{ className?: string }>;
	return <Composant className="size-5" />;
}

/**
 * Une icône Lucide n'est PAS une fonction.
 *
 * `lucide-react` expose des composants `forwardRef`, c'est-à-dire des OBJETS
 * (`{$$typeof, render, displayName}`). Le test `typeof icon === "function"`
 * était donc toujours faux : la branche « nœud déjà rendu » s'exécutait et
 * injectait l'objet brut comme enfant React. Au rendu serveur cela passait
 * inaperçu — React ignore l'objet — mais la sérialisation du flux RSC échouait
 * avec « Functions cannot be passed directly to Client Components », une fois
 * par icône distincte, et l'hydratation du tableau de bord tombait.
 *
 * Le bon test est l'inverse : si c'est déjà un élément React, on l'affiche tel
 * quel ; sinon c'est un TYPE de composant (fonction ou forwardRef) qu'il faut
 * instancier.
 */
/**
 * Shared Metric/Stats card for Dashboards.
 */
export function DashboardStatsCard({
	title,
	label,
	value,
	icon,
	description,
	href,
	trend,
	accent = "primary",
	accentColor,
	className,
}: DashboardStatsCardProps) {
	const Icon = icon;
	const displayTitle = title || label || "";
	const finalAccent = (accentColor || accent) as keyof typeof accentVariants;

	const content = (
		<div
			className={cn(
				"group relative overflow-hidden rounded-xl border border-border bg-card p-6 transition-all hover:shadow-md",
				className
			)}
		>
			<div className="flex items-center justify-between">
				<div
					className={cn("rounded-lg p-2.5", accentVariants[finalAccent] || accentVariants.primary)}
				>
					{/* La branche « chaîne » s'appuyait sur une police `material-icons`
					    qui n'est déclarée dans aucune des deux apps : le nom de l'icône
					    s'affichait en toutes lettres, débordant de son carré de 20px.
					    On n'accepte plus qu'un composant Lucide ou un nœud déjà rendu. */}
					{renderIcone(Icon)}
				</div>
				{trend && (
					<span
						className={cn(
							"text-xs font-medium",
							trend.value > 0 ? "text-succes" : "text-destructive"
						)}
					>
						{trend.value > 0 ? "+" : ""}
						{trend.value}%
					</span>
				)}
			</div>
			<div className="mt-4">
				<p className="text-sm font-medium text-muted-foreground">{displayTitle}</p>
				<h3 className="text-2xl font-bold tracking-tight text-foreground">
					{typeof value === "number" ? value.toLocaleString("fr-FR") : value}
				</h3>
				{description && <p className="mt-1 text-xs text-muted-foreground">{description}</p>}
			</div>
		</div>
	);

	if (href) {
		return <Link href={href}>{content}</Link>;
	}

	return content;
}
