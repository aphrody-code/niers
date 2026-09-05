"use client";

import { Image } from "@/components/ui/image";
import { CommonSpriteIcon } from "@/components/ui/CommonSpriteIcon";
import { Icon } from "@/components/ui/Icon";
import type { SpriteCommonKey } from "@/config/sprites-common";
import { useFilterNavigation } from "@/lib/hooks/use-filter-navigation";
import { cn } from "@/lib/utils";

export interface FilterOption {
	label: string;
	value: string;
	icon?: string;
	imageIcon?: string;
	commonSprite?: string;
}

export interface FilterChipsProps {
	paramName: string;
	options: FilterOption[];
	className?: string;
	hideLabel?: boolean;
}

export function FilterChips({ paramName, options, className, hideLabel }: FilterChipsProps) {
	const { isPending, navigate, searchParams } = useFilterNavigation();
	const currentValue = searchParams.get(paramName);

	const toggleFilter = (value: string) => {
		navigate((params) => {
			if (params.get(paramName) === value) {
				params.delete(paramName);
			} else {
				params.set(paramName, value);
			}
		});
	};

	return (
		<div
			className={cn(
				"flex flex-wrap gap-2",
				isPending && "pointer-events-none opacity-70",
				className
			)}
		>
			{options.map((option) => {
				const isSelected = currentValue === option.value;
				const isImageOnly = hideLabel && (option.imageIcon || option.commonSprite || option.icon);

				return (
					<button
						key={option.value}
						onClick={() => toggleFilter(option.value)}
						title={option.label}
						aria-pressed={isSelected}
						className={cn(
							// MD3 Filter Chip styling
							"inline-flex items-center gap-1.5 transition-all duration-200 ease-[cubic-bezier(0.2,0,0,1)]",
							"focus-visible:outline-hidden focus-visible:ring-2 focus-visible:ring-primary/50",
							isImageOnly
								? "p-1 bg-transparent border-none hover:scale-110 active:scale-95 min-h-11 min-w-11 justify-center"
								: "px-4 py-2 rounded-full text-sm font-medium border min-h-11 sm:min-h-0",
							!isImageOnly &&
								(isSelected
									? "bg-secondary-container text-on-secondary-container border-transparent"
									: "bg-surface text-on-surface-variant border-outline-variant hover:bg-on-surface/[0.08]"),
							isImageOnly &&
								isSelected &&
								"brightness-125 scale-110 drop-shadow-[0_0_8px_rgba(var(--md-sys-color-primary-rgb),0.5)]"
						)}
					>
						{option.imageIcon ? (
							<div className={cn("relative", isImageOnly ? "size-10" : "size-6")}>
								<Image
									src={option.imageIcon}
									fill
									alt={option.label}
									sizes={isImageOnly ? "40px" : "24px"}
									className="object-contain"
								/>
							</div>
						) : option.commonSprite ? (
							<CommonSpriteIcon
								name={option.commonSprite as SpriteCommonKey}
								scale={isImageOnly ? 0.6 : 0.4}
							/>
						) : option.icon ? (
							<Icon name={option.icon} size={isImageOnly ? 28 : 20} />
						) : null}

						{!hideLabel && option.label}
					</button>
				);
			})}
		</div>
	);
}
