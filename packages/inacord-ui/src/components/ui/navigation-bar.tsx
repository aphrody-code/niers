"use client";

// Material Design 3 — Navigation Bar (bottom navigation)
// Spec : https://m3.material.io/components/navigation-bar/specs
// Container : bg-surface-container, height 80dp, padding-top 12dp.
// 3 à 5 destinations ; chaque item porte un active-indicator pill
// (bg-secondary-container, shape full) derrière l'icon active uniquement.
// Label visible toujours par défaut ; label-large variant : on-active only.

import type * as React from "react";

import { cn } from "../../lib/utils";

function NavigationBar({ className, children, ...props }: React.ComponentProps<"nav">) {
	return (
		<nav
			data-slot="navigation-bar"
			className={cn(
				"flex min-h-20 w-full items-center justify-around bg-surface-container px-2 pt-3 pb-4 text-on-surface",
				className
			)}
			{...props}
		>
			{children}
		</nav>
	);
}

type NavigationBarItemProps = Omit<React.ComponentProps<"button">, "type"> & {
	active?: boolean;
	icon: React.ReactNode;
	label: string;
	/**
	 * Lorsque `true`, le label n'apparaît que sur l'item actif (variant M3
	 * "labels on active"). Par défaut, le label est toujours visible.
	 */
	labelOnActive?: boolean;
};

function NavigationBarItem({
	active = false,
	icon,
	label,
	labelOnActive = false,
	className,
	...props
}: NavigationBarItemProps) {
	return (
		<button
			data-slot="navigation-bar-item"
			data-active={active ? "true" : undefined}
			type="button"
			aria-current={active ? "page" : undefined}
			className={cn(
				"group/nav-bar-item relative flex min-h-12 min-w-12 flex-1 cursor-pointer flex-col items-center justify-center gap-1 px-2 py-1 text-on-surface-variant transition-colors duration-(--md-sys-motion-duration-short4) ease-(--md-sys-motion-easing-emphasized) disabled:pointer-events-none disabled:opacity-50 outline-hidden focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px] data-[active=true]:text-on-secondary-container",
				className
			)}
			{...props}
		>
			<span
				data-slot="navigation-bar-item-indicator"
				className={cn(
					"relative flex h-8 w-16 items-center justify-center rounded-(--md-sys-shape-corner-full) transition-[background-color,transform] duration-(--md-sys-motion-duration-medium2) ease-(--md-sys-motion-easing-emphasized)",
					"group-data-[active=true]/nav-bar-item:bg-secondary-container",
					"[&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-6"
				)}
			>
				{icon}
			</span>
			<span
				data-slot="navigation-bar-item-label"
				className={cn(
					"text-xs font-medium leading-4 tracking-[0.5px]",
					labelOnActive
						? "opacity-0 group-data-[active=true]/nav-bar-item:opacity-100"
						: "opacity-100"
				)}
			>
				{label}
			</span>
		</button>
	);
}

export { NavigationBar, NavigationBarItem };
export type { NavigationBarItemProps };
