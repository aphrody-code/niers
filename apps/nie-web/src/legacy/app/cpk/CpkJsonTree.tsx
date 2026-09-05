"use client";

/**
 * Arbre JSON repliable, générique et client-safe — remplace le `<pre>` brut de la
 * fiche fichier CPK. Sert l'affichage des `cfg.bin` décodés (typés via `/typed` ou
 * RDBN/T2B génériques via `/cfg`), avec coloration par type et repli par nœud.
 *
 * Aucune dépendance serveur (sqlite/Node) : reçoit la valeur déjà fetchée en prop.
 */
import { useMemo, useState } from "react";
import { Icon } from "@/components/ui/Icon";

/** Profondeur sous laquelle les nœuds sont repliés par défaut (lisibilité). */
const AUTO_OPEN_DEPTH = 2;
/** Nombre d'enfants au-delà duquel on pagine l'affichage d'un tableau. */
const CHUNK = 100;

type Json = unknown;

export interface CpkJsonTreeProps {
	/** La valeur JSON racine à afficher. */
	data: Json;
	/** Étiquette du nœud racine (ex. nom de la famille). */
	rootLabel?: string;
}

/** Vrai si la chaîne ressemble à un hash hex (`0x…` ou octets bruts MAJUSCULE). */
function isHexish(s: string): boolean {
	return /^0x[0-9a-fA-F]+$/.test(s) || /^[0-9A-F]{8,}$/.test(s);
}

export function CpkJsonTree({ data, rootLabel = "racine" }: CpkJsonTreeProps) {
	return (
		<div className="p-4 font-mono text-xs leading-relaxed overflow-auto max-h-[65vh]">
			<TreeNode label={rootLabel} value={data} depth={0} path="$" />
		</div>
	);
}

function TreeNode({
	label,
	value,
	depth,
	path,
}: {
	label: string;
	value: Json;
	depth: number;
	path: string;
}) {
	const [open, setOpen] = useState(depth < AUTO_OPEN_DEPTH);

	const isArray = Array.isArray(value);
	const isObject = value !== null && typeof value === "object" && !isArray;
	const isBranch = isArray || isObject;

	if (!isBranch) {
		return (
			<div className="flex gap-2 py-0.5">
				<span className="text-on-surface-variant shrink-0">{label}:</span>
				<LeafValue value={value} />
			</div>
		);
	}

	const entries: Array<[string, Json]> = isArray
		? (value as Json[]).map((v, i) => [String(i), v])
		: Object.entries(value as Record<string, Json>);
	const count = entries.length;
	const bracket = isArray ? `[${count}]` : `{${count}}`;

	return (
		<div className="py-0.5">
			<button
				type="button"
				onClick={() => setOpen((o) => !o)}
				className="flex items-center gap-1 hover:text-primary transition w-full text-left"
				aria-expanded={open}
			>
				<Icon
					name={open ? "expand_more" : "chevron_right"}
					size={14}
					className="text-on-surface-variant shrink-0"
				/>
				<span className="text-on-surface font-semibold">{label}</span>
				<span className="text-on-surface-variant/60">{bracket}</span>
			</button>
			{open && (
				<div className="ml-3 border-l border-outline-variant/30 pl-3">
					<Children entries={entries} depth={depth} path={path} />
				</div>
			)}
		</div>
	);
}

/** Affiche les enfants d'un nœud, paginés au-delà de `CHUNK` pour les gros tableaux. */
function Children({
	entries,
	depth,
	path,
}: {
	entries: Array<[string, Json]>;
	depth: number;
	path: string;
}) {
	const [shown, setShown] = useState(CHUNK);
	const visible = entries.slice(0, shown);
	return (
		<>
			{visible.map(([k, v]) => (
				<TreeNode
					key={`${path}.${k}`}
					label={k}
					value={v}
					depth={depth + 1}
					path={`${path}.${k}`}
				/>
			))}
			{entries.length > shown && (
				<button
					type="button"
					onClick={() => setShown((s) => s + CHUNK * 5)}
					className="mt-1 text-primary hover:underline"
				>
					+ {entries.length - shown} de plus…
				</button>
			)}
		</>
	);
}

/** Rend une valeur primitive avec coloration par type. */
function LeafValue({ value }: { value: Json }) {
	const { text, cls } = useMemo(() => describe(value), [value]);
	return <span className={`break-all ${cls}`}>{text}</span>;
}

function describe(value: Json): { text: string; cls: string } {
	if (value === null) return { text: "null", cls: "text-on-surface-variant/50" };
	switch (typeof value) {
		case "boolean":
			return { text: String(value), cls: "text-tertiary" };
		case "number":
			return { text: String(value), cls: "text-primary" };
		case "string":
			return {
				text: `"${value}"`,
				cls: isHexish(value) ? "text-secondary" : "text-on-surface",
			};
		default:
			return { text: String(value), cls: "text-on-surface" };
	}
}
