"use client";

import { useRouter, useSearchParams } from "../../../../compat/next";
import { useCallback } from "react";
import { Icon } from "../../../../components/wiki/ui/Icon";
import { cn } from "../../../../lib/utils";

export interface ItemCategoryOption {
	value: string;
	label: string;
	icon?: string;
}

interface ItemFilterBarProps {
	categories: ItemCategoryOption[];
	currentCategory: string;
}

export function ItemFilterBar({ categories, currentCategory }: ItemFilterBarProps) {
	const router = useRouter();
	const searchParams = useSearchParams();

	const updateCategory = useCallback(
		(value: string) => {
			const params = new URLSearchParams(searchParams.toString());
			if (value) {
				params.set("category", value);
			} else {
				params.delete("category");
			}
			params.delete("page");
			router.push(`/item?${params.toString()}`);
		},
		[router, searchParams]
	);

	return (
		<div className="space-y-4">
			{/* Category Chips */}
			<div className="flex flex-wrap gap-2">
				{categories.map((f) => {
					const isActive = currentCategory === f.value;
					return (
						<button
							key={f.value}
							onClick={() => updateCategory(f.value)}
							className={cn(
								"inline-flex items-center justify-center gap-2 px-4 py-2 rounded-full text-sm font-medium",
								// Puces dimensionnées au contenu : avec ~19 catégories le retour à la ligne
								// reste lisible (pas d'étirement flex-1 qui déforme les rangées).
								"min-h-11 sm:min-h-0 transition-all duration-200 border cursor-pointer",
								isActive
									? "bg-primary text-on-primary border-primary shadow-md"
									: "bg-surface-container hover:bg-surface-container-high border-outline-variant/30 text-on-surface-variant hover:text-on-surface"
							)}
						>
							{f.icon && <Icon name={f.icon} size={18} />}
							<span>{f.label}</span>
						</button>
					);
				})}
			</div>
		</div>
	);
}
