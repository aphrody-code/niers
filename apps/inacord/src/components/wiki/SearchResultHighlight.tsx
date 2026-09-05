/**
 * SearchResultHighlight - Highlight matching text in search results
 *
 * Material Design 3 component for highlighting matched portions of text
 */

import { highlightMatches } from "@/lib/wikiTypes";
import { cn } from "@/lib/utils";

interface SearchResultHighlightProps {
	text: string;
	query: string;
	className?: string;
}

export function SearchResultHighlight({ text, query, className }: SearchResultHighlightProps) {
	const parts = highlightMatches(text, query);

	return (
		<span className={cn("inline", className)}>
			{parts.map((part, index) =>
				part.highlight ? (
					<mark
						key={index}
						className="bg-[var(--md-sys-color-tertiary-container)] text-[var(--md-sys-color-on-tertiary-container)] rounded-sm px-0.5 font-semibold"
					>
						{part.text}
					</mark>
				) : (
					<span key={index}>{part.text}</span>
				)
			)}
		</span>
	);
}
