import { Link } from "../../../compat/next";
import { Icon } from "../../../components/wiki/ui/Icon";

interface DashboardMetricCardProps {
	label: string;
	value: string | number;
	icon: string;
	href?: string;
	description?: string;
	trend?: {
		value: number;
		label?: string;
	};
	accentColor?: string;
}

export function DashboardMetricCard({
	label,
	value,
	icon,
	href,
	description,
	trend,
	accentColor,
}: DashboardMetricCardProps) {
	const iconBg = accentColor || "bg-primary-container text-on-primary-container";

	const content = (
		<div className="flex items-center gap-4 p-4 sm:p-5 rounded-[24px] bg-surface-container-low border border-outline-variant/20 transition-all duration-300 hover:shadow-level-1 hover:bg-surface-container group">
			<div
				className={`size-12 sm:size-14 rounded-[16px] flex items-center justify-center shrink-0 transition-transform group-hover:scale-105 duration-300 ${iconBg}`}
			>
				<Icon name={icon} size={24} fill />
			</div>
			<div className="min-w-0 flex-1">
				<p className="text-sm text-on-surface-variant">{label}</p>
				<div className="flex items-baseline gap-2">
					<p className="text-2xl sm:text-3xl font-normal tracking-tight text-on-surface">
						{typeof value === "number" ? value.toLocaleString("fr-FR") : value}
					</p>
					{trend && (
						<span
							className={`inline-flex items-center gap-0.5 text-xs font-medium ${
								trend.value > 0
									? "text-green-600"
									: trend.value < 0
										? "text-red-500"
										: "text-on-surface-variant"
							}`}
						>
							<Icon
								name={
									trend.value > 0
										? "trending_up"
										: trend.value < 0
											? "trending_down"
											: "trending_flat"
								}
								size={14}
							/>
							{trend.value > 0 ? "+" : ""}
							{trend.value}%
						</span>
					)}
				</div>
				{description && <p className="text-xs text-on-surface-variant/70 mt-0.5">{description}</p>}
				{trend?.label && (
					<p className="text-[10px] text-on-surface-variant/50 mt-0.5">{trend.label}</p>
				)}
			</div>
			{href && (
				<Icon
					name="arrow_forward"
					size={20}
					className="text-on-surface-variant/50 group-hover:text-on-surface transition-colors shrink-0"
				/>
			)}
		</div>
	);

	if (href) {
		return <Link href={href}>{content}</Link>;
	}

	return content;
}
