"use client";

import { Eye, TrendingUp } from "lucide-react";
import { Link } from "../../../compat/next";

const CATEGORY_LABELS: Record<string, string> = {
	announcement: "Annonce",
	community: "Communauté",
	event: "Événement",
};

interface PopularArticlesProps {
	articles: Array<{
		id: string;
		title: string;
		slug: string;
		category: string | null;
		view_count: number;
	}>;
}

export function PopularArticles({ articles }: PopularArticlesProps) {
	if (articles.length === 0) {
		return null;
	}

	return (
		<div className="p-5 rounded-2xl bg-surface-container-low border border-outline-variant/10">
			<div className="flex items-center gap-2 mb-4">
				<TrendingUp className="size-4 text-primary" />
				<h3 className="text-xs font-bold uppercase tracking-wider text-on-surface-variant/60">
					Articles populaires
				</h3>
			</div>
			<div className="space-y-3">
				{articles.map((article, index) => (
					<Link
						key={article.id}
						href={`/news/${article.slug}`}
						className="flex items-start gap-3 group"
					>
						<span className="text-lg font-black text-on-surface-variant/20 w-6 shrink-0 tabular-nums">
							{index + 1}
						</span>
						<div className="min-w-0 flex-1">
							<p className="text-sm font-medium text-on-surface line-clamp-2 group-hover:text-primary transition-colors">
								{article.title}
							</p>
							<div className="flex items-center gap-2 mt-1">
								{article.category && (
									<span className="text-[10px] font-semibold text-primary/70 uppercase">
										{CATEGORY_LABELS[article.category] || article.category}
									</span>
								)}
								<span className="inline-flex items-center gap-0.5 text-[10px] text-on-surface-variant/50">
									<Eye className="size-3" />
									{article.view_count}
								</span>
							</div>
						</div>
					</Link>
				))}
			</div>
		</div>
	);
}
