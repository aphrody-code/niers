"use client";

import { useCallback, useRef, useState } from "react";
import { cn } from "../../lib/utils";

const THRESHOLD = 80;

/**
 * Tirer vers le bas pour rafraîchir — porté du wiki vers le paquet partagé.
 *
 * Il appelait `useRouter().refresh()` de `next/navigation`. Ce paquet est monté par **deux
 * hôtes** dont aucun n'est Next — Aphrody est un Vite/SPA, Inacord une application Tauri — et
 * cet import aurait fait échouer les deux à la résolution. Le geste est donc rendu à
 * l'appelant : le composant sait *quand* rafraîchir, l'hôte sait *comment*.
 *
 * `onRafraichir` est optionnel et attendu : sans lui, le composant anime et ne fait rien, ce
 * qui est le comportement honnête pour un hôte qui n'a rien à recharger.
 */
export function PullToRefresh({
	children,
	onRafraichir,
}: {
	children: React.ReactNode;
	onRafraichir?: () => void | Promise<void>;
}) {
	const [pullDistance, setPullDistance] = useState(0);
	const [isRefreshing, setIsRefreshing] = useState(false);
	const [pulling, setPulling] = useState(false);
	const startY = useRef(0);

	const onTouchStart = useCallback(
		(e: React.TouchEvent) => {
			if (window.scrollY > 0 || isRefreshing) {
				return;
			}
			startY.current = e.touches[0].clientY;
			setPulling(true);
		},
		[isRefreshing]
	);

	const onTouchMove = useCallback(
		(e: React.TouchEvent) => {
			if (!pulling || isRefreshing) {
				return;
			}
			const dy = e.touches[0].clientY - startY.current;
			if (dy < 0) {
				setPulling(false);
				setPullDistance(0);
				return;
			}
			setPullDistance(Math.min(dy * 0.4, THRESHOLD * 1.5));
		},
		[pulling, isRefreshing]
	);

	const onTouchEnd = useCallback(() => {
		if (!pulling) {
			return;
		}
		setPulling(false);

		if (pullDistance >= THRESHOLD) {
			setIsRefreshing(true);
			setPullDistance(THRESHOLD * 0.6);
			// Le rafraîchissement est celui de l'hôte : `router.refresh()` sous Next, un
			// rechargement de données ailleurs, rien du tout si l'hôte n'a rien à recharger.
			void onRafraichir?.();
			setTimeout(() => {
				setIsRefreshing(false);
				setPullDistance(0);
			}, 1000);
		} else {
			setPullDistance(0);
		}
	}, [pulling, pullDistance, onRafraichir]);

	return (
		<div
			onTouchStart={onTouchStart}
			onTouchMove={onTouchMove}
			onTouchEnd={onTouchEnd}
			className="relative"
		>
			{/* Pull indicator */}
			<div
				className={cn(
					"absolute left-1/2 -translate-x-1/2 z-10 flex items-center justify-center transition-opacity",
					pullDistance > 10 ? "opacity-100" : "opacity-0"
				)}
				style={{ top: Math.max(0, pullDistance - 40) }}
			>
				<div
					className={cn(
						"size-8 rounded-full border-2 border-primary border-t-transparent",
						isRefreshing && "animate-spin"
					)}
					style={{
						transform: isRefreshing
							? undefined
							: `rotate(${Math.min(pullDistance / THRESHOLD, 1) * 360}deg)`,
					}}
				/>
			</div>

			{/* Content with pull offset */}
			<div
				style={{
					transform: pullDistance > 0 ? `translateY(${pullDistance}px)` : undefined,
					transition: pulling ? "none" : "transform 0.3s cubic-bezier(0.2, 0, 0, 1)",
				}}
			>
				{children}
			</div>
		</div>
	);
}
