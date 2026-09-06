"use client";

import { useRouter, useSearchParams } from "../../../../compat/next";
import { useCallback } from "react";
import { Icon } from "../../../../components/wiki/ui/Icon";
import { cn } from "../../../../lib/utils";

export interface GalleryCategoryOption {
	value: string;
	label: string;
	icon?: string;
	count?: number;
}

interface GalleryFilterBarProps {
	categories: GalleryCategoryOption[];
	currentCategory: string;
}

export function GalleryFilterBar({ categories, currentCategory }: GalleryFilterBarProps) {
	const router = useRouter();
	const searchParams = useSearchParams();

	const updateCategory = useCallback(
		(value: string) => {
			const params = new URLSearchParams(searchParams.toString());
			if (value && value !== "all") {
				params.set("category", value);
			} else {
				params.delete("category");
			}
			params.delete("page");
			const qs = params.toString();
			router.push(qs ? `/gallery?${qs}` : "/gallery");
		},
		[router, searchParams]
	);

	return (
		<div className="flex flex-wrap gap-2">
			{categories.map((f) => {
				const isActive = currentCategory === f.value || (f.value === "all" && !currentCategory);
				return (
					<button
						key={f.value}
						type="button"
						onClick={() => updateCategory(f.value)}
						className={cn(
							"inline-flex items-center justify-center gap-2 rounded-full px-4 py-2 text-sm font-medium",
							"min-h-11 sm:min-h-0 cursor-pointer border transition-all duration-200",
							isActive
								? "border-primary bg-primary text-on-primary shadow-md"
								: "border-outline-variant/30 bg-surface-container text-on-surface-variant hover:bg-surface-container-high hover:text-on-surface"
						)}
					>
						{f.icon && <Icon name={f.icon} size={18} />}
						<span>{f.label}</span>
						{typeof f.count === "number" && (
							<span
								className={cn(
									"rounded-full px-1.5 text-[11px] font-bold",
									isActive ? "bg-on-primary/20" : "bg-surface-container-highest/60"
								)}
							>
								{f.count}
							</span>
						)}
					</button>
				);
			})}
		</div>
	);
}
