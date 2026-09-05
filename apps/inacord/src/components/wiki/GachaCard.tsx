"use client";

import { Gift, Shirt } from "lucide-react";
import { Link } from "@/components/ui/link";
import type { CapsulePrize, Costume } from "@/lib/wikiTypes";
import { cn } from "@/lib/utils";

/**
 * Carte d'un lot de capsule (gacha). Le jeu n'expose le contenu qu'à travers des
 * hashs (pas de nom/image résolus dans nos tables) : on affiche donc l'identité
 * réelle du lot — son hash, son contenu (hash), son pool de tirage. Lien vers le
 * détail qui montre la composition complète du pool.
 */
export function CapsuleCard({
	prize,
	className,
}: {
	prize: CapsulePrize;
	className?: string;
}) {
	return (
		<Link
			href={`/capsule/${prize.id}`}
			className={cn(
				"group flex h-full flex-col gap-2 rounded-2xl p-4",
				"border border-outline-variant/30 bg-surface-container-low",
				"transition-all duration-200 hover:-translate-y-0.5 hover:bg-surface-container hover:shadow-lg",
				className
			)}
		>
			<div className="flex items-center gap-3">
				<span className="flex size-10 shrink-0 items-center justify-center rounded-xl bg-primary/10 text-primary">
					<Gift size={20} aria-hidden="true" />
				</span>
				<div className="min-w-0">
					<span className="text-[10px] font-bold uppercase tracking-wider text-on-surface-variant/60">
						Lot
					</span>
					<h3 className="truncate font-mono text-sm font-bold leading-tight text-on-surface transition-colors group-hover:text-primary">
						{prize.id}
					</h3>
				</div>
			</div>
			<div className="mt-auto space-y-1 text-xs">
				<div className="flex items-center justify-between gap-2">
					<span className="text-on-surface-variant/70">Contenu</span>
					<span className="truncate font-mono text-on-surface-variant">{prize.contentRef}</span>
				</div>
				<div className="flex items-center justify-between gap-2">
					<span className="text-on-surface-variant/70">Pool</span>
					<span className="truncate font-mono text-on-surface-variant">{prize.poolRef}</span>
				</div>
			</div>
		</Link>
	);
}

/**
 * Carte d'un costume. Le `model_ref` est un hash CRC de modèle 3D (pas une image
 * affichable directement) ; on présente l'index, le type et le hash, sans inventer
 * de visuel.
 */
export function CostumeCard({
	costume,
	className,
}: {
	costume: Costume;
	className?: string;
}) {
	const typeColor =
		costume.type === 0
			? "bg-slate-500/15 text-slate-500"
			: costume.type === 1
				? "bg-blue-500/15 text-blue-600"
				: "bg-purple-500/15 text-purple-600";

	return (
		<div
			className={cn(
				"flex h-full flex-col gap-2 rounded-2xl p-4",
				"border border-outline-variant/30 bg-surface-container-low",
				"transition-colors duration-200 hover:bg-surface-container",
				className
			)}
		>
			<div className="flex items-center gap-3">
				<span className="flex size-10 shrink-0 items-center justify-center rounded-xl bg-tertiary/10 text-tertiary">
					<Shirt size={20} aria-hidden="true" />
				</span>
				<div className="min-w-0">
					<span className="text-[10px] font-bold uppercase tracking-wider text-on-surface-variant/60">
						Costume #{costume.index}
					</span>
					<h3 className="truncate font-mono text-sm font-bold leading-tight text-on-surface">
						{costume.modelRef}
					</h3>
				</div>
			</div>
			<div className="mt-auto flex flex-wrap items-center gap-1.5">
				<span
					className={cn(
						"inline-flex items-center rounded-full px-2 py-0.5 text-[10px] font-bold uppercase tracking-wider",
						typeColor
					)}
				>
					{costume.typeLabel}
				</span>
				{(costume.flag1 !== 0 || costume.flag2 !== 0) && (
					<span className="inline-flex items-center rounded-full bg-surface-container-highest px-2 py-0.5 font-mono text-[10px] text-on-surface-variant/70">
						{costume.flag1 !== 0 ? `f1:${costume.flag1}` : ""}
						{costume.flag1 !== 0 && costume.flag2 !== 0 ? " " : ""}
						{costume.flag2 !== 0 ? `f2:${costume.flag2}` : ""}
					</span>
				)}
			</div>
		</div>
	);
}
