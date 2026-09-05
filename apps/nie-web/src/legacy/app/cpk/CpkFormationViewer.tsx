"use client";

/**
 * Viewer de `formation_config` : terrain interactif affichant les 11 joueurs d'une
 * formation, depuis les données TYPÉES de `/typed` (nie-model-serve → nie-data).
 *
 * Remplace les positions approximées codées en dur de `lib/formations.ts` (extraites
 * du CSS de zukan.inazuma.jp) par les VRAIES coordonnées f32 byte-exactes du jeu.
 *
 * Données : `{ formations[], placements[], positions[] }`. Une formation référence sa
 * tranche `placements[placement_offset .. +placement_count]` (11 slots), chacun portant
 * `start_pos / offense_pos / defense_pos` en `{x, y}` f32. Les coordonnées sont
 * normalisées par boîte englobante (robuste à la convention d'axes du jeu).
 */
import { useMemo, useState } from "react";
import { Icon } from "@/components/ui/Icon";

interface Vec2 {
	x: number;
	y: number;
}
interface Placement {
	position_no: number;
	position_id: number;
	pass_no?: number;
	b_kickoff?: boolean;
	b_follow?: boolean;
	start_pos?: Vec2;
	offense_pos?: Vec2;
	defense_pos?: Vec2;
}
interface Formation {
	form_id: number;
	placement_offset: number;
	placement_count: number;
	power_offense: number;
	power_defense: number;
	noun_id?: number;
}
interface FormationData {
	formations?: Formation[];
	placements?: Placement[];
}

type PosField = "start_pos" | "offense_pos" | "defense_pos";

type Role = "GK" | "DF" | "MF" | "FW";

/** Style (libellé + couleur) par rôle de jeu. */
const ROLE_STYLE: Record<Role, { label: string; cls: string }> = {
	GK: { label: "GB", cls: "bg-amber-500 text-black" },
	DF: { label: "DF", cls: "bg-sky-500 text-white" },
	MF: { label: "MI", cls: "bg-emerald-500 text-black" },
	FW: { label: "AT", cls: "bg-red-500 text-white" },
};

/**
 * Rôle déduit de `position_id` (1..10, type de poste), via les poids off/déf de
 * `m_SoccerPositionInfoList` : GK=1, DF=2-3, MF=4-7, FW=8-10 (identique à
 * `export_formations`). L'ancien mapping 1-4 affichait « ? » pour les postes 5-10.
 */
function roleOf(positionId: number): Role {
	if (positionId === 1) return "GK";
	if (positionId <= 3) return "DF";
	if (positionId <= 7) return "MF";
	return "FW";
}

function hex(n: number): string {
	return `0x${(n >>> 0).toString(16).toUpperCase().padStart(8, "0")}`;
}

export function CpkFormationViewer({ data }: { data: FormationData }) {
	// Mémoïsé : `data.formations ?? []` créerait un nouveau tableau à chaque render
	// (deps instables des useMemo en aval, cf. react-hooks/exhaustive-deps).
	const formations = useMemo(() => data.formations ?? [], [data]);
	const placements = useMemo(() => data.placements ?? [], [data]);
	const [idx, setIdx] = useState(0);
	const [field, setField] = useState<PosField>("start_pos");

	const formation = formations[idx];

	const slots = useMemo(() => {
		if (!formation) return [];
		const start = formation.placement_offset;
		const end = start + formation.placement_count;
		return placements.slice(start, end);
	}, [formation, placements]);

	// Boîte englobante du champ de position choisi → normalisation [0,1] avec marge.
	const norm = useMemo(() => {
		const pts = slots.map((s) => s[field]).filter((p): p is Vec2 => !!p);
		if (pts.length === 0) return null;
		const xs = pts.map((p) => p.x);
		const ys = pts.map((p) => p.y);
		const minX = Math.min(...xs);
		const maxX = Math.max(...xs);
		const minY = Math.min(...ys);
		const maxY = Math.max(...ys);
		const spanX = maxX - minX || 1;
		const spanY = maxY - minY || 1;
		return { minX, minY, spanX, spanY };
	}, [slots, field]);

	if (!formation) {
		return <p className="p-6 text-sm text-on-surface-variant">Aucune formation dans ce fichier.</p>;
	}

	return (
		<div className="p-4 space-y-4">
			{/* Sélecteur de formation + champ de position */}
			<div className="flex flex-wrap items-center gap-3">
				<label className="flex items-center gap-2 text-sm">
					<Icon name="grid_view" size={16} className="text-on-surface-variant" />
					<select
						value={idx}
						onChange={(e) => setIdx(Number(e.target.value))}
						className="h-10 rounded-xl border border-outline-variant/40 bg-surface-container-low px-3 text-on-surface"
					>
						{formations.map((f, i) => (
							<option key={f.form_id} value={i}>
								#{i + 1} · {hex(f.form_id)} (ATT {f.power_offense} / DÉF {f.power_defense})
							</option>
						))}
					</select>
				</label>
				<div className="inline-flex rounded-full border border-outline-variant/40 overflow-hidden text-xs">
					{(["start_pos", "offense_pos", "defense_pos"] as PosField[]).map((f) => (
						<button
							key={f}
							type="button"
							onClick={() => setField(f)}
							className={`h-9 px-3 font-semibold transition ${
								field === f
									? "bg-primary text-on-primary"
									: "bg-surface-container-low text-on-surface-variant hover:text-on-surface"
							}`}
						>
							{f === "start_pos" ? "Départ" : f === "offense_pos" ? "Attaque" : "Défense"}
						</button>
					))}
				</div>
				<span className="text-xs text-on-surface-variant font-mono">{slots.length} joueurs</span>
			</div>

			{/* Terrain */}
			<div
				className="relative mx-auto w-full max-w-sm rounded-2xl border border-emerald-700/40 overflow-hidden"
				style={{
					aspectRatio: "2 / 3",
					background:
						"repeating-linear-gradient(0deg,#14532d 0px,#14532d 28px,#166534 28px,#166534 56px)",
				}}
			>
				{/* Lignes de terrain */}
				<div className="absolute inset-3 border-2 border-white/30 rounded-lg" />
				<div className="absolute left-3 right-3 top-1/2 h-px bg-white/30" />
				<div className="absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 size-16 rounded-full border-2 border-white/30" />

				{norm &&
					slots.map((s, i) => {
						const p = s[field];
						if (!p) return null;
						const left = 8 + ((p.x - norm.minX) / norm.spanX) * 84;
						const top = 6 + ((p.y - norm.minY) / norm.spanY) * 88;
						const role = ROLE_STYLE[roleOf(s.position_id)];
						return (
							<div
								key={`${s.position_no}-${i}`}
								className="absolute -translate-x-1/2 -translate-y-1/2 flex flex-col items-center"
								style={{ left: `${left}%`, top: `${top}%` }}
								title={`pos ${s.position_no} · id ${s.position_id} · pass ${s.pass_no ?? "-"}${
									s.b_kickoff ? " · coup d'envoi" : ""
								}`}
							>
								<span
									className={`size-7 rounded-full grid place-items-center text-[10px] font-bold shadow ${role.cls} ${
										s.b_kickoff ? "ring-2 ring-white" : ""
									}`}
								>
									{role.label}
								</span>
							</div>
						);
					})}
			</div>

			{/* Légende */}
			<div className="flex flex-wrap justify-center gap-3 text-xs text-on-surface-variant">
				{Object.values(ROLE_STYLE).map((r) => (
					<span key={r.label} className="inline-flex items-center gap-1.5">
						<span className={`size-3 rounded-full ${r.cls}`} /> {r.label}
					</span>
				))}
				<span className="inline-flex items-center gap-1.5">
					<span className="size-3 rounded-full bg-white/80 ring-2 ring-white" /> coup d'envoi
				</span>
			</div>
		</div>
	);
}
