import * as React from "react";

// Material 3 Breakpoints (in pixels for web)
// Compact: < 600px
// Medium: 600px - 839px
// Expanded: >= 840px
export const BREAKPOINTS = {
	COMPACT: 600,
	MEDIUM: 840,
	EXPANDED: 1200,
} as const;

export type Breakpoint = "compact" | "medium" | "expanded" | "large";

export function useIsMobile() {
	const [isMobile, setIsMobile] = React.useState<boolean>(false);

	React.useEffect(() => {
		const mql = window.matchMedia(`(max-width: ${BREAKPOINTS.COMPACT - 1}px)`);
		const onChange = () => {
			setIsMobile(window.innerWidth < BREAKPOINTS.COMPACT);
		};
		mql.addEventListener("change", onChange);
		setIsMobile(window.innerWidth < BREAKPOINTS.COMPACT);
		return () => mql.removeEventListener("change", onChange);
	}, []);

	return isMobile;
}

export function useBreakpoint(): Breakpoint {
	const [breakpoint, setBreakpoint] = React.useState<Breakpoint>("compact");

	React.useEffect(() => {
		const updateBreakpoint = () => {
			const width = window.innerWidth;
			if (width < BREAKPOINTS.COMPACT) {
				setBreakpoint("compact");
			} else if (width < BREAKPOINTS.MEDIUM) {
				setBreakpoint("medium");
			} else if (width < BREAKPOINTS.EXPANDED) {
				setBreakpoint("expanded");
			} else {
				setBreakpoint("large");
			}
		};

		updateBreakpoint();
		window.addEventListener("resize", updateBreakpoint);
		return () => window.removeEventListener("resize", updateBreakpoint);
	}, []);

	return breakpoint;
}
