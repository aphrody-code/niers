"use client";

import { Link } from "../../../compat/next";

interface TagCloudProps {
	tags: Array<{ tag: string; count: number }>;
}

export function TagCloud({ tags }: TagCloudProps) {
	if (tags.length === 0) {
		return null;
	}

	const maxCount = Math.max(...tags.map((t) => t.count));
	const minCount = Math.min(...tags.map((t) => t.count));
	const range = maxCount - minCount || 1;

	const getSize = (count: number) => {
		const ratio = (count - minCount) / range;
		// Text-xs (0.75rem) to text-lg (1.125rem)
		return 0.75 + ratio * 0.375;
	};

	return (
		<div className="p-5 rounded-2xl bg-surface-container-low border border-outline-variant/10">
			<h3 className="text-xs font-bold uppercase tracking-wider text-on-surface-variant/60 mb-3">
				Tags populaires
			</h3>
			<div className="flex flex-wrap gap-2">
				{tags.map(({ tag, count }) => (
					<Link
						key={tag}
						href={`/news/tag/${encodeURIComponent(tag)}`}
						className="inline-block rounded-lg bg-secondary-container/60 text-on-secondary-container px-2.5 py-1 hover:bg-secondary-container transition-colors"
						style={{ fontSize: `${getSize(count)}rem` }}
						title={`${count} article${count > 1 ? "s" : ""}`}
					>
						{tag}
					</Link>
				))}
			</div>
		</div>
	);
}
