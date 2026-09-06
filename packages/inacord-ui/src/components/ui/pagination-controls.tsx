"use client";

import { ChevronLeft, ChevronRight } from "lucide-react";
import { Link } from "../../compat/next";
import { useSearchParams } from "../../compat/next";
import type * as React from "react";

import { Button } from "./button";

interface PaginationControlsProps {
	currentPage: number;
	totalPages: number;
	baseUrl: string;
	previousLabel?: React.ReactNode;
	nextLabel?: React.ReactNode;
	pageLabel?: (current: number, total: number) => React.ReactNode;
	LinkComponent?: React.ComponentType<{
		href: string;
		className?: string;
		children?: React.ReactNode;
	}>;
}

const defaultPageLabel = (current: number, total: number) => `Page ${current} sur ${total}`;

export function PaginationControls({
	currentPage,
	totalPages,
	baseUrl,
	previousLabel = "Précédent",
	nextLabel = "Suivant",
	pageLabel = defaultPageLabel,
	LinkComponent = Link,
}: PaginationControlsProps) {
	const searchParams = useSearchParams();

	const createPageUrl = (page: number) => {
		const params = new URLSearchParams(searchParams?.toString() || "");
		params.set("page", page.toString());
		return `${baseUrl}?${params.toString()}`;
	};

	// Une seule page : la barre n'apporte rien et coûte ~56px de chrome vide.
	if (totalPages <= 1) {
		return null;
	}

	const isFirst = currentPage <= 1;
	const isLast = currentPage >= totalPages;

	// `disabled` sur un `<Button asChild>` atterrit sur un `<a>`, où l'attribut
	// n'existe pas et où `:disabled` ne matche jamais : les deux boutons
	// restaient cliquables ET non grisés, et « Précédent » en page 1 menait à
	// `?page=0` → plage négative → écran vide sans message. On rend donc un
	// `<span>` inerte quand la cible est hors bornes.
	const inactiveClass =
		"inline-flex h-8 items-center gap-2 rounded-md border border-input px-3 text-sm font-medium opacity-50";

	return (
		<div className="flex items-center justify-between gap-4">
			<p className="text-sm text-muted-foreground">{pageLabel(currentPage, totalPages)}</p>
			<div className="flex gap-2">
				{isFirst ? (
					<span className={inactiveClass} aria-disabled="true">
						<ChevronLeft className="size-4" />
						{previousLabel}
					</span>
				) : (
					<Button
						variant="outline"
						size="sm"
						render={
							<LinkComponent href={createPageUrl(currentPage - 1)}>
								<ChevronLeft className="size-4 mr-2" />
								{previousLabel}
							</LinkComponent>
						}
					/>
				)}
				{isLast ? (
					<span className={inactiveClass} aria-disabled="true">
						{nextLabel}
						<ChevronRight className="size-4" />
					</span>
				) : (
					<Button
						variant="outline"
						size="sm"
						render={
							<LinkComponent href={createPageUrl(currentPage + 1)}>
								{nextLabel}
								<ChevronRight className="size-4 ml-2" />
							</LinkComponent>
						}
					/>
				)}
			</div>
		</div>
	);
}
