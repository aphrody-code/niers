import { ImageResponse } from "next/og";
import { getOgLogoDataUri } from "@/lib/og-logo";

export const alt = "Passifs - Wiki Azalee";
export const size = { height: 630, width: 1200 };
export const contentType = "image/png";

const ACCENT = "#AB47BC";

const PLAYSTYLES = [
	{ color: "#78909C", label: "Général" },
	{ color: "#E53935", label: "Brèche" },
	{ color: "#F9A825", label: "Tension" },
	{ color: "#43A047", label: "Contre-attaque" },
	{ color: "#42A5F5", label: "Lien" },
	{ color: "#FF5722", label: "Jeu Violent" },
	{ color: "#AB47BC", label: "Justice" },
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
					bottom: -100,
					display: "flex",
					height: 500,
					left: "50%",
					position: "absolute",
					transform: "translateX(-50%)",
					width: 500,
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
					Passifs
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
					700+ talents joueur, personnalises et coordinateur
				</div>

				{/* Playstyle pills */}
				<div style={{ display: "flex", flexWrap: "wrap", gap: 10, marginTop: 12 }}>
					{PLAYSTYLES.map((p) => (
						<div
							key={p.label}
							style={{
								background: `${p.color}15`,
								border: `1px solid ${p.color}30`,
								borderRadius: 20,
								display: "flex",
								padding: "6px 16px",
							}}
						>
							<div style={{ color: p.color, display: "flex", fontSize: 13, fontWeight: 700 }}>
								{p.label}
							</div>
						</div>
					))}
				</div>
			</div>

			{/* Right side - passive card preview */}
			<div
				style={{
					display: "flex",
					flexDirection: "column",
					gap: 12,
					justifyContent: "center",
					padding: "40px 60px 40px 0",
					width: 340,
				}}
			>
				{[
					{ color: "#AB47BC", desc: "Boost conditionnel de stats", name: "Talent Joueur" },
					{ color: "#29B6F6", desc: "Bonus personnalisé", name: "Talent Perso." },
					{ color: "#66BB6A", desc: "Bonus d'équipe", name: "Coordinateur" },
				].map((card) => (
					<div
						key={card.name}
						style={{
							background: "#3D2E2260",
							borderLeft: `3px solid ${card.color}`,
							borderRadius: 16,
							display: "flex",
							flexDirection: "column",
							gap: 4,
							padding: "14px 18px",
						}}
					>
						<div style={{ color: card.color, display: "flex", fontSize: 14, fontWeight: 800 }}>
							{card.name}
						</div>
						<div style={{ color: "#8A7A6D", display: "flex", fontSize: 12 }}>{card.desc}</div>
					</div>
				))}
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
