"use client";

// Material Design 3 — Floating Action Button (FAB)
// Spec : https://m3.material.io/components/floating-action-button/specs
// 4 color variants (surface | primary | secondary | tertiary)
// 3 sizes (small | medium | large) + extended variant (icon + label)
// Elevation level3 default, level4 au hover ; shape extra-large (28px) sauf
// small qui passe en large (16px). Motion : easing-emphasized / duration-short4.

import { cva, type VariantProps } from "class-variance-authority";
import type * as React from "react";

import { cn } from "../../lib/utils";

const fabVariants = cva(
	"relative inline-flex shrink-0 items-center justify-center transition-[box-shadow,background-color,color,transform] duration-(--md-sys-motion-duration-short4) ease-(--md-sys-motion-easing-emphasized) shadow-(--md-sys-elevation-level3) hover:shadow-(--md-sys-elevation-level4) active:shadow-(--md-sys-elevation-level3) disabled:pointer-events-none disabled:opacity-50 outline-hidden focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px] [&_svg]:pointer-events-none [&_svg]:shrink-0",
	{
		variants: {
			variant: {
				surface: "bg-surface-container-high text-primary hover:bg-surface-container-highest",
				primary:
					"bg-primary-container text-on-primary-container hover:brightness-95 dark:hover:brightness-110",
				secondary:
					"bg-secondary-container text-on-secondary-container hover:brightness-95 dark:hover:brightness-110",
				tertiary:
					"bg-tertiary-container text-on-tertiary-container hover:brightness-95 dark:hover:brightness-110",
			},
			size: {
				small: "size-10 rounded-(--md-sys-shape-corner-large) [&_svg:not([class*='size-'])]:size-6",
				medium:
					"size-14 rounded-(--md-sys-shape-corner-extra-large) [&_svg:not([class*='size-'])]:size-6",
				large:
					"size-24 rounded-(--md-sys-shape-corner-extra-large) [&_svg:not([class*='size-'])]:size-9",
			},
		},
		defaultVariants: {
			variant: "surface",
			size: "medium",
		},
	}
);

type FabProps = React.ComponentProps<"button"> & VariantProps<typeof fabVariants>;

function Fab({ className, variant, size, type = "button", ...props }: FabProps) {
	return (
		<button
			data-slot="fab"
			data-variant={variant ?? "surface"}
			data-size={size ?? "medium"}
			type={type}
			className={cn(fabVariants({ variant, size }), className)}
			{...props}
		/>
	);
}

const fabExtendedVariants = cva(
	"relative inline-flex h-14 shrink-0 items-center justify-center gap-3 rounded-(--md-sys-shape-corner-extra-large) px-4 text-sm font-medium transition-[box-shadow,background-color,color,transform] duration-(--md-sys-motion-duration-short4) ease-(--md-sys-motion-easing-emphasized) shadow-(--md-sys-elevation-level3) hover:shadow-(--md-sys-elevation-level4) active:shadow-(--md-sys-elevation-level3) disabled:pointer-events-none disabled:opacity-50 outline-hidden focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px] [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-6",
	{
		variants: {
			variant: {
				surface: "bg-surface-container-high text-primary hover:bg-surface-container-highest",
				primary:
					"bg-primary-container text-on-primary-container hover:brightness-95 dark:hover:brightness-110",
				secondary:
					"bg-secondary-container text-on-secondary-container hover:brightness-95 dark:hover:brightness-110",
				tertiary:
					"bg-tertiary-container text-on-tertiary-container hover:brightness-95 dark:hover:brightness-110",
			},
		},
		defaultVariants: {
			variant: "surface",
		},
	}
);

type FabExtendedProps = React.ComponentProps<"button"> &
	VariantProps<typeof fabExtendedVariants> & {
		icon?: React.ReactNode;
	};

function FabExtended({
	className,
	variant,
	icon,
	children,
	type = "button",
	...props
}: FabExtendedProps) {
	return (
		<button
			data-slot="fab-extended"
			data-variant={variant ?? "surface"}
			type={type}
			className={cn(fabExtendedVariants({ variant }), className)}
			{...props}
		>
			{icon ? (
				<span data-slot="fab-extended-icon" className="inline-flex shrink-0">
					{icon}
				</span>
			) : null}
			<span data-slot="fab-extended-label">{children}</span>
		</button>
	);
}

export { Fab, FabExtended, fabExtendedVariants, fabVariants };
export type { FabExtendedProps, FabProps };
