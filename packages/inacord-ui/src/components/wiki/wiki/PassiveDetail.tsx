import { Sparkles } from "lucide-react";
import { Image } from "../../../compat/next";
import { cn } from "../../../lib/utils";

// ── Types (miroir de PassiveDetail de wiki-service, côté client) ──

interface PassiveLoc {
	fr: string | null;
	en: string | null;
	ja: string | null;
}

export interface PassiveDetailData {
	passive_id: string;
	string_id: string;
	effect_id: string;
	rarity: number;
	element: number;
	element_name: string;
	buff_icon_type: number | null;
	effect_params: number[];
	main_value: number | null;
	name: PassiveLoc;
	description: PassiveLoc;
	text_raw: PassiveLoc;
	category: string;
	family: Array<{
		passive_id: string;
		string_id: string;
		main_value: number | null;
		rarity: number;
		element_name: string;
		description: PassiveLoc;
	}>;
	imageUrl: string | null;
}

// ── Libellés ──

const ELEMENT_COLORS: Record<string, string> = {
	// Rôles `element-*` : les teintes du JEU, relevées sur ses icônes officielles. Les classes
	// Tailwind en dur d'avant divergeaient d'un écran à l'autre — le vent y était bleu ici, vert
	// ailleurs. Ce paquet ne fournit AUCUNE valeur : chaque hôte mappe le rôle dans son
	// `@theme inline` (`apps/nie-web/src/base.css` pour Aphrody), et un rôle non mappé s'affiche
	// transparent sur transparent, sans erreur.
	fire: "bg-element-feu/15 text-element-feu border-element-feu/40",
	forest: "bg-element-foret/15 text-element-foret border-element-foret/40",
	mountain: "bg-element-montagne/15 text-element-montagne border-element-montagne/40",
	neutral: "bg-outline-variant/20 text-on-surface-variant border-outline-variant/40",
	wind: "bg-element-vent/15 text-element-vent border-element-vent/40",
};

const ELEMENT_LABELS: Record<string, string> = {
	fire: "Feu",
	forest: "Forêt",
	mountain: "Montagne",
	neutral: "Neutre",
	wind: "Vent",
};

export const PASSIVE_CATEGORY_LABELS: Record<string, string> = {
	custom: "Personnalisé",
	hero: "Héros",
	miximax: "Miximax",
	other: "Autre",
	player: "Joueur",
	soul: "Soul",
	swap: "Swap",
	team_custom: "Cust. équipe",
	team_mixi: "Mixi équipe",
};

export function elementLabel(name: string): string {
	return ELEMENT_LABELS[name] ?? name;
}

export function elementColor(name: string): string {
	return ELEMENT_COLORS[name] ?? ELEMENT_COLORS.neutral;
}

// ── Vue détail ──

export function PassiveDetailView({ passive }: { passive: PassiveDetailData }) {
	const textFr =
		passive.description.fr ?? passive.description.en ?? passive.description.ja ?? "";
	const nameJa = passive.name.ja ?? "";
	const elColor = elementColor(passive.element_name);
	const elLabel = elementLabel(passive.element_name);
	const catLabel = PASSIVE_CATEGORY_LABELS[passive.category] ?? passive.category;

	// Courbe d'effets : toutes les valeurs de la famille (effet partagé).
	const curve = passive.family.filter((f) => f.main_value !== null);
	const currentId = passive.passive_id;

	return (
		<div className="space-y-8">
			{/* En-tête */}
			<div className="rounded-3xl border border-outline-variant/30 bg-surface-container-low p-6 md:p-8">
				<div className="flex flex-col md:flex-row md:items-start gap-6">
					<div className="shrink-0">
						{passive.imageUrl ? (
							<Image
								src={passive.imageUrl}
								alt={textFr || passive.string_id}
								width={96}
								height={96}
								className="rounded-2xl bg-surface-container size-24 object-contain"
								unoptimized
							/>
						) : (
							<div className="size-24 rounded-2xl bg-primary/10 flex items-center justify-center">
								<Sparkles size={40} className="text-primary" aria-hidden="true" />
							</div>
						)}
					</div>

					<div className="flex-1 space-y-3">
						<div className="flex flex-wrap items-center gap-2">
							<span className="inline-flex items-center px-2.5 py-1 rounded-full text-[11px] font-bold uppercase tracking-wider bg-primary/15 text-primary border border-primary/30">
								{catLabel}
							</span>
							{passive.element_name !== "neutral" && (
								<span
									className={cn(
										"inline-flex items-center px-2.5 py-1 rounded-full text-[11px] font-bold uppercase tracking-wider border",
										elColor
									)}
								>
									{elLabel}
								</span>
							)}
							<span className="inline-flex items-center px-2.5 py-1 rounded-full text-[11px] font-bold uppercase tracking-wider bg-amber-500/15 text-amber-600 border border-amber-300/40">
								Rareté {passive.rarity}
							</span>
						</div>

						<h1 className="text-2xl md:text-3xl font-extrabold text-on-surface font-display leading-tight whitespace-pre-line">
							{textFr}
						</h1>

						{nameJa && (
							<p className="text-sm text-on-surface-variant whitespace-pre-line" lang="ja">
								{nameJa}
							</p>
						)}

						<div className="flex flex-wrap gap-x-6 gap-y-1 text-[11px] font-mono text-on-surface-variant/70 pt-2">
							<span>string_id : {passive.string_id}</span>
							<span>effect_id : {passive.effect_id}</span>
							<span>passive_id : {passive.passive_id}</span>
							{passive.buff_icon_type !== null && (
								<span>buff_icon : {passive.buff_icon_type}</span>
							)}
						</div>
					</div>
				</div>
			</div>

			{/* Description résolue (FR / EN / JA) */}
			<section aria-labelledby="passive-desc-heading" className="space-y-3">
				<h2 id="passive-desc-heading" className="text-lg font-bold text-on-surface">
					Description
				</h2>
				<div className="grid grid-cols-1 md:grid-cols-3 gap-3">
					{(
						[
							["Français", passive.description.fr],
							["English", passive.description.en],
							["日本語", passive.description.ja],
						] as const
					).map(([lang, txt]) =>
						txt ? (
							<div
								key={lang}
								className="rounded-2xl border border-outline-variant/30 bg-surface-container-low p-4"
							>
								<div className="text-[10px] font-bold uppercase tracking-wider text-on-surface-variant mb-2">
									{lang}
								</div>
								<p className="text-sm text-on-surface whitespace-pre-line leading-snug">{txt}</p>
							</div>
						) : null
					)}
				</div>
			</section>

			{/* Courbe d'effets (valeurs par instance de la famille) */}
			{curve.length > 1 && (
				<section aria-labelledby="passive-curve-heading" className="space-y-3">
					<h2 id="passive-curve-heading" className="text-lg font-bold text-on-surface">
						Valeurs de l&apos;effet ({curve.length})
					</h2>
					<p className="text-sm text-on-surface-variant">
						Toutes les variantes partageant cet effet, de la plus faible à la plus forte.
					</p>
					<div className="overflow-x-auto rounded-2xl border border-outline-variant/30">
						<table className="w-full text-sm">
							<thead>
								<tr className="bg-surface-container-high text-on-surface-variant">
									<th className="text-left font-bold px-4 py-2.5 text-xs uppercase tracking-wider">
										Valeur
									</th>
									<th className="text-left font-bold px-4 py-2.5 text-xs uppercase tracking-wider hidden md:table-cell">
										Texte
									</th>
								</tr>
							</thead>
							<tbody>
								{curve.map((f) => {
									const isCurrent = f.passive_id === currentId;
									return (
										<tr
											key={f.passive_id}
											className={cn(
												"border-t border-outline-variant/20",
												isCurrent
													? "bg-primary/10"
													: "hover:bg-surface-container-low transition-colors"
											)}
										>
											<td className="px-4 py-2.5 font-mono font-bold text-on-surface whitespace-nowrap">
												+{f.main_value}%
												{isCurrent && (
													<span className="ml-2 text-[10px] font-sans font-bold uppercase tracking-wider text-primary">
														actuel
													</span>
												)}
											</td>
											<td className="px-4 py-2.5 text-xs text-on-surface-variant whitespace-pre-line hidden md:table-cell">
												{f.description.fr ?? f.description.en ?? ""}
											</td>
										</tr>
									);
								})}
							</tbody>
						</table>
					</div>
				</section>
			)}
		</div>
	);
}
