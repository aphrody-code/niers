import { ImageResponse } from "next/og";
import { getOgLogoDataUri } from "@/lib/og-logo";

export const alt = "Joueurs - Wiki Azalee";
export const size = { height: 630, width: 1200 };
export const contentType = "image/png";

const ACCENT = "#42A5F5";

const POSITIONS = [
	{ abbr: "FW", color: "#E53935", label: "Attaquant" },
	{ abbr: "MF", color: "#43A047", label: "Milieu" },
	{ abbr: "DF", color: "#29B6F6", label: "Defenseur" },
	{ abbr: "GK", color: "#F9A825", label: "Gardien" },
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
					background: `radial-gradient(circle, ${ACCENT}12 0%, transparent 70%)`,
					borderRadius: "50%",
					display: "flex",
					height: 500,
					position: "absolute",
					right: -100,
					top: -100,
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
					Joueurs
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
					5 900+ personnages avec stats, recrutement et techniques
				</div>

				{/* Position pills */}
				<div style={{ display: "flex", gap: 12, marginTop: 12 }}>
					{POSITIONS.map((p) => (
						<div
							key={p.abbr}
							style={{
								alignItems: "center",
								background: `${p.color}18`,
								border: `1px solid ${p.color}35`,
								borderRadius: 20,
								display: "flex",
								gap: 8,
								padding: "8px 18px",
							}}
						>
							<div style={{ color: p.color, display: "flex", fontSize: 14, fontWeight: 800 }}>
								{p.abbr}
							</div>
							<div
								style={{ color: `${p.color}CC`, display: "flex", fontSize: 14, fontWeight: 600 }}
							>
								{p.label}
							</div>
						</div>
					))}
				</div>
			</div>

			{/* Right side stats preview */}
			<div
				style={{
					alignItems: "center",
					display: "flex",
					flexDirection: "column",
					gap: 16,
					justifyContent: "center",
					padding: "40px 60px 40px 0",
					width: 320,
				}}
			>
				{["Frappe", "Controle", "Technique", "Pression", "Physique", "Agilite", "Intelligence"].map(
					(s, i) => (
						<div key={s} style={{ alignItems: "center", display: "flex", gap: 12, width: "100%" }}>
							<div
								style={{
									color: "#6D5E50",
									display: "flex",
									fontSize: 13,
									fontWeight: 600,
									justifyContent: "flex-end",
									textAlign: "right",
									width: 90,
								}}
							>
								{s}
							</div>
							<div
								style={{
									background: "#3D2E22",
									borderRadius: 4,
									display: "flex",
									flex: 1,
									height: 8,
									overflow: "hidden",
								}}
							>
								<div
									style={{
										background: `linear-gradient(90deg, ${ACCENT}80, ${ACCENT})`,
										borderRadius: 4,
										display: "flex",
										height: "100%",
										width: `${55 + i * 6}%`,
									}}
								/>
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
