import { ChevronLeftIcon, ChevronRightIcon, MoreHorizontalIcon } from "lucide-react";
import type * as React from "react";

import { type Button, buttonVariants } from "./button";
import { cn } from "../../lib/utils";

function Pagination({ className, ...props }: React.ComponentProps<"nav">) {
	return (
		<nav
			aria-label="pagination"
			data-slot="pagination"
			className={cn("mx-auto flex w-full justify-center", className)}
			{...props}
		/>
	);
}

function PaginationContent({ className, ...props }: React.ComponentProps<"ul">) {
	return (
		<ul
			data-slot="pagination-content"
			className={cn("flex flex-row items-center gap-1", className)}
			{...props}
		/>
	);
}

function PaginationItem({ ...props }: React.ComponentProps<"li">) {
	return <li data-slot="pagination-item" {...props} />;
}

type PaginationLinkProps = {
	isActive?: boolean;
} & Pick<React.ComponentProps<typeof Button>, "size"> &
	React.ComponentProps<"a">;

function PaginationLink({ className, isActive, size = "icon", ...props }: PaginationLinkProps) {
	return (
		<a
			aria-current={isActive ? "page" : undefined}
			data-slot="pagination-link"
			data-active={isActive}
			className={cn(
				buttonVariants({
					variant: isActive ? "outline" : "ghost",
					size,
				}),
				className
			)}
			{...props}
		/>
	);
}

type PaginationLabels = {
	previousAria?: string;
	previousLabel?: React.ReactNode;
	nextAria?: string;
	nextLabel?: React.ReactNode;
	morePagesLabel?: string;
};

function PaginationPrevious({
	className,
	previousAria = "Aller à la page précédente",
	previousLabel = "Précédent",
	...props
}: React.ComponentProps<typeof PaginationLink> &
	Pick<PaginationLabels, "previousAria" | "previousLabel">) {
	return (
		<PaginationLink
			aria-label={previousAria}
			size="default"
			className={cn("gap-1 px-2.5 sm:pl-2.5", className)}
			{...props}
		>
			<ChevronLeftIcon />
			<span className="hidden sm:block">{previousLabel}</span>
		</PaginationLink>
	);
}

function PaginationNext({
	className,
	nextAria = "Aller à la page suivante",
	nextLabel = "Suivant",
	...props
}: React.ComponentProps<typeof PaginationLink> & Pick<PaginationLabels, "nextAria" | "nextLabel">) {
	return (
		<PaginationLink
			aria-label={nextAria}
			size="default"
			className={cn("gap-1 px-2.5 sm:pr-2.5", className)}
			{...props}
		>
			<span className="hidden sm:block">{nextLabel}</span>
			<ChevronRightIcon />
		</PaginationLink>
	);
}

function PaginationEllipsis({
	className,
	morePagesLabel = "Plus de pages",
	...props
}: React.ComponentProps<"span"> & Pick<PaginationLabels, "morePagesLabel">) {
	return (
		<span
			aria-hidden
			data-slot="pagination-ellipsis"
			className={cn("flex size-9 items-center justify-center", className)}
			{...props}
		>
			<MoreHorizontalIcon className="size-4" />
			<span className="sr-only">{morePagesLabel}</span>
		</span>
	);
}

export {
	Pagination,
	PaginationContent,
	PaginationEllipsis,
	PaginationItem,
	PaginationLink,
	PaginationNext,
	PaginationPrevious,
};
