import { ImageResponse } from "next/og";
import { getOgLogoDataUri } from "@/lib/og-logo";

export const alt = "Tactiques - Wiki Azalee";
export const size = { height: 630, width: 1200 };
export const contentType = "image/png";

const ACCENT = "#26C6DA";

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
					top: -100,
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
					Tactiques
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
					100+ tactiques speciales de match
				</div>

				{/* Info pills */}
				<div style={{ display: "flex", gap: 12, marginTop: 12 }}>
					{[
						{ label: "Duree", value: "Variable" },
						{ label: "Cooldown", value: "Par match" },
						{ label: "Effets", value: "Boost/Debuff" },
					].map((p) => (
						<div
							key={p.label}
							style={{
								alignItems: "center",
								background: `${ACCENT}12`,
								border: `1px solid ${ACCENT}25`,
								borderRadius: 16,
								display: "flex",
								flexDirection: "column",
								gap: 4,
								padding: "12px 20px",
							}}
						>
							<div
								style={{
									color: "#6D5E50",
									display: "flex",
									fontSize: 11,
									fontWeight: 700,
									letterSpacing: 1,
									textTransform: "uppercase",
								}}
							>
								{p.label}
							</div>
							<div style={{ color: ACCENT, display: "flex", fontSize: 16, fontWeight: 800 }}>
								{p.value}
							</div>
						</div>
					))}
				</div>
			</div>

			{/* Right side - tactical field preview */}
			<div
				style={{
					alignItems: "center",
					display: "flex",
					justifyContent: "center",
					padding: "40px 60px 40px 0",
					width: 300,
				}}
			>
				<div
					style={{
						alignItems: "center",
						background: `${ACCENT}08`,
						border: `2px solid ${ACCENT}30`,
						borderRadius: 20,
						display: "flex",
						flexDirection: "column",
						gap: 12,
						height: 280,
						justifyContent: "center",
						position: "relative",
						width: 200,
					}}
				>
					{/* Field lines */}
					<div
						style={{
							background: `${ACCENT}25`,
							display: "flex",
							height: 1,
							left: 10,
							position: "absolute",
							right: 10,
							top: "50%",
						}}
					/>
					<div
						style={{
							border: `1px solid ${ACCENT}25`,
							borderRadius: "50%",
							display: "flex",
							height: 60,
							left: "50%",
							position: "absolute",
							top: "50%",
							transform: "translate(-50%, -50%)",
							width: 60,
						}}
					/>
					{/* Player dots */}
					{[
						{ left: "50%", top: "15%" },
						{ left: "25%", top: "35%" },
						{ left: "75%", top: "35%" },
						{ left: "30%", top: "55%" },
						{ left: "70%", top: "55%" },
						{ left: "50%", top: "75%" },
					].map((pos, i) => (
						<div
							key={i}
							style={{
								background: ACCENT,
								borderRadius: "50%",
								boxShadow: `0 0 8px ${ACCENT}60`,
								display: "flex",
								height: 12,
								left: pos.left,
								position: "absolute",
								top: pos.top,
								transform: "translate(-50%, -50%)",
								width: 12,
							}}
						/>
					))}
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
