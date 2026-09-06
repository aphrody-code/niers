import { format } from "date-fns";
import { fr } from "date-fns/locale";
import { Newspaper } from "lucide-react";
import { Image } from "../../../compat/next";
import { Link } from "../../../compat/next";
import { Badge } from "../../../components/ui/badge";

const CATEGORY_LABELS: Record<string, string> = {
	announcement: "Annonce",
	community: "Communauté",
	event: "Événement",
};

interface RelatedArticle {
	id: string;
	title: string;
	slug: string;
	excerpt: string | null;
	featured_image_url: string | null;
	published_at: string | null;
	category: string | null;
}

interface RelatedArticlesProps {
	articles: RelatedArticle[];
}

export function RelatedArticles({ articles }: RelatedArticlesProps) {
	if (articles.length === 0) {
		return null;
	}

	return (
		<section className="border-t border-outline-variant/20 pt-8">
			<div className="flex items-center gap-2 mb-6">
				<Newspaper className="size-5 text-primary" />
				<h2 className="text-lg font-bold font-sans tracking-tight text-on-surface">
					Articles similaires
				</h2>
			</div>
			<div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
				{articles.map((article) => (
					<Link
						key={article.id}
						href={`/news/${article.slug}`}
						className="group rounded-2xl overflow-hidden bg-surface-container-lowest border border-outline-variant/20 hover:shadow-lg transition-all duration-300"
					>
						<div className="relative aspect-[16/9] bg-surface-container">
							<Image
								src={article.featured_image_url || "/images/placeholder-news.svg"}
								alt={article.title}
								fill
								className="object-cover group-hover:scale-105 transition-transform duration-500"
								sizes="(max-width: 640px) 100vw, (max-width: 1024px) 50vw, 25vw"
							/>
						</div>
						<div className="p-3 space-y-1.5">
							<div className="flex items-center gap-2">
								{article.category && (
									<Badge className="bg-primary/10 text-primary border-none text-[10px] rounded-lg">
										{CATEGORY_LABELS[article.category] || article.category}
									</Badge>
								)}
								{article.published_at && (
									<span className="text-[10px] text-on-surface-variant">
										{format(new Date(article.published_at), "d MMM yyyy", { locale: fr })}
									</span>
								)}
							</div>
							<h3 className="text-sm font-bold text-on-surface group-hover:text-primary transition-colors line-clamp-2">
								{article.title}
							</h3>
							{article.excerpt && (
								<p className="text-xs text-on-surface-variant line-clamp-2">{article.excerpt}</p>
							)}
						</div>
					</Link>
				))}
			</div>
		</section>
	);
}
