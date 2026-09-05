/**
 * La coquille « InaCord » — l'ambiance de l'application de messagerie du jeu, pour Inacord.
 *
 * Meme regle que la coquille du menu : ces composants ne dessinent que des formes. Ils ne
 * lisent aucune source, ne connaissent pas leur hote, et se montent aussi bien dans une fenetre
 * Tauri que dans un navigateur.
 *
 * Le trait qui la signe est l'HEXAGONE : le fond de l'application du jeu est un maillage
 * hexagonal sombre, et les listes s'y posent en panneaux mats. Les trois teintes viennent de la
 * decision produit (`PLAN.md` J5), pas de la quantification de la reference.
 */
import type { ReactNode } from "react";

/**
 * Le cadre du telephone.
 *
 * Il borne la largeur et centre le contenu : l'application du jeu est verticale, et l'etirer sur
 * un ecran large detruirait la ressemblance. `pleinEcran` leve la contrainte pour les hotes qui
 * disposent de toute la fenetre.
 */
export function PhoneFrame({
	children,
	pleinEcran = false,
}: {
	children: ReactNode;
	pleinEcran?: boolean;
}) {
	return (
		<div
			style={{
				width: "100%",
				maxWidth: pleinEcran ? "none" : 420,
				margin: "0 auto",
				height: "100%",
				display: "flex",
				flexDirection: "column",
				overflow: "hidden",
				borderRadius: pleinEcran ? 0 : "var(--jeu-espace-m)",
				background: "var(--inacord-panneau)",
				color: "var(--jeu-texte-vif)",
				boxShadow: pleinEcran ? "none" : "var(--jeu-ombre-panneau)",
			}}
		>
			{children}
		</div>
	);
}

/**
 * Le maillage hexagonal du fond.
 *
 * Dessine en SVG plutot qu'en image : il se teinte avec les variables, reste net a toute
 * densite d'ecran, et ne coute aucune requete. `aria-hidden` parce qu'un decor n'a rien a dire
 * a un lecteur d'ecran.
 */
export function HexBackdrop({ opacite = 0.15 }: { opacite?: number }) {
	return (
		<svg
			aria-hidden="true"
			style={{ position: "absolute", inset: 0, width: "100%", height: "100%", opacity: opacite, pointerEvents: "none" }}
		>
			<defs>
				<pattern id="inacord-hex" width="28" height="24" patternUnits="userSpaceOnUse">
					{/* Un hexagone pointe en haut, repete en quinconce par le pas du motif. */}
					<path
						d="M14 1 L26 7 L26 17 L14 23 L2 17 L2 7 Z"
						fill="none"
						stroke="var(--inacord-accent)"
						strokeWidth="1"
					/>
				</pattern>
			</defs>
			<rect width="100%" height="100%" fill="url(#inacord-hex)" />
		</svg>
	);
}

/** Une entree de la liste des salons. */
export interface Salon {
	id: string;
	nom: string;
	apercu?: string;
	nonLus?: number;
}

/**
 * La liste des salons.
 *
 * Rendue en `<ul>` de `<button>` : la navigation au clavier et l'annonce « liste de N elements »
 * viennent alors du navigateur, sans qu'on ait a les reimplementer.
 */
export function RoomList({
	salons,
	actif,
	onChoisir,
}: {
	salons: Salon[];
	actif?: string;
	onChoisir?: (id: string) => void;
}) {
	return (
		<ul style={{ listStyle: "none", margin: 0, padding: 0, overflowY: "auto", flex: 1 }}>
			{salons.map((s) => (
				<li key={s.id}>
					<button
						type="button"
						onClick={onChoisir ? () => onChoisir(s.id) : undefined}
						aria-current={s.id === actif ? "true" : undefined}
						style={{
							display: "flex",
							alignItems: "center",
							gap: "var(--jeu-espace-s)",
							width: "100%",
							padding: "var(--jeu-espace-s) var(--jeu-espace-m)",
							border: "none",
							borderLeft:
								s.id === actif ? "3px solid var(--inacord-accent)" : "3px solid transparent",
							background: s.id === actif ? "var(--inacord-panneau-clair)" : "transparent",
							color: "inherit",
							font: "inherit",
							textAlign: "left",
							cursor: onChoisir ? "pointer" : "default",
							transition: "background var(--jeu-duree-rapide) var(--jeu-courbe)",
						}}
					>
						<span style={{ flex: 1, minWidth: 0 }}>
							<span style={{ display: "block", fontWeight: 600 }}>{s.nom}</span>
							{s.apercu ? (
								<span
									style={{
										display: "block",
										fontSize: "0.8rem",
										color: "var(--jeu-surface-cendre)",
										overflow: "hidden",
										textOverflow: "ellipsis",
										whiteSpace: "nowrap",
									}}
								>
									{s.apercu}
								</span>
							) : null}
						</span>
						{s.nonLus ? (
							<span
								aria-label={`${s.nonLus} non lus`}
								style={{
									minWidth: "1.5em",
									padding: "0 6px",
									borderRadius: "999px",
									background: "var(--inacord-accent)",
									color: "var(--jeu-fond-abysse)",
									fontSize: "0.75rem",
									fontWeight: 800,
									textAlign: "center",
								}}
							>
								{s.nonLus}
							</span>
						) : null}
					</button>
				</li>
			))}
		</ul>
	);
}

/** Un message du fil. */
export interface Message {
	id: string;
	auteur: string;
	corps: ReactNode;
	deMoi?: boolean;
	horodatage?: string;
}

/**
 * Le fil de discussion.
 *
 * `aria-live="polite"` annonce les messages qui arrivent sans couper la parole, ce qui est le
 * comportement attendu d'une messagerie — `assertive` interromprait a chaque ligne.
 */
export function MessageThread({ messages }: { messages: Message[] }) {
	return (
		<div
			aria-live="polite"
			style={{
				display: "flex",
				flexDirection: "column",
				gap: "var(--jeu-espace-s)",
				padding: "var(--jeu-espace-m)",
				overflowY: "auto",
				flex: 1,
			}}
		>
			{messages.map((m) => (
				<article
					key={m.id}
					style={{
						alignSelf: m.deMoi ? "flex-end" : "flex-start",
						maxWidth: "78%",
						padding: "var(--jeu-espace-s) var(--jeu-espace-m)",
						borderRadius: "var(--jeu-espace-m)",
						borderBottomRightRadius: m.deMoi ? "var(--jeu-rayon)" : undefined,
						borderBottomLeftRadius: m.deMoi ? undefined : "var(--jeu-rayon)",
						background: m.deMoi ? "var(--inacord-accent)" : "var(--inacord-panneau-clair)",
						color: m.deMoi ? "var(--jeu-fond-abysse)" : "var(--jeu-texte-vif)",
					}}
				>
					{m.deMoi ? null : (
						<div style={{ fontSize: "0.75rem", fontWeight: 700, opacity: 0.8 }}>{m.auteur}</div>
					)}
					<div>{m.corps}</div>
					{m.horodatage ? (
						<time style={{ display: "block", fontSize: "0.7rem", opacity: 0.7, textAlign: "right" }}>
							{m.horodatage}
						</time>
					) : null}
				</article>
			))}
		</div>
	);
}

/** Un onglet de la barre du bas. */
export interface Onglet {
	id: string;
	libelle: string;
	icone?: ReactNode;
}

/**
 * La barre d'onglets du bas.
 *
 * `role="tablist"` et `aria-selected` donnent au clavier et aux lecteurs d'ecran la semantique
 * d'onglets. Le libelle reste VISIBLE sous l'icone : une barre qui n'affiche que des
 * pictogrammes oblige a deviner, et se traduit mal.
 */
export function TabBar({
	onglets,
	actif,
	onChoisir,
}: {
	onglets: Onglet[];
	actif?: string;
	onChoisir?: (id: string) => void;
}) {
	return (
		<nav
			role="tablist"
			style={{
				display: "flex",
				borderTop: "1px solid rgb(15 16 17 / 60%)",
				background: "var(--inacord-panneau)",
			}}
		>
			{onglets.map((o) => {
				const choisi = o.id === actif;
				return (
					<button
						key={o.id}
						type="button"
						role="tab"
						aria-selected={choisi}
						onClick={onChoisir ? () => onChoisir(o.id) : undefined}
						style={{
							flex: 1,
							display: "flex",
							flexDirection: "column",
							alignItems: "center",
							gap: 2,
							padding: "var(--jeu-espace-s) 0",
							border: "none",
							borderTop: choisi ? "2px solid var(--inacord-accent)" : "2px solid transparent",
							background: "transparent",
							color: choisi ? "var(--inacord-accent)" : "var(--jeu-surface-cendre)",
							font: "inherit",
							fontSize: "0.75rem",
							cursor: onChoisir ? "pointer" : "default",
						}}
					>
						{o.icone}
						<span>{o.libelle}</span>
					</button>
				);
			})}
		</nav>
	);
}
