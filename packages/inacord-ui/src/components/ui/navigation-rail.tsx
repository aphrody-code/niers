"use client";

// Material Design 3 — Navigation Rail (side navigation)
// Spec : https://m3.material.io/components/navigation-rail/specs
// Container : bg-surface, width 80dp, padding-top 44dp.
// Slots optionnels : header (au-dessus du FAB), fab (au-dessus des items),
// footer (en bas). Chaque NavigationRailItem porte le même active-indicator
// pill que NavigationBar (bg-secondary-container, shape full).

import type * as React from "react";

import { cn } from "../../lib/utils";

type NavigationRailProps = React.ComponentProps<"nav"> & {
	fab?: React.ReactNode;
	header?: React.ReactNode;
	footer?: React.ReactNode;
};

function NavigationRail({
	fab,
	header,
	footer,
	children,
	className,
	...props
}: NavigationRailProps) {
	return (
		<nav
			data-slot="navigation-rail"
			className={cn(
				"flex h-full w-20 flex-col items-center gap-3 bg-surface pt-11 pb-3 text-on-surface",
				className
			)}
			{...props}
		>
			{header ? (
				<div data-slot="navigation-rail-header" className="flex w-full flex-col items-center">
					{header}
				</div>
			) : null}
			{fab ? (
				<div data-slot="navigation-rail-fab" className="flex w-full justify-center pb-2">
					{fab}
				</div>
			) : null}
			<div
				data-slot="navigation-rail-items"
				className="flex w-full flex-1 flex-col items-center gap-3"
			>
				{children}
			</div>
			{footer ? (
				<div data-slot="navigation-rail-footer" className="flex w-full flex-col items-center">
					{footer}
				</div>
			) : null}
		</nav>
	);
}

type NavigationRailItemProps = Omit<React.ComponentProps<"button">, "type"> & {
	active?: boolean;
	icon: React.ReactNode;
	label: string;
};

function NavigationRailItem({
	active = false,
	icon,
	label,
	className,
	...props
}: NavigationRailItemProps) {
	return (
		<button
			data-slot="navigation-rail-item"
			data-active={active ? "true" : undefined}
			type="button"
			aria-current={active ? "page" : undefined}
			className={cn(
				"group/nav-rail-item relative flex min-h-14 w-full cursor-pointer flex-col items-center justify-center gap-1 px-2 py-1 text-on-surface-variant transition-colors duration-(--md-sys-motion-duration-short4) ease-(--md-sys-motion-easing-emphasized) disabled:pointer-events-none disabled:opacity-50 outline-hidden focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px] data-[active=true]:text-on-secondary-container",
				className
			)}
			{...props}
		>
			<span
				data-slot="navigation-rail-item-indicator"
				className={cn(
					"relative flex h-8 w-14 items-center justify-center rounded-(--md-sys-shape-corner-full) transition-[background-color,transform] duration-(--md-sys-motion-duration-medium2) ease-(--md-sys-motion-easing-emphasized)",
					"group-data-[active=true]/nav-rail-item:bg-secondary-container",
					"[&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-6"
				)}
			>
				{icon}
			</span>
			<span
				data-slot="navigation-rail-item-label"
				className="text-xs font-medium leading-4 tracking-[0.5px]"
			>
				{label}
			</span>
		</button>
	);
}

export { NavigationRail, NavigationRailItem };
export type { NavigationRailItemProps, NavigationRailProps };
