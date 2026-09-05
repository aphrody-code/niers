/**
 * La coquille « menu principal » — la direction artistique du jeu, pour Aphrody.
 *
 * Ces composants ne dessinent que des FORMES : ils ne savent rien des donnees, ne lisent aucune
 * source, et ne connaissent pas leur hote. Ce qui les rend reutilisables tels quels dans
 * Inacord comme dans Aphrody.
 *
 * Le trait qui signe l'interface du jeu est le BISEAU : les tuiles du menu ne sont pas des
 * rectangles arrondis mais des parallelogrammes coupes, empiles en rangees decalees. Tout le
 * reste — couleurs, ombres, durees — vient de `game-tokens.css`, mesure sur la reference
 * archivee.
 */
import type { CSSProperties, ReactNode } from "react";

/** Fusionne des classes en ignorant les vides — evite une dependance pour trois lignes. */
function cx(...parties: (string | false | null | undefined)[]): string {
	return parties.filter(Boolean).join(" ");
}

/** Un biseau en `clip-path`, incline vers la droite comme dans le jeu. */
function biseau(inverse = false): CSSProperties {
	const b = "var(--jeu-biseau)";
	return {
		clipPath: inverse
			? `polygon(${b} 0, 100% 0, calc(100% - ${b}) 100%, 0 100%)`
			: `polygon(0 0, calc(100% - ${b}) 0, 100% 100%, ${b} 100%)`,
	};
}

/**
 * Une tuile biseautee du menu.
 *
 * `actif` marque l'entree courante, `sourdine` une entree indisponible — et les deux se lisent
 * SANS la couleur : l'accent s'accompagne d'un liser et d'un decalage. Une interface qui ne
 * signale l'etat que par la teinte exclut qui ne la distingue pas.
 */
export function SkewTile({
	children,
	actif = false,
	sourdine = false,
	onClick,
	className,
}: {
	children: ReactNode;
	actif?: boolean;
	sourdine?: boolean;
	onClick?: () => void;
	className?: string;
}) {
	const interactif = Boolean(onClick) && !sourdine;
	return (
		<button
			type="button"
			onClick={interactif ? onClick : undefined}
			disabled={sourdine}
			aria-current={actif ? "true" : undefined}
			className={cx("jeu-tuile", actif && "jeu-tuile--actif", sourdine && "jeu-tuile--sourdine", className)}
			style={{
				...biseau(),
				background: actif ? "var(--jeu-fond-moyen)" : "var(--jeu-fond-nuit)",
				color: sourdine ? "var(--jeu-surface-cendre)" : "var(--jeu-texte-vif)",
				borderLeft: actif
					? "var(--jeu-bordure) solid var(--jeu-accent-cyan)"
					: "var(--jeu-bordure) solid transparent",
				padding: "var(--jeu-espace-m) var(--jeu-espace-l)",
				font: "inherit",
				fontWeight: "var(--jeu-titre-poids)" as CSSProperties["fontWeight"],
				letterSpacing: "var(--jeu-libelle-espacement)",
				textAlign: "left",
				cursor: interactif ? "pointer" : sourdine ? "not-allowed" : "default",
				opacity: sourdine ? 0.55 : 1,
				transform: actif ? "translateX(var(--jeu-espace-s))" : "none",
				transition: "transform var(--jeu-duree-rapide) var(--jeu-courbe), background var(--jeu-duree-rapide) var(--jeu-courbe)",
				boxShadow: actif ? "var(--jeu-lueur-accent)" : "var(--jeu-ombre-tuile)",
			}}
		>
			{children}
		</button>
	);
}

/**
 * Une rangee de tuiles, decalees en escalier.
 *
 * Le decalage est ce qui donne au menu son mouvement : chaque tuile recule d'un cran par
 * rapport a la precedente. `pas` le regle ; a zero, les tuiles s'alignent.
 */
export function TileRow({ children, pas = 12 }: { children: ReactNode[]; pas?: number }) {
	return (
		<div style={{ display: "flex", flexDirection: "column", gap: "var(--jeu-espace-xs)" }}>
			{children.map((tuile, i) => (
				// biome-ignore lint/suspicious/noArrayIndexKey: l'ordre EST l'identite d'une rangee
				<div key={i} style={{ marginLeft: `${i * pas}px` }}>
					{tuile}
				</div>
			))}
		</div>
	);
}

/** La banniere de tete : titre du jeu a gauche, actions a droite. */
export function HeaderBanner({ titre, actions }: { titre: ReactNode; actions?: ReactNode }) {
	return (
		<header
			style={{
				display: "flex",
				alignItems: "center",
				justifyContent: "space-between",
				gap: "var(--jeu-espace-m)",
				padding: "var(--jeu-espace-m) var(--jeu-espace-xl)",
				background: "linear-gradient(90deg, var(--jeu-fond-abysse), var(--jeu-fond-nuit))",
				borderBottom: "var(--jeu-bordure) solid var(--jeu-accent-azur)",
				color: "var(--jeu-texte-vif)",
			}}
		>
			<div style={{ fontWeight: 800, letterSpacing: "var(--jeu-titre-espacement)" }}>{titre}</div>
			{actions ? <div style={{ display: "flex", gap: "var(--jeu-espace-s)" }}>{actions}</div> : null}
		</header>
	);
}

/** Le panneau lateral : biseaute a l'oppose des tuiles, pour cadrer la rangee. */
export function SidePanel({ children, largeur = 320 }: { children: ReactNode; largeur?: number }) {
	return (
		<aside
			style={{
				...biseau(true),
				width: largeur,
				flex: "0 0 auto",
				padding: "var(--jeu-espace-l)",
				background: "var(--jeu-fond-abysse)",
				color: "var(--jeu-surface-craie)",
				boxShadow: "var(--jeu-ombre-panneau)",
			}}
		>
			{children}
		</aside>
	);
}

/** Le bandeau de titre d'une section, souligne d'ambre. */
export function TitleBand({ children }: { children: ReactNode }) {
	return (
		<h2
			style={{
				margin: 0,
				padding: "var(--jeu-espace-s) 0",
				borderBottom: "var(--jeu-bordure) solid var(--jeu-accent-ambre)",
				color: "var(--jeu-texte-vif)",
				fontWeight: 800,
				letterSpacing: "var(--jeu-titre-espacement)",
				textTransform: "uppercase",
			}}
		>
			{children}
		</h2>
	);
}

/** La pastille de version, en bas d'ecran comme dans le jeu. */
export function VersionChip({ version }: { version: string }) {
	return (
		<span
			style={{
				display: "inline-block",
				padding: "2px var(--jeu-espace-s)",
				borderRadius: "var(--jeu-rayon)",
				background: "rgb(15 16 17 / 60%)",
				color: "var(--jeu-surface-cendre)",
				fontSize: "0.75rem",
				letterSpacing: "var(--jeu-libelle-espacement)",
			}}
		>
			{version}
		</span>
	);
}

/**
 * Un encart d'information ou d'alerte.
 *
 * `ton` porte le sens ; `role="alert"` n'est pose que pour `alerte`, sinon les lecteurs d'ecran
 * interrompraient leur utilisateur pour une simple note.
 */
export function Callout({
	children,
	ton = "info",
}: {
	children: ReactNode;
	ton?: "info" | "alerte" | "succes";
}) {
	const teinte = {
		info: "var(--jeu-accent-azur)",
		alerte: "var(--jeu-accent-brique)",
		succes: "var(--jeu-accent-turquoise)",
	}[ton];
	return (
		<div
			role={ton === "alerte" ? "alert" : undefined}
			style={{
				borderLeft: "4px solid " + teinte,
				padding: "var(--jeu-espace-s) var(--jeu-espace-m)",
				background: "rgb(48 66 98 / 45%)",
				color: "var(--jeu-surface-glace)",
			}}
		>
			{children}
		</div>
	);
}

/** Une pastille de comptage ou d'etat. */
export function Badge({ children, teinte = "cyan" }: { children: ReactNode; teinte?: "cyan" | "ambre" | "brique" }) {
	const fond = {
		cyan: "var(--jeu-accent-cyan)",
		ambre: "var(--jeu-accent-ambre)",
		brique: "var(--jeu-accent-brique)",
	}[teinte];
	return (
		<span
			style={{
				display: "inline-flex",
				alignItems: "center",
				minWidth: "1.5em",
				justifyContent: "center",
				padding: "0 var(--jeu-espace-xs)",
				borderRadius: "999px",
				background: fond,
				color: "var(--jeu-fond-abysse)",
				fontSize: "0.75rem",
				fontWeight: 800,
			}}
		>
			{children}
		</span>
	);
}
