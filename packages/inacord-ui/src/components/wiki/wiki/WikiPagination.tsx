"use client";

import { Link } from "../../../compat/next";
import { usePathname, useSearchParams } from "../../../compat/next";
import { Pagination, PaginationContent, PaginationEllipsis, PaginationItem, PaginationLink, PaginationNext, PaginationPrevious } from "../../../components/ui/pagination";
import { cn } from "../../../lib/utils";

interface WikiPaginationProps {
	totalItems: number;
	itemsPerPage: number;
	currentPage: number;
	perPageOptions?: number[];
}

export function WikiPagination({
	totalItems,
	itemsPerPage,
	currentPage,
	perPageOptions,
}: WikiPaginationProps) {
	const pathname = usePathname();
	const searchParams = useSearchParams();
	const totalPages = Math.ceil(totalItems / itemsPerPage);

	const createPageUrl = (pageNumber: number, perPage?: number) => {
		const params = new URLSearchParams(searchParams.toString());
		params.set("page", pageNumber.toString());
		if (perPage) {
			params.set("perPage", perPage.toString());
		}
		return `${pathname}?${params.toString()}`;
	};

	const createPerPageUrl = (perPage: number) => {
		const params = new URLSearchParams(searchParams.toString());
		params.set("perPage", perPage.toString());
		params.delete("page");
		return `${pathname}?${params.toString()}`;
	};

	// Show all pages if total is small enough, otherwise show a wide window
	const getVisiblePages = (): Array<number | "ellipsis"> => {
		if (totalPages <= 20) {
			return Array.from({ length: totalPages }, (_, i) => i + 1);
		}

		const pages: Array<number | "ellipsis"> = [];
		const windowSize = 4;

		const start = Math.max(3, currentPage - windowSize);
		const end = Math.min(totalPages - 2, currentPage + windowSize);

		pages.push(1, 2);
		if (start > 3) {
			pages.push("ellipsis");
		}

		for (let i = start; i <= end; i++) {
			pages.push(i);
		}

		if (end < totalPages - 2) {
			pages.push("ellipsis");
		}
		pages.push(totalPages - 1, totalPages);

		return pages;
	};

	const showPerPage = perPageOptions && perPageOptions.length > 1;

	return (
		<div className="flex flex-col items-center gap-4 pt-8">
			{/* Per-page selector */}
			{showPerPage && (
				<div className="flex items-center gap-2 text-xs text-on-surface-variant">
					<span className="font-medium">Par page :</span>
					<div className="flex gap-1">
						{perPageOptions.map((opt) => (
							<Link
								key={opt}
								href={createPerPageUrl(opt)}
								scroll={false}
								prefetch
								aria-current={opt === itemsPerPage ? "page" : undefined}
								className={cn(
									"inline-flex items-center justify-center min-h-11 sm:min-h-0 px-2.5 py-1 rounded-full text-xs font-medium transition-colors",
									opt === itemsPerPage
										? "bg-primary text-on-primary"
										: "bg-surface-container-high text-on-surface-variant hover:bg-surface-container-highest"
								)}
							>
								{opt}
							</Link>
						))}
					</div>
				</div>
			)}

			{/* Page navigation */}
			{totalPages > 1 && (
				<Pagination>
					<PaginationContent className="flex-wrap gap-1">
						<PaginationItem>
							<PaginationPrevious
								href={currentPage > 1 ? createPageUrl(currentPage - 1) : "#"}
								aria-disabled={currentPage <= 1}
								className={currentPage <= 1 ? "pointer-events-none opacity-50" : ""}
							/>
						</PaginationItem>

						{getVisiblePages().map((page, index) => (
							<PaginationItem key={`page-${index}`}>
								{page === "ellipsis" ? (
									<PaginationEllipsis />
								) : (
									<PaginationLink href={createPageUrl(page)} isActive={page === currentPage}>
										{page}
									</PaginationLink>
								)}
							</PaginationItem>
						))}

						<PaginationItem>
							<PaginationNext
								href={currentPage < totalPages ? createPageUrl(currentPage + 1) : "#"}
								aria-disabled={currentPage >= totalPages}
								className={currentPage >= totalPages ? "pointer-events-none opacity-50" : ""}
							/>
						</PaginationItem>
					</PaginationContent>
				</Pagination>
			)}
		</div>
	);
}
