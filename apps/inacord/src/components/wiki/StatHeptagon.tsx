"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { SpriteIcon } from "@/components/ui/SpriteIcon";
import type { SpriteKey } from "@/config/sprites";
import { cn } from "@/lib/utils";

export interface StatHeptagonProps {
	stats: {
		kick: number; // FRAPPE
		control: number; // CONTRÔLE
		technique: number; // TECHNIQUE
		pressure: number; // PRESSION
		physical: number; // PHYSIQUE
		agility: number; // AGILITÉ
		intelligence: number; // INTELLIGENCE
	};
	size?: number;
	showLabels?: boolean;
	className?: string;
}

// Position sprites mapping
const POSITION_SPRITES: Record<string, SpriteKey> = {
	ATT: "role-att-tag",
	DEF: "role-def-simple",
	GAR: "role-gar-simple",
	MIL: "role-mil-simple",
};

// Stats configuration with labels and relevant positions
const STATS_CONFIG = [
	{ key: "kick", label: "FRAPPE", positions: ["ATT"] },
	{ key: "control", label: "CONTRÔLE", positions: ["ATT", "MIL"] },
	{ key: "pressure", label: "PRESSION", positions: ["GAR", "DEF"] },
	{ key: "physical", label: "PHYSIQUE", positions: ["GAR", "DEF"] },
	{ key: "agility", label: "AGILITÉ", positions: ["GAR"] },
	{ key: "intelligence", label: "INTELLIGENCE", positions: ["DEF", "MIL"] },
	{ key: "technique", label: "TECHNIQUE", positions: ["ATT", "MIL"] },
] as const;

/**
 * Heptagonal (7-sided) stat radar chart matching the official game design
 */
export function StatHeptagon({
	stats,
	size: sizeProp = 280,
	showLabels = true,
	className,
}: StatHeptagonProps) {
	const containerRef = useRef<HTMLDivElement>(null);
	const [containerSize, setContainerSize] = useState(sizeProp);

	useEffect(() => {
		if (!containerRef.current) {
			return;
		}
		const observer = new ResizeObserver((entries) => {
			const width = entries[0]?.contentRect.width;
			if (width) {
				setContainerSize(Math.min(sizeProp, width));
			}
		});
		observer.observe(containerRef.current);
		return () => observer.disconnect();
	}, [sizeProp]);

	const size = containerSize;
	const center = size / 2;
	const radius = size * 0.32; // Inner hexagon radius
	const labelRadius = size * 0.46; // Where labels are positioned

	// Calculate heptagon points (7 vertices, starting from top)
	const getPoint = useCallback(
		(index: number, r: number) => {
			const angle = (Math.PI * 2 * index) / 7 - Math.PI / 2;
			return {
				x: center + r * Math.cos(angle),
				y: center + r * Math.sin(angle),
			};
		},
		[center]
	);

	// Background heptagon points
	const heptagonPoints = useMemo(
		() =>
			Array.from({ length: 7 }, (_, i) => {
				const p = getPoint(i, radius);
				return `${p.x},${p.y}`;
			}).join(" "),
		[radius, getPoint]
	);

	// Inner grid rings
	const innerRings = useMemo(
		() =>
			[0.25, 0.5, 0.75].map((scale) => {
				return Array.from({ length: 7 }, (_, i) => {
					const p = getPoint(i, radius * scale);
					return `${p.x},${p.y}`;
				}).join(" ");
			}),
		[radius, getPoint]
	);

	// Stat polygon points (normalized to max 999)
	const statPoints = useMemo(() => {
		const statValues = [
			stats.kick,
			stats.control,
			stats.pressure,
			stats.physical,
			stats.agility,
			stats.intelligence,
			stats.technique,
		];
		return statValues
			.map((val, i) => {
				const normalized = Math.min(1, Math.max(0, val / 999));
				const p = getPoint(i, radius * normalized);
				return `${p.x},${p.y}`;
			})
			.join(" ");
	}, [stats, radius, getPoint]);

	// Label positions
	const labelPositions = useMemo(
		() =>
			STATS_CONFIG.map((config, i) => {
				const p = getPoint(i, labelRadius);
				const statValue = stats[config.key as keyof typeof stats];
				return {
					...config,
					x: p.x,
					y: p.y,
					value: statValue,
				};
			}),
		[stats, labelRadius, getPoint]
	);

	return (
		<div
			ref={containerRef}
			className={cn("relative w-full max-w-full", className)}
			style={{ height: size, maxWidth: sizeProp }}
		>
			<svg width={size} height={size} viewBox={`0 0 ${size} ${size}`} className="absolute inset-0">
				{/* Definitions for gradients and patterns */}
				<defs>
					{/* Dark background gradient */}
					<radialGradient id="heptBg" cx="50%" cy="50%" r="50%">
						<stop offset="0%" stopColor="#3a4a5c" />
						<stop offset="100%" stopColor="#1a2a3c" />
					</radialGradient>

					{/* Stat polygon gradient */}
					<linearGradient id="statGradient" x1="0%" y1="0%" x2="100%" y2="100%">
						<stop offset="0%" stopColor="#fbbf24" />
						<stop offset="50%" stopColor="#f59e0b" />
						<stop offset="100%" stopColor="#d97706" />
					</linearGradient>

					{/* Soccer ball pattern (simplified) */}
					<pattern id="soccerPattern" patternUnits="userSpaceOnUse" width="40" height="40">
						<circle
							cx="20"
							cy="20"
							r="8"
							fill="none"
							stroke="#4a5a6c"
							strokeWidth="0.5"
							opacity="0.3"
						/>
						<path
							d="M20 12 L26 18 L24 26 L16 26 L14 18 Z"
							fill="none"
							stroke="#4a5a6c"
							strokeWidth="0.5"
							opacity="0.3"
						/>
					</pattern>
				</defs>

				{/* Outer glow/shadow */}
				<circle
					cx={center}
					cy={center}
					r={radius + 8}
					fill="none"
					stroke="#0a1a2c"
					strokeWidth="16"
					opacity="0.3"
				/>

				{/* Background heptagon with pattern */}
				<polygon points={heptagonPoints} fill="url(#heptBg)" stroke="#2a3a4c" strokeWidth="2" />

				{/* Soccer ball pattern overlay */}
				<polygon points={heptagonPoints} fill="url(#soccerPattern)" opacity="0.5" />

				{/* Inner grid rings */}
				{innerRings.map((ring, i) => (
					<polygon
						key={i}
						points={ring}
						fill="none"
						stroke="#4a5a6c"
						strokeWidth="0.5"
						opacity="0.4"
					/>
				))}

				{/* Axis lines from center to vertices */}
				{Array.from({ length: 7 }).map((_, i) => {
					const p = getPoint(i, radius);
					return (
						<line
							key={i}
							x1={center}
							y1={center}
							x2={p.x}
							y2={p.y}
							stroke="#4a5a6c"
							strokeWidth="0.5"
							opacity="0.4"
						/>
					);
				})}

				{/* Stat polygon */}
				<polygon
					points={statPoints}
					fill="url(#statGradient)"
					fillOpacity="0.7"
					stroke="#fbbf24"
					strokeWidth="2"
				/>

				{/* Center dot */}
				<circle cx={center} cy={center} r="4" fill="#fbbf24" />

				{/* Outer decorative ring */}
				<circle
					cx={center}
					cy={center}
					r={radius + 2}
					fill="none"
					stroke="#5a6a7c"
					strokeWidth="1"
					opacity="0.6"
				/>
			</svg>

			{/* Labels around the heptagon */}
			{showLabels &&
				labelPositions.map((label) => (
					<div
						key={label.key}
						className="absolute flex flex-col items-center gap-0 sm:gap-0.5 -translate-x-1/2 -translate-y-1/2"
						style={{
							left: label.x,
							top: label.y,
						}}
					>
						{/* Position sprites — hidden on very small screens to save space */}
						<div className="hidden sm:flex gap-0.5 items-center">
							{label.positions.map((pos) => (
								<SpriteIcon
									key={pos}
									name={POSITION_SPRITES[pos]}
									scale={0.35}
									className="drop-shadow-sm"
								/>
							))}
						</div>

						{/* Stat value */}
						<span className="text-xs sm:text-lg font-black text-slate-700 dark:text-slate-200 leading-none">
							{label.value}
						</span>

						{/* Stat name */}
						<span className="text-[7px] sm:text-[9px] font-bold text-slate-500 dark:text-slate-400 uppercase tracking-wide">
							{label.label}
						</span>
					</div>
				))}
		</div>
	);
}

/**
 * Compact version for cards (no labels, smaller size)
 */
export function MiniStatHeptagon({
	stats,
	size = 56,
	className,
}: Omit<StatHeptagonProps, "showLabels">) {
	const center = size / 2;
	const radius = size / 2 - 4;

	const getPoint = useCallback(
		(index: number, r: number) => {
			const angle = (Math.PI * 2 * index) / 7 - Math.PI / 2;
			return {
				x: center + r * Math.cos(angle),
				y: center + r * Math.sin(angle),
			};
		},
		[center]
	);

	const heptagonPoints = useMemo(
		() =>
			Array.from({ length: 7 }, (_, i) => {
				const p = getPoint(i, radius);
				return `${p.x},${p.y}`;
			}).join(" "),
		[radius, getPoint]
	);

	const innerRings = useMemo(
		() =>
			[0.33, 0.66].map((scale) => {
				return Array.from({ length: 7 }, (_, i) => {
					const p = getPoint(i, radius * scale);
					return `${p.x},${p.y}`;
				}).join(" ");
			}),
		[radius, getPoint]
	);

	const statPoints = useMemo(() => {
		const statValues = [
			stats.kick,
			stats.control,
			stats.pressure,
			stats.physical,
			stats.agility,
			stats.intelligence,
			stats.technique,
		];
		return statValues
			.map((val, i) => {
				const normalized = Math.min(1, Math.max(0, val / 999));
				const p = getPoint(i, radius * normalized);
				return `${p.x},${p.y}`;
			})
			.join(" ");
	}, [stats, radius, getPoint]);

	// Color based on average
	const avgStat = useMemo(() => {
		const values = [
			stats.kick,
			stats.control,
			stats.technique,
			stats.pressure,
			stats.physical,
			stats.agility,
			stats.intelligence,
		];
		return values.reduce((a, b) => a + b, 0) / values.length;
	}, [stats]);

	const statColor =
		avgStat >= 600
			? "#8b5cf6"
			: avgStat >= 400
				? "#f59e0b"
				: avgStat >= 200
					? "#22c55e"
					: "#ef4444";

	return (
		<svg
			width={size}
			height={size}
			viewBox={`0 0 ${size} ${size}`}
			className={cn("shrink-0", className)}
		>
			<defs>
				<radialGradient id="miniBg" cx="50%" cy="50%" r="50%">
					<stop offset="0%" stopColor="#3a4a5c" />
					<stop offset="100%" stopColor="#1a2a3c" />
				</radialGradient>
			</defs>

			{/* Background */}
			<polygon points={heptagonPoints} fill="url(#miniBg)" stroke="#4a5a6c" strokeWidth="1" />

			{/* Inner rings */}
			{innerRings.map((ring, i) => (
				<polygon
					key={i}
					points={ring}
					fill="none"
					stroke="#4a5a6c"
					strokeWidth="0.3"
					opacity="0.5"
				/>
			))}

			{/* Axis lines */}
			{Array.from({ length: 7 }).map((_, i) => {
				const p = getPoint(i, radius);
				return (
					<line
						key={i}
						x1={center}
						y1={center}
						x2={p.x}
						y2={p.y}
						stroke="#4a5a6c"
						strokeWidth="0.3"
						opacity="0.4"
					/>
				);
			})}

			{/* Stat polygon */}
			<polygon
				points={statPoints}
				fill={statColor}
				fillOpacity="0.5"
				stroke={statColor}
				strokeWidth="1.5"
			/>

			{/* Center */}
			<circle cx={center} cy={center} r="2" fill={statColor} />
		</svg>
	);
}
