import { ImageResponse } from "next/og";
import { getOgLogoDataUri } from "@/lib/og-logo";

export const alt = "Techniques - Wiki Azalee";
export const size = { height: 630, width: 1200 };
export const contentType = "image/png";

const ACCENT = "#E53935";

const CATEGORIES = [
	{ color: "#E53935", label: "Tir" },
	{ color: "#43A047", label: "Dribble" },
	{ color: "#29B6F6", label: "Defense" },
	{ color: "#F9A825", label: "Arret" },
];

const ELEMENTS = [
	{ color: "#E53935", label: "Feu" },
	{ color: "#43A047", label: "Vent" },
	{ color: "#1B5E20", label: "Foret" },
	{ color: "#F9A825", label: "Montagne" },
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
					background: `linear-gradient(90deg, ${CATEGORIES[0].color}, ${CATEGORIES[1].color}, ${CATEGORIES[2].color}, ${CATEGORIES[3].color})`,
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
					bottom: -150,
					display: "flex",
					height: 500,
					left: -100,
					position: "absolute",
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
					Techniques
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
					900+ super techniques avec videos et statistiques
				</div>

				{/* Category pills */}
				<div style={{ display: "flex", gap: 12, marginTop: 8 }}>
					{CATEGORIES.map((c) => (
						<div
							key={c.label}
							style={{
								background: `${c.color}18`,
								border: `1px solid ${c.color}35`,
								borderRadius: 20,
								display: "flex",
								padding: "8px 18px",
							}}
						>
							<div style={{ color: c.color, display: "flex", fontSize: 15, fontWeight: 700 }}>
								{c.label}
							</div>
						</div>
					))}
				</div>

				{/* Element pills */}
				<div style={{ display: "flex", gap: 12 }}>
					{ELEMENTS.map((e) => (
						<div
							key={e.label}
							style={{
								background: `${e.color}12`,
								border: `1px solid ${e.color}25`,
								borderRadius: 20,
								display: "flex",
								padding: "6px 16px",
							}}
						>
							<div
								style={{ color: `${e.color}CC`, display: "flex", fontSize: 13, fontWeight: 600 }}
							>
								{e.label}
							</div>
						</div>
					))}
				</div>
			</div>

			{/* Right side decorative */}
			<div
				style={{
					alignItems: "center",
					display: "flex",
					flexDirection: "column",
					gap: 20,
					justifyContent: "center",
					padding: "40px 60px 40px 0",
					width: 300,
				}}
			>
				<div
					style={{
						alignItems: "center",
						background: "#3D2E2280",
						border: "1px solid #5D4E4240",
						borderRadius: 24,
						display: "flex",
						flexDirection: "column",
						gap: 8,
						padding: "24px 32px",
					}}
				>
					<div
						style={{
							color: "#A09080",
							display: "flex",
							fontSize: 14,
							fontWeight: 700,
							letterSpacing: 2,
							textTransform: "uppercase",
						}}
					>
						Puissance
					</div>
					<div style={{ color: "#F2A93B", display: "flex", fontSize: 48, fontWeight: 900 }}>
						???
					</div>
					<div style={{ color: "#6D5E50", display: "flex", fontSize: 13 }}>
						TP &bull; Element &bull; Type
					</div>
				</div>
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
