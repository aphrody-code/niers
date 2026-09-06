"use client";

import { useEffect, useRef, useState } from "react";

interface AnimatedCounterProps {
	value: number;
	duration?: number;
	className?: string;
}

export function AnimatedCounter({ value, duration = 1500, className }: AnimatedCounterProps) {
	const [display, setDisplay] = useState(0);
	const ref = useRef<HTMLSpanElement>(null);
	const hasAnimated = useRef(false);

	useEffect(() => {
		const el = ref.current;
		if (!el || hasAnimated.current) {
			return;
		}

		// Respect prefers-reduced-motion
		const prefersReduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
		if (prefersReduced) {
			setDisplay(value);
			hasAnimated.current = true;
			return;
		}

		const observer = new IntersectionObserver(
			([entry]) => {
				if (!entry.isIntersecting || hasAnimated.current) {
					return;
				}
				hasAnimated.current = true;
				observer.disconnect();

				const start = performance.now();
				const animate = (now: number) => {
					const elapsed = now - start;
					const progress = Math.min(elapsed / duration, 1);
					// Ease-out cubic
					const eased = 1 - (1 - progress) ** 3;
					setDisplay(Math.round(eased * value));
					if (progress < 1) {
						requestAnimationFrame(animate);
					}
				};
				requestAnimationFrame(animate);
			},
			{ threshold: 0.3 }
		);

		observer.observe(el);
		return () => observer.disconnect();
	}, [value, duration]);

	return (
		<span ref={ref} className={className} aria-label={`${value.toLocaleString("fr-FR")}`}>
			{display.toLocaleString("fr-FR")}
		</span>
	);
}
