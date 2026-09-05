import { ImageResponse } from "next/og";
import { getOgLogoDataUri } from "@/lib/og-logo";

export const alt = "Objets - Wiki Azalee";
export const size = { height: 630, width: 1200 };
export const contentType = "image/png";

const ACCENT = "#66BB6A";

const ITEM_TYPES = [
	{ emoji: "👟", label: "Chaussures" },
	{ emoji: "⌚", label: "Bracelets" },
	{ emoji: "📿", label: "Pendentifs" },
	{ emoji: "🔗", label: "Liens Kizuna" },
	{ emoji: "🧪", label: "Consommables" },
];

export default async function Image() {
	return new ImageResponse(
		<div
			style={{
				background: "linear-gradient(135deg, #1A120D 0%, #2D1F14 50%, #1A120D 100%)",
				color: "white",
				display: "flex",
				fontFamily: "sans-serif",
				height: "100%",
				overflow: "hidden",
				position: "relative",
				width: "100%",
			}}
		>
			{/* Top bar */}
			<div
				style={{
					background: `linear-gradient(90deg, #F2A93B, ${ACCENT}, #F2A93B)`,
					display: "flex",
					height: 4,
					left: 0,
					position: "absolute",
					right: 0,
					top: 0,
				}}
			/>

			{/* Background glow */}
			<div
				style={{
					background: `radial-gradient(circle, ${ACCENT}10 0%, transparent 70%)`,
					borderRadius: "50%",
					display: "flex",
					height: 450,
					position: "absolute",
					right: -80,
					top: -80,
					width: 450,
				}}
			/>

			{/* Content */}
			<div
				style={{
					display: "flex",
					flex: 1,
					flexDirection: "column",
					gap: 20,
					justifyContent: "center",
					padding: "60px 80px",
				}}
			>
				<div
					style={{
						color: ACCENT,
						display: "flex",
						fontSize: 16,
						fontWeight: 700,
						letterSpacing: 4,
						textTransform: "uppercase",
					}}
				>
					Wiki Azalee
				</div>

				<div
					style={{
						color: "white",
						display: "flex",
						fontSize: 64,
						fontWeight: 900,
						lineHeight: 1.1,
					}}
				>
					Objets
				</div>

				<div
					style={{
						color: "#A09080",
						display: "flex",
						fontSize: 24,
						lineHeight: 1.4,
						maxWidth: 600,
					}}
				>
					3 000+ equipements, consommables et liens Kizuna
				</div>

				{/* Item type pills */}
				<div style={{ display: "flex", flexWrap: "wrap", gap: 12, marginTop: 12 }}>
					{ITEM_TYPES.map((t) => (
						<div
							key={t.label}
							style={{
								alignItems: "center",
								background: `${ACCENT}15`,
								border: `1px solid ${ACCENT}30`,
								borderRadius: 20,
								display: "flex",
								gap: 8,
								padding: "8px 18px",
							}}
						>
							<div style={{ display: "flex", fontSize: 16 }}>{t.emoji}</div>
							<div style={{ color: ACCENT, display: "flex", fontSize: 14, fontWeight: 700 }}>
								{t.label}
							</div>
						</div>
					))}
				</div>
			</div>

			{/* Right side stats */}
			<div
				style={{
					alignItems: "center",
					display: "flex",
					flexDirection: "column",
					gap: 12,
					justifyContent: "center",
					padding: "40px 60px 40px 0",
					width: 300,
				}}
			>
				{["Frappe", "Controle", "Technique", "Pression", "Physique", "Agilite", "Intelligence"].map(
					(s) => (
						<div key={s} style={{ alignItems: "center", display: "flex", gap: 10, width: "100%" }}>
							<div
								style={{
									color: "#6D5E50",
									display: "flex",
									fontSize: 12,
									fontWeight: 600,
									justifyContent: "flex-end",
									textAlign: "right",
									width: 80,
								}}
							>
								{s}
							</div>
							<div
								style={{
									background: `${ACCENT}30`,
									borderRadius: 8,
									display: "flex",
									padding: "3px 10px",
								}}
							>
								<div style={{ color: ACCENT, display: "flex", fontSize: 14, fontWeight: 800 }}>
									+??
								</div>
							</div>
						</div>
					)
				)}
			</div>

			{/* Branding */}
			<div
				style={{
					alignItems: "center",
					bottom: 20,
					display: "flex",
					gap: 10,
					left: 80,
					position: "absolute",
				}}
			>
				<img
					src={await getOgLogoDataUri()}
					width={28}
					height={28}
					style={{ borderRadius: 7 }}
					alt="Logo Azalée"
				/>
				<div style={{ color: "#F2A93B", display: "flex", fontSize: 14, fontWeight: 700 }}>
					azalee.rosegriffon.fr
				</div>
				<div style={{ color: "#6D5E50", display: "flex", fontSize: 12 }}>
					Inazuma Eleven: Victory Road
				</div>
			</div>
		</div>,
		{ ...size }
	);
}
