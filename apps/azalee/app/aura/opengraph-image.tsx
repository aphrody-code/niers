import { ImageResponse } from "next/og";
import { getOgLogoDataUri } from "@/lib/og-logo";

export const alt = "Hyper Techniques - Wiki Azalee";
export const size = { height: 630, width: 1200 };
export const contentType = "image/png";

const ACCENT = "#F9A825";

const AURA_TYPES = [
	{ color: "#F9A825", label: "Esprits Guerriers" },
	{ color: "#AB47BC", label: "Totems" },
	{ color: "#26C6DA", label: "Miximax" },
	{ color: "#E53935", label: "Eveil" },
	{ color: "#66BB6A", label: "Mode Change" },
];

export default async function Image() {
	return new ImageResponse(
		<div
			style={{
				alignItems: "center",
				background: "linear-gradient(135deg, #1A120D 0%, #2D1F14 50%, #1A120D 100%)",
				color: "white",
				display: "flex",
				flexDirection: "column",
				fontFamily: "sans-serif",
				height: "100%",
				justifyContent: "center",
				overflow: "hidden",
				position: "relative",
				width: "100%",
			}}
		>
			{/* Top bar */}
			<div
				style={{
					background: `linear-gradient(90deg, ${AURA_TYPES[0].color}, ${AURA_TYPES[1].color}, ${AURA_TYPES[2].color}, ${AURA_TYPES[3].color}, ${AURA_TYPES[4].color})`,
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
					background: `radial-gradient(circle, ${ACCENT}08 0%, transparent 70%)`,
					borderRadius: "50%",
					display: "flex",
					height: 600,
					left: "50%",
					position: "absolute",
					top: "50%",
					transform: "translate(-50%, -50%)",
					width: 600,
				}}
			/>

			{/* Header */}
			<div
				style={{
					alignItems: "center",
					display: "flex",
					flexDirection: "column",
					gap: 8,
					marginBottom: 20,
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
				<div style={{ color: "white", display: "flex", fontSize: 64, fontWeight: 900 }}>
					Hyper Techniques
				</div>
				<div style={{ color: "#A09080", display: "flex", fontSize: 22 }}>
					Esprits Guerriers, Totems, Miximax et plus
				</div>
			</div>

			{/* Aura type cards */}
			<div style={{ display: "flex", gap: 16, marginTop: 24 }}>
				{AURA_TYPES.map((a) => (
					<div
						key={a.label}
						style={{
							alignItems: "center",
							background: `${a.color}12`,
							border: `1px solid ${a.color}30`,
							borderRadius: 20,
							display: "flex",
							flexDirection: "column",
							gap: 8,
							minWidth: 140,
							padding: "16px 20px",
						}}
					>
						<div
							style={{
								alignItems: "center",
								background: `${a.color}25`,
								borderRadius: 10,
								display: "flex",
								height: 32,
								justifyContent: "center",
								width: 32,
							}}
						>
							<div
								style={{
									background: a.color,
									borderRadius: "50%",
									display: "flex",
									height: 12,
									width: 12,
								}}
							/>
						</div>
						<div
							style={{
								color: a.color,
								display: "flex",
								fontSize: 13,
								fontWeight: 700,
								textAlign: "center",
							}}
						>
							{a.label}
						</div>
					</div>
				))}
			</div>

			{/* Branding */}
			<div
				style={{ alignItems: "center", bottom: 20, display: "flex", gap: 10, position: "absolute" }}
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
