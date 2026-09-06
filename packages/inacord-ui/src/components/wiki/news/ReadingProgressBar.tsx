"use client";

import { useEffect, useState } from "react";

export function ReadingProgressBar() {
	const [progress, setProgress] = useState(0);

	useEffect(() => {
		const handleScroll = () => {
			const scrollTop = window.scrollY;
			const docHeight = document.documentElement.scrollHeight - window.innerHeight;
			if (docHeight > 0) {
				setProgress(Math.min((scrollTop / docHeight) * 100, 100));
			}
		};

		window.addEventListener("scroll", handleScroll, { passive: true });
		return () => window.removeEventListener("scroll", handleScroll);
	}, []);

	return (
		<div className="fixed top-0 left-0 w-full h-[3px] z-50 bg-transparent">
			<div
				className="h-full bg-primary transition-[width] duration-150 ease-out"
				style={{ width: `${progress}%` }}
			/>
		</div>
	);
}
