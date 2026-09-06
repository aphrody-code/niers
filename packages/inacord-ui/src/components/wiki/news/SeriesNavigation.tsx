"use client";

import { BookOpen, ChevronLeft, ChevronRight } from "lucide-react";
import { Link } from "../../../compat/next";
import { useRouter } from "../../../compat/next";
import { cn } from "../../../lib/utils";

interface SeriesArticle {
	id: string;
	title: string;
	slug: string;
	series_order: number | null;
}

interface SeriesNavigationProps {
	series: {
		id: string;
		title: string;
		slug: string;
		description: string | null;
	};
	articles: SeriesArticle[];
	currentIndex: number;
}

export function SeriesNavigation({ series, articles, currentIndex }: SeriesNavigationProps) {
	const router = useRouter();
	const sorted = [...articles].toSorted((a, b) => (a.series_order ?? 0) - (b.series_order ?? 0));
	const total = sorted.length;
	const prevArticle = currentIndex > 0 ? sorted[currentIndex - 1] : null;
	const nextArticle = currentIndex < total - 1 ? sorted[currentIndex + 1] : null;

	return (
		<div className="rounded-2xl bg-surface-container-low border border-outline-variant/10 overflow-hidden">
			{/* En-tête série */}
			<div className="p-4 border-b border-outline-variant/10">
				<div className="flex items-center gap-2 mb-1">
					<BookOpen className="size-4 text-primary shrink-0" />
					<span className="text-xs font-bold uppercase tracking-wider text-on-surface-variant/60">
						Série
					</span>
				</div>
				<p className="text-sm font-bold text-on-surface leading-snug">{series.title}</p>
				{series.description && (
					<p className="text-xs text-on-surface-variant/60 mt-1 line-clamp-2">
						{series.description}
					</p>
				)}
				<p className="text-xs text-on-surface-variant/50 mt-2">
					Article {currentIndex + 1} sur {total}
				</p>
			</div>

			{/* Liste des articles */}
			<div className="divide-y divide-outline-variant/10">
				{sorted.map((article, index) => {
					const isCurrent = index === currentIndex;
					return (
						<Link
							key={article.id}
							href={`/news/${article.slug}`}
							className={cn(
								"flex items-center gap-3 px-4 py-3 transition-colors",
								isCurrent
									? "bg-primary/10 cursor-default pointer-events-none"
									: "hover:bg-surface-container"
							)}
						>
							<span
								className={cn(
									"text-xs font-black tabular-nums w-5 shrink-0",
									isCurrent ? "text-primary" : "text-on-surface-variant/30"
								)}
							>
								{article.series_order}
							</span>
							<p
								className={cn(
									"text-xs font-medium line-clamp-2 leading-snug",
									isCurrent ? "text-primary" : "text-on-surface-variant hover:text-on-surface"
								)}
							>
								{article.title}
							</p>
							{isCurrent && <span className="ml-auto shrink-0 size-1.5 rounded-full bg-primary" />}
						</Link>
					);
				})}
			</div>

			{/* Navigation précédent / suivant */}
			{(prevArticle || nextArticle) && (
				<div className="flex border-t border-outline-variant/10">
					<button
						onClick={() => prevArticle && router.push(`/news/${prevArticle.slug}`)}
						disabled={!prevArticle}
						className={cn(
							"flex-1 flex items-center gap-1.5 px-3 py-3 text-xs font-medium transition-colors",
							prevArticle
								? "text-on-surface-variant hover:text-primary hover:bg-surface-container"
								: "text-on-surface-variant/30 cursor-not-allowed"
						)}
					>
						<ChevronLeft className="size-3.5 shrink-0" />
						<span className="truncate">{prevArticle ? prevArticle.title : "Début"}</span>
					</button>

					<div className="w-px bg-outline-variant/20" />

					<button
						onClick={() => nextArticle && router.push(`/news/${nextArticle.slug}`)}
						disabled={!nextArticle}
						className={cn(
							"flex-1 flex items-center justify-end gap-1.5 px-3 py-3 text-xs font-medium transition-colors",
							nextArticle
								? "text-on-surface-variant hover:text-primary hover:bg-surface-container"
								: "text-on-surface-variant/30 cursor-not-allowed"
						)}
					>
						<span className="truncate text-right">{nextArticle ? nextArticle.title : "Fin"}</span>
						<ChevronRight className="size-3.5 shrink-0" />
					</button>
				</div>
			)}
		</div>
	);
}
