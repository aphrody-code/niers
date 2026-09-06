import { BookOpen } from "lucide-react";
import { Image } from "../../../compat/next";
import { Link } from "../../../compat/next";

const CATEGORY_LABELS: Record<string, string> = {
	announcement: "Annonce",
	community: "Communauté",
	event: "Événement",
};

interface ContinueReadingArticle {
	article_id: string;
	title: string;
	slug: string;
	featured_image_url: string | null;
	category: string | null;
	progress: number;
	last_read_at: string;
}

interface ContinueReadingProps {
	articles: ContinueReadingArticle[];
}

export function ContinueReading({ articles }: ContinueReadingProps) {
	if (articles.length === 0) {
		return null;
	}

	return (
		<section>
			<div className="flex items-center gap-2 mb-4">
				<BookOpen className="size-4 text-primary" />
				<h2 className="text-sm font-bold text-on-surface">Continuer la lecture</h2>
			</div>

			<div className="flex gap-3 overflow-x-auto scrollbar-none pb-1">
				{articles.map((article) => (
					<div
						key={article.article_id}
						className="shrink-0 w-52 rounded-2xl overflow-hidden bg-surface-container border border-outline-variant/20 flex flex-col"
					>
						{/* Image */}
						<div className="relative aspect-[16/9] bg-surface-container-high">
							<Image
								src={article.featured_image_url || "/images/placeholder-news.svg"}
								alt={article.title}
								fill
								className="object-cover"
								sizes="208px"
							/>
						</div>

						{/* Contenu */}
						<div className="p-3 flex flex-col gap-2 flex-1">
							{article.category && (
								<span className="text-[10px] font-semibold text-primary/70 uppercase tracking-wide">
									{CATEGORY_LABELS[article.category] || article.category}
								</span>
							)}

							<p className="text-xs font-semibold text-on-surface line-clamp-3 leading-snug flex-1">
								{article.title}
							</p>

							{/* Barre de progression */}
							<div className="space-y-1">
								<div className="h-1 w-full bg-surface-container-high rounded-full overflow-hidden">
									<div
										className="h-full bg-primary rounded-full transition-[width]"
										style={{ width: `${Math.min(article.progress, 100)}%` }}
									/>
								</div>
								<span className="text-[10px] text-on-surface-variant/50 tabular-nums">
									{article.progress}% lu
								</span>
							</div>

							<Link
								href={`/news/${article.slug}`}
								className="inline-flex items-center justify-center px-3 py-1.5 rounded-xl bg-primary text-on-primary text-xs font-semibold hover:opacity-90 transition-opacity"
							>
								Continuer
							</Link>
						</div>
					</div>
				))}
			</div>
		</section>
	);
}
