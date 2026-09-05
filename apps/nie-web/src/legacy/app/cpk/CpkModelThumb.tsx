"use client";
/**
 * Mini-aperçu 3D **lazy** pour la galerie d'un dossier de modèles CPK (`_waza`, `_item`,
 * `_animal`, `_keshin`, `_armd`). Le `<model-viewer>` n'est monté QUE lorsque la vignette
 * entre dans le viewport (IntersectionObserver) — on évite ainsi de créer N contextes WebGL
 * d'un coup dans un dossier de plusieurs centaines d'entrées. Repli sur l'icône dossier si
 * le modèle n'est pas servi (404) ou échoue à charger.
 */
import { useEffect, useRef, useState } from "react";
import { Icon } from "@/components/ui/Icon";

const MODEL_VIEWER_SRC = "/vendor/model-viewer.min.js";
let modelViewerLoading: Promise<void> | null = null;

function loadModelViewer(): Promise<void> {
	if (typeof window === "undefined") return Promise.resolve();
	if (window.customElements?.get("model-viewer")) return Promise.resolve();
	if (modelViewerLoading) return modelViewerLoading;
	const existing = document.querySelector<HTMLScriptElement>(
		`script[src="${MODEL_VIEWER_SRC}"]`,
	);
	if (existing) {
		modelViewerLoading = window.customElements.whenDefined("model-viewer").then(() => {});
		return modelViewerLoading;
	}
	modelViewerLoading = new Promise<void>((resolve, reject) => {
		const script = document.createElement("script");
		script.src = MODEL_VIEWER_SRC;
		script.async = true;
		script.addEventListener("load", () => {
			window.customElements.whenDefined("model-viewer").then(() => resolve(), reject);
		});
		script.addEventListener("error", () => {
			modelViewerLoading = null;
			reject(new Error("model-viewer load failed"));
		});
		document.head.appendChild(script);
	});
	return modelViewerLoading;
}

export function CpkModelThumb({ glbUrl, name }: { glbUrl: string; name: string }) {
	const rootRef = useRef<HTMLDivElement>(null);
	const hostRef = useRef<HTMLDivElement>(null);
	const [visible, setVisible] = useState(false);
	const [ready, setReady] = useState(false);
	const [errored, setErrored] = useState(false);

	// Gate viewport : ne monte le viewer que lorsque la vignette devient visible.
	useEffect(() => {
		const el = rootRef.current;
		if (!el || visible) return;
		const obs = new IntersectionObserver(
			(entries) => {
				if (entries.some((e) => e.isIntersecting)) {
					setVisible(true);
					obs.disconnect();
				}
			},
			{ rootMargin: "300px" },
		);
		obs.observe(el);
		return () => obs.disconnect();
	}, [visible]);

	// Charge le bundle <model-viewer> une fois la vignette visible.
	useEffect(() => {
		if (!visible) return;
		let cancelled = false;
		loadModelViewer()
			.then(() => !cancelled && setReady(true))
			.catch(() => !cancelled && setErrored(true));
		return () => {
			cancelled = true;
		};
	}, [visible]);

	// Monte l'élément custom impérativement (non typé par JSX) et écoute son erreur.
	useEffect(() => {
		if (!ready || errored || !hostRef.current) return;
		const host = hostRef.current;
		host.innerHTML = "";
		const mv = document.createElement("model-viewer");
		mv.setAttribute("alt", `Modèle 3D ${name}`);
		// Aperçu NON interactif (auto-rotate seul) : les clics traversent vers le bouton parent
		// (navigation dans le dossier). L'interaction complète vit dans le viewer par-fichier.
		mv.setAttribute("auto-rotate", "");
		mv.setAttribute("rotation-per-second", "40deg");
		mv.setAttribute("interaction-prompt", "none");
		mv.setAttribute("disable-zoom", "");
		mv.style.width = "100%";
		mv.style.height = "100%";
		mv.style.backgroundColor = "transparent";
		mv.style.pointerEvents = "none";
		mv.addEventListener("error", () => setErrored(true));
		mv.setAttribute("src", glbUrl);
		host.appendChild(mv);
		return () => {
			host.innerHTML = "";
		};
	}, [ready, errored, glbUrl, name]);

	// Repli : modèle non servi/échoué → icône dossier (la vignette reste cohérente avec un dossier).
	if (errored) {
		return (
			<div className="w-full h-full flex items-center justify-center text-on-surface-variant/60">
				<Icon name="folder" size={32} />
			</div>
		);
	}

	return (
		<div ref={rootRef} className="w-full h-full relative">
			<div ref={hostRef} className="absolute inset-0" />
			{(!visible || !ready) && (
				<div className="absolute inset-0 flex items-center justify-center">
					<div className="animate-spin rounded-full size-5 border-b-2 border-primary/50" />
				</div>
			)}
		</div>
	);
}
