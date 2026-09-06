"use client";

/**
 * Barre d'outils d'une famille de modèles : **recherche** et bascule **grille / liste**.
 *
 * La recherche porte sur la famille ENTIÈRE, pas sur la page affichée : filtrer les 48 codes
 * déjà rendus laisserait 5 478 personnages inatteignables autrement qu'en tournant 46 pages.
 * Elle passe donc par l'URL (`?q=`), que la page relit côté serveur — ce qui la rend aussi
 * partageable et navigable au bouton retour.
 *
 * Saisie temporisée (300 ms) et `replace` plutôt que `push` : une frappe de dix lettres ne doit
 * ni déclencher dix rendus serveur, ni empiler dix entrées d'historique.
 *
 * L'état complet de la vue (`q`, `view`) arrive du serveur en props et les URL se reconstruisent
 * à partir de lui — pas de `useSearchParams`, qui imposerait une frontière Suspense pour lire ce
 * que l'appelant connaît déjà. `page` est délibérément abandonné à chaque changement.
 */

import { useEffect, useRef, useState } from "react";
import { useRouter } from "../../../compat/next";
import { Icon } from "../../../components/wiki/ui/Icon";

/** Disposition de la galerie. */
export type ModelView = "grid" | "list";

/** URL d'une vue de la famille — `grid` et la page 1 sont les valeurs par défaut, donc implicites. */
function urlVue(base: string, q: string, view: ModelView): string {
	const p = new URLSearchParams();
	if (q) p.set("q", q);
	if (view === "list") p.set("view", "list");
	const s = p.toString();
	return s ? `${base}?${s}` : base;
}

export function ModelToolbar({ base, q, view }: { base: string; q: string; view: ModelView }) {
	const router = useRouter();
	const [texte, setTexte] = useState(q);
	// La valeur venue de l'URL fait foi quand elle change sans nous (retour arrière, lien).
	const dernierQ = useRef(q);

	useEffect(() => {
		if (dernierQ.current !== q) {
			dernierQ.current = q;
			setTexte(q);
		}
	}, [q]);

	useEffect(() => {
		if (texte === q) return;
		const t = setTimeout(() => {
			dernierQ.current = texte;
			router.replace(urlVue(base, texte, view));
		}, 300);
		return () => clearTimeout(t);
	}, [texte, q, base, view, router]);

	return (
		<div className="flex flex-wrap items-center gap-2">
			<div className="relative min-w-0 flex-1">
				<Icon
					name="search"
					size={18}
					className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-on-surface-variant"
				/>
				<input
					type="search"
					value={texte}
					onChange={(e) => setTexte(e.target.value)}
					placeholder="Rechercher un modèle — nom ou code"
					aria-label="Rechercher un modèle"
					className="w-full rounded-full border border-outline-variant bg-surface-container-low py-2 pl-10 pr-4 text-sm text-on-surface outline-none transition placeholder:text-on-surface-variant focus:border-primary"
				/>
			</div>
			<div
				role="group"
				aria-label="Disposition"
				className="flex shrink-0 overflow-hidden rounded-full border border-outline-variant"
			>
				{(
					[
						{ v: "grid", icon: "grid_view", label: "Grille" },
						{ v: "list", icon: "view_list", label: "Liste" },
					] as const
				).map(({ v, icon, label }) => {
					const actif = view === v;
					return (
						<a
							key={v}
							href={urlVue(base, texte, v)}
							aria-current={actif ? "true" : undefined}
							title={label}
							className={`flex items-center gap-1.5 px-3.5 py-2 text-sm transition ${
								actif
									? "bg-primary text-on-primary"
									: "bg-surface-container-low text-on-surface-variant hover:bg-surface-container"
							}`}
						>
							<Icon name={icon} size={18} />
							<span className="hidden sm:inline">{label}</span>
						</a>
					);
				})}
			</div>
		</div>
	);
}
