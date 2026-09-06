"use client";

import { ChevronUp } from "lucide-react";
import { useEffect, useState } from "react";

export function BackToTopButton() {
	const [visible, setVisible] = useState(false);

	useEffect(() => {
		const onScroll = () => setVisible(window.scrollY > 400);
		window.addEventListener("scroll", onScroll, { passive: true });
		return () => window.removeEventListener("scroll", onScroll);
	}, []);

	return (
		<button
			onClick={() => window.scrollTo({ behavior: "smooth", top: 0 })}
			aria-label="Retour en haut"
			className={`fixed bottom-24 md:bottom-6 right-4 z-30 size-12 rounded-full bg-primary text-on-primary shadow-lg flex items-center justify-center transition-all duration-300 hover:bg-primary/90 ${
				visible ? "opacity-100 translate-y-0" : "opacity-0 translate-y-4 pointer-events-none"
			}`}
		>
			<ChevronUp className="size-5" />
		</button>
	);
}
