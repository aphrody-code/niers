import { Skeleton } from "../../../components/ui/skeleton";

/**
 * Loading skeleton for wiki list pages (characters, skills, items, etc.)
 * Shows a title bar + grid of card placeholders.
 */
export function WikiListSkeleton({
	columns = 4,
	aspectRatio = "square",
}: {
	columns?: number;
	aspectRatio?: "square" | "video";
}) {
	const gridCols =
		columns === 3
			? "grid-cols-1 sm:grid-cols-2 lg:grid-cols-3"
			: "grid-cols-2 sm:grid-cols-3 lg:grid-cols-4";

	const aspectClass = aspectRatio === "video" ? "aspect-[16/9]" : "aspect-square";

	return (
		<div className="space-y-6">
			{/* Filter bar skeleton */}
			<div className="flex items-center gap-3 flex-wrap">
				<Skeleton className="h-10 w-48 rounded-full bg-surface-container-highest" />
				<Skeleton className="h-8 w-20 rounded-full bg-surface-container-highest" />
				<Skeleton className="h-8 w-24 rounded-full bg-surface-container-highest" />
				<Skeleton className="h-8 w-20 rounded-full bg-surface-container-highest" />
			</div>

			{/* Grid skeleton */}
			<div className={`grid ${gridCols} gap-3`}>
				{Array.from({ length: 12 }).map((_, i) => (
					<div key={i} className="rounded-2xl bg-surface-container-low overflow-hidden">
						<Skeleton className={`w-full ${aspectClass} bg-surface-container-highest`} />
						<div className="p-3 space-y-2">
							<Skeleton className="h-4 w-3/4 bg-surface-container-highest rounded-md" />
							<Skeleton className="h-3 w-1/2 bg-surface-container-highest rounded-md" />
						</div>
					</div>
				))}
			</div>
		</div>
	);
}

/**
 * Loading skeleton for wiki detail pages (character detail, skill detail, etc.)
 * Shows a hero area + content blocks.
 */
export function WikiDetailSkeleton() {
	return (
		<div className="space-y-8">
			{/* Hero / Header */}
			<div className="flex flex-col md:flex-row gap-6">
				<Skeleton className="size-32 md:size-48 rounded-3xl bg-surface-container-highest shrink-0" />
				<div className="flex-1 space-y-3 pt-2">
					<Skeleton className="h-8 w-64 bg-surface-container-highest rounded-lg" />
					<div className="flex gap-2">
						<Skeleton className="h-6 w-20 rounded-full bg-surface-container-highest" />
						<Skeleton className="h-6 w-16 rounded-full bg-surface-container-highest" />
						<Skeleton className="h-6 w-24 rounded-full bg-surface-container-highest" />
					</div>
					<Skeleton className="h-4 w-full bg-surface-container-highest rounded-md" />
					<Skeleton className="h-4 w-3/4 bg-surface-container-highest rounded-md" />
				</div>
			</div>

			{/* Content blocks */}
			<div className="space-y-4">
				<Skeleton className="h-6 w-40 bg-surface-container-highest rounded-md" />
				<div className="grid grid-cols-2 md:grid-cols-4 gap-3">
					{Array.from({ length: 4 }).map((_, i) => (
						<Skeleton key={i} className="h-20 rounded-2xl bg-surface-container-highest" />
					))}
				</div>
			</div>

			<div className="space-y-4">
				<Skeleton className="h-6 w-32 bg-surface-container-highest rounded-md" />
				<Skeleton className="h-40 w-full rounded-2xl bg-surface-container-highest" />
			</div>
		</div>
	);
}

/**
 * Loading skeleton for hub/navigation pages (wiki hub, aura hub, etc.)
 * Shows a title + grid of section cards.
 */
export function WikiHubSkeleton() {
	return (
		<div className="space-y-6">
			<Skeleton className="h-8 w-48 bg-surface-container-highest rounded-lg" />
			<div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
				{Array.from({ length: 6 }).map((_, i) => (
					<div key={i} className="flex items-center gap-4 p-4 rounded-2xl bg-surface-container-low">
						<Skeleton className="size-12 rounded-xl bg-surface-container-highest shrink-0" />
						<div className="flex-1 space-y-2">
							<Skeleton className="h-5 w-32 bg-surface-container-highest rounded-md" />
							<Skeleton className="h-3 w-20 bg-surface-container-highest rounded-md" />
						</div>
					</div>
				))}
			</div>
		</div>
	);
}

/**
 * Loading skeleton for news/article list pages.
 */
export function NewsListSkeleton() {
	return (
		<div className="space-y-4">
			<Skeleton className="h-8 w-48 bg-surface-container-highest rounded-lg" />
			{Array.from({ length: 5 }).map((_, i) => (
				<div key={i} className="flex gap-4 p-4 rounded-2xl bg-surface-container-low">
					<Skeleton className="w-24 h-16 rounded-xl bg-surface-container-highest shrink-0" />
					<div className="flex-1 space-y-2">
						<Skeleton className="h-5 w-3/4 bg-surface-container-highest rounded-md" />
						<Skeleton className="h-3 w-1/2 bg-surface-container-highest rounded-md" />
						<Skeleton className="h-3 w-24 bg-surface-container-highest rounded-md" />
					</div>
				</div>
			))}
		</div>
	);
}
