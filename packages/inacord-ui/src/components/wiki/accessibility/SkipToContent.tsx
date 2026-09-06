export function SkipToContent() {
	const handleClick = (e: React.MouseEvent<HTMLAnchorElement>) => {
		e.preventDefault();
		const main = document.querySelector("#main-content");
		if (main instanceof HTMLElement) {
			main.setAttribute("tabindex", "-1");
			main.focus();
			// Retirer tabindex après focus pour ne pas perturber la navigation au clavier
			main.addEventListener("blur", () => main.removeAttribute("tabindex"), {
				once: true,
			});
		}
	};

	return (
		<a
			href="#main-content"
			onClick={handleClick}
			className="sr-only focus:not-sr-only focus:fixed focus:top-0 focus:left-0 focus:z-50 focus:px-4 focus:py-3 focus:text-sm focus:font-semibold focus:bg-primary focus:text-on-primary focus:rounded-br-xl focus:outline-hidden focus:ring-2 focus:ring-primary/50"
		>
			Aller au contenu principal
		</a>
	);
}
