"use client";

import { ClipboardList } from "lucide-react";
import { Image } from "@/components/ui/image";
import { Link } from "@/components/ui/link";
import { useState } from "react";
import { ElementIcon } from "@/components/wiki/ElementIcon";
import { getCharacterFaceUrl } from "@/lib/wikiImages";
import { cn } from "@/lib/utils";

export interface CoachCardProps {
	id: number;
	name: string;
	roleFr: string;
	role: string;
	playstyleFr?: string | null;
	elementKey?: string | null;
	elementFr?: string | null;
	/**
	 * Code interne du personnage homonyme (`c0xxxxxxx`) pour dériver le portrait
	 * via `getCharacterFaceUrl`. `null` pour les NPC sans personnage correspondant.
	 */
	internalCode?: string | null;
	/** Statistique affectée par le passif d'équipe (texte source EN). */
	stat?: string | null;
	/** Valeur du bonus pré-résolue. */
	buff?: string | null;
	className?: string;
}

/** Couleur d'accent par rôle (Coordinateur / Manager / Coach). */
const ROLE_ACCENT: Record<string, string> = {
	Coach: "text-tertiary",
	Coordinator: "text-primary",
	Manager: "text-secondary",
};

/**
 * Carte d'un membre de banc (entraîneur / coach).
 *
 * La table source n'a pas d'illustration (`image` toujours vide) : on dérive le
 * portrait du personnage homonyme via `getCharacterFaceUrl(internalCode)`. À
 * défaut de code (NPC sans match) ou en cas d'erreur de chargement, on retombe
 * sur la vignette « sifflet ». L'icône d'élément reste affichée en surimpression.
 */
export function CoachCard({
	id,
	name,
	roleFr,
	role,
	playstyleFr,
	elementKey,
	elementFr,
	internalCode,
	stat,
	buff,
	className,
}: CoachCardProps) {
	const accent = ROLE_ACCENT[role] ?? "text-on-surface-variant";
	const [imageFailed, setImageFailed] = useState(false);
	const faceUrl = internalCode ? getCharacterFaceUrl(internalCode) : null;
	const showFace = Boolean(faceUrl) && !imageFailed;

	return (
		<Link
			href={`/entraineur/${id}`}
			className={cn(
				"group flex h-full flex-col overflow-hidden rounded-2xl",
				"border border-outline-variant/30 bg-surface-container-low",
				"transition-all duration-200 hover:-translate-y-0.5 hover:bg-surface-container hover:shadow-lg",
				className
			)}
		>
			<div className="relative flex aspect-[3/2] w-full items-center justify-center overflow-hidden bg-surface-container-high">
				{showFace && faceUrl ? (
					<Image
						src={faceUrl}
						alt={name}
						fill
						sizes="(max-width: 640px) 50vw, (max-width: 1024px) 33vw, 20vw"
						unoptimized
						className="object-contain transition-transform duration-300 group-hover:scale-110"
						onError={() => setImageFailed(true)}
					/>
				) : (
					<ClipboardList
						size={40}
						className={cn(
							"opacity-30 transition-transform duration-300 group-hover:scale-110",
							accent
						)}
						aria-hidden="true"
					/>
				)}
				{elementKey ? (
					<div className="absolute right-2 top-2 rounded-full bg-surface/80 p-1 backdrop-blur-sm">
						<ElementIcon element={elementKey} size="sm" />
					</div>
				) : null}
			</div>

			<div className="flex flex-1 flex-col gap-1 p-3">
				<span className={cn("text-[10px] font-bold uppercase tracking-wider", accent)}>
					{roleFr}
					{playstyleFr ? ` · ${playstyleFr}` : ""}
				</span>
				<h3 className="line-clamp-1 text-sm font-bold leading-tight text-on-surface transition-colors group-hover:text-primary">
					{name}
				</h3>

				{stat ? (
					<div className="mt-auto flex items-center justify-between gap-2 pt-1">
						<span className="line-clamp-1 text-xs text-on-surface-variant">{stat}</span>
						{buff ? (
							<span className="shrink-0 rounded-full bg-primary-container px-2 py-0.5 text-[11px] font-bold text-on-primary-container">
								{buff}
							</span>
						) : null}
					</div>
				) : (
					<span className="mt-auto pt-1 text-xs italic text-on-surface-variant/50">
						{elementFr ?? "—"}
					</span>
				)}
			</div>
		</Link>
	);
}
