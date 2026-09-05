"use client";

import { Link } from "@/components/ui/link";
import { CommonSpriteIcon } from "@/components/ui/CommonSpriteIcon";
import type { SpriteCommonKey } from "@/config/sprites-common";
import { cn } from "@/lib/utils";

// ── Types nie-data (passives-full.json) ──

/** Une instance de passif joueur issue de passives-full.json (nie-data). */
export interface NierPassiveInstance {
	passive_id: string;
	string_id: string;
	effect_id: string;
	rarity: number;
	element: number;
	element_name: string;
	buff_icon_type: number | null;
	effect_params: number[];
	main_value: number | null;
	name: { fr: string | null; en: string | null; ja: string | null };
	description: { fr: string | null; en: string | null; ja: string | null };
	text_raw: { fr: string | null; en: string | null; ja: string | null };
}

/** Une famille de passifs (toutes les instances partageant le même effect_id). */
export interface NierPassiveFamily {
	effect_id: string;
	instances: NierPassiveInstance[];
}

/** Un passif d'équipe (team passive). */
export interface NierTeamPassive {
	team_passive_id: string;
	effect_id: string;
	value_min: number;
	value_max: number;
	text_id: string;
	text: { fr: string | null; en: string | null; ja: string | null };
}

// ── Types ──

export interface RarityValues {
	low: string;
	high: string;
}

export interface PlayerPassiveData {
	no: number;
	playstyle: string;
	requirement: string;
	stat: string;
	legendary: RarityValues;
	top: RarityValues;
	advanced: RarityValues;
	growing: RarityValues;
	common: RarityValues;
}

export interface CustomPassiveData {
	no: number;
	requirement: string;
	stat: string;
	buff: string;
}

export interface CoordinatorPassiveData {
	no: number;
	playstyle: string;
	requirement: string;
	stat: string;
	coordinatorCommon: string;
	coordinatorLegendary: string;
	managerCommon: string;
	managerLegendary: string;
}

// ── Translations ──

export const REQUIREMENT_FR: Record<string, string> = {
	"After a substitution, for the player who comes on (15s)":
		"Après remplacement, joueur entrant (15s)",
	"For nearby players": "Joueurs à proximité",
	"For players of different elements": "Joueurs d'éléments différents",
	"For players of different positions": "Joueurs de positions différentes",
	"For players of the same element": "Joueurs du même élément",
	"For players of the same position": "Joueurs de la même position",
	"For players of the same positions": "Joueurs de la même position",
	"For the first half": "En première mi-temps",
	"For the second half": "En seconde mi-temps",
	"Non-Conditional": "Sans condition",
	"On Dash knockback": "Lors d'un plaquage en sprint",
	"On gaining possession (excluding catches) (15s)": "Récupération du ballon (hors arrêts) (15s)",
	"On gaining possession(excluding catches) (15s)": "Récupération du ballon (hors arrêts) (15s)",
	"On the opposition's half of the pitch": "Sur la moitié de terrain adverse",
	"On your half of the pitch": "Sur votre moitié de terrain",
	"Until you incur a foul": "Tant que vous ne commettez pas de faute",
	"Upon being subbed in (15s)": "Après remplacement (15s)",
	"When a player of a different element is nearby": "Joueur d'élément différent à proximité",
	"When a player of the same element is nearby": "Joueur du même élément à proximité",
	"When at +15% or higher Breach Rate": "Taux de brèche +15% ou plus",
	"When at 100% Tension": "Tension à 100%",
	"When at 15% of higher Breach Rate": "Taux de brèche +15% ou plus",
	"When at 20% of more Bond Power": "Puissance de lien à 20% ou plus",
	"When at 20% or more Bond Power": "Puissance de lien à 20% ou plus",
	"When at 50% of higher Tension": "Tension à 50% ou plus",
	"When at 50% or higher tension": "Tension à 50% ou plus",
	"When losing a Scramble": "En perdant une mêlée",
	"When making a pass": "En faisant une passe",
	"When opposition passes the ball out during Focus Battle":
		"L'adversaire passe le ballon lors d'un affrontement",
	"When outside the zone": "Hors de la zone",
	"When the opposition commits a foul": "Quand l'adversaire commet une faute",
	"When tied or behind in goals": "Score à égalité ou en retard",
	"When winning a Focus or Scramble Battle": "En remportant un affrontement ou une mêlée",
};

export const STAT_FR: Record<string, string> = {
	"Castle Wall DF": "DEF Mur de défense",
	"Drain Tension": "Drain de tension",
	"Focus AT & DF": "ATT & DEF Affrontement",
	"Own Castle Wall": "Mur de défense (perso)",
	"Own Castle Wall DF": "DEF Mur de défense (perso)",
	"Own Focus AT & DF": "ATT & DEF Affrontement (perso)",
	"Own Scramble AT & DF": "ATT & DEF Mêlée (perso)",
	"Own Shot AT": "ATT Tir (perso)",
	"Own Special Tactics Cooldown": "Cooldown Tactiques spé. (perso)",
	"Scramble AT & DF": "ATT & DEF Mêlée",
	"Shot AT": "ATT Tir",
	"Team AT": "ATT (équipe)",
	"Team AT & DF": "ATT & DEF (équipe)",
	"Team Bond Power": "Puissance de lien (équipe)",
	"Team Bond Power Loss": "Perte puissance de lien (équipe)",
	"Team Breach Rate": "Taux de brèche (équipe)",
	"Team Castle Wall DF": "DEF Mur de défense (équipe)",
	"Team Castle Wall Pierce Rate": "Taux de percée (équipe)",
	"Team DF": "DEF (équipe)",
	"Team Dash Foul Rate": "Taux faute sprint (équipe)",
	"Team Direct Shot AT": "ATT Tir direct (équipe)",
	"Team Focus AT & DF": "ATT & DEF Affrontement (équipe)",
	"Team Foul Rate": "Taux de faute (équipe)",
	"Team Rough Attack AT & DF": "ATT & DEF Assaut (équipe)",
	"Team Scramble AT & DF": "ATT & DEF Mêlée (équipe)",
	"Team Shot AT": "ATT Tir (équipe)",
	"Team Tension": "Tension (équipe)",
	"Team Tension Breach Cost": "Coût tension brèche (équipe)",
	"[DF] Focus AT & DF": "[DF] ATT & DEF Affrontement",
	"[KP] KP": "[GK] Gardien",
	"[MF] Focus AT & DF": "[MF] ATT & DEF Affrontement",
	"[Substitute Player] AT": "[Remplaçant] ATT",
	"[Substitute Player] DF": "[Remplaçant] DEF",
};

export const PLAYSTYLE_FR: Record<string, string> = {
	Bond: "Lien",
	Breach: "Brèche",
	Counter: "Contre-attaque",
	Justice: "Justice",
	"Rough Play": "Jeu brutal",
	Tension: "Tension",
};

const PLAYSTYLE_SPRITES: Record<string, SpriteCommonKey> = {
	Bond: "lien",
	Breach: "breche",
	Counter: "contre",
	Justice: "justice",
	"Rough Play": "jeu_violent",
	Tension: "tension",
};

const PLAYSTYLE_COLORS: Record<string, string> = {
	Bond: "text-pink-500",
	Breach: "text-red-500",
	Counter: "text-blue-500",
	Justice: "text-emerald-500",
	"Rough Play": "text-orange-500",
	Tension: "text-amber-500",
};

const RARITY_COLORS: Record<string, string> = {
	advanced: "bg-blue-500/20 text-blue-600",
	common: "bg-slate-500/20 text-slate-600",
	growing: "bg-green-500/20 text-green-600",
	legendary: "bg-amber-500/20 text-amber-600",
	top: "bg-purple-500/20 text-purple-600",
};

const RARITY_LABELS: Record<string, string> = {
	advanced: "Expérimenté",
	common: "Normal",
	growing: "En progression",
	legendary: "Normal",
	top: "Émérite",
};

// ── Player Passive Card ──

export function PlayerPassiveCard({ passive }: { passive: PlayerPassiveData }) {
	const reqFr = REQUIREMENT_FR[passive.requirement] || passive.requirement;
	const statFr = STAT_FR[passive.stat] || passive.stat;
	const playstyleFr = PLAYSTYLE_FR[passive.playstyle] || null;
	const playstyleSprite = PLAYSTYLE_SPRITES[passive.playstyle] || null;
	const playstyleColor = PLAYSTYLE_COLORS[passive.playstyle] || "text-on-surface-variant";

	const rarities = [
		{ key: "legendary", values: passive.legendary },
		{ key: "top", values: passive.top },
		{ key: "advanced", values: passive.advanced },
		{ key: "growing", values: passive.growing },
		{ key: "common", values: passive.common },
	];

	return (
		<div className="rounded-2xl border border-outline-variant/30 bg-surface-container-low hover:bg-surface-container transition-colors p-4 space-y-3">
			{/* Header: stat + playstyle */}
			<div className="flex items-start justify-between gap-2">
				<div>
					<h3 className="font-bold text-sm text-on-surface leading-tight">{statFr}</h3>
					<p className="text-xs text-on-surface-variant mt-0.5">{reqFr}</p>
				</div>
				{playstyleFr && (
					<span
						className={cn(
							"inline-flex items-center gap-1 px-2 py-1 rounded-full text-[10px] font-bold uppercase tracking-wider bg-surface-container-highest",
							playstyleColor
						)}
					>
						{playstyleSprite && <CommonSpriteIcon name={playstyleSprite} scale={0.2} />}
						{playstyleFr}
					</span>
				)}
			</div>

			{/* Rarity values grid */}
			<div className="space-y-1">
				{rarities.map(({ key, values }) => {
					if (!values.low && !values.high) {
						return null;
					}
					const display =
						values.low === values.high ? values.low : `${values.low} ~ ${values.high}`;
					return (
						<div key={key} className="flex items-center gap-2">
							<span
								className={cn(
									"text-[10px] font-bold uppercase tracking-wider w-20 shrink-0 px-1.5 py-0.5 rounded text-center",
									RARITY_COLORS[key]
								)}
							>
								{RARITY_LABELS[key]}
							</span>
							<span className="text-xs font-mono font-bold text-on-surface flex-1">{display}</span>
						</div>
					);
				})}
			</div>
		</div>
	);
}

// ── Custom Passive Card ──

export function CustomPassiveCard({ passive }: { passive: CustomPassiveData }) {
	const reqFr = REQUIREMENT_FR[passive.requirement] || passive.requirement;
	const statFr = STAT_FR[passive.stat] || passive.stat;

	return (
		<div className="rounded-2xl border border-outline-variant/30 bg-surface-container-low hover:bg-surface-container transition-colors p-4 space-y-2">
			<div>
				<h3 className="font-bold text-sm text-on-surface leading-tight">{statFr}</h3>
				<p className="text-xs text-on-surface-variant mt-0.5">{reqFr}</p>
			</div>
			<div className="flex items-center gap-2">
				<span className="text-[10px] font-bold uppercase tracking-wider px-1.5 py-0.5 rounded bg-tertiary/20 text-tertiary">
					Bonus
				</span>
				<span className="text-sm font-mono font-bold text-on-surface">{passive.buff}</span>
			</div>
		</div>
	);
}

// ── Nie-data Passive Family Card ──

const ELEMENT_COLORS: Record<string, string> = {
	// Tokens `element-*` de `@rosegriffon/ui` : les teintes du JEU, relevées sur
	// ses icônes officielles. Les classes Tailwind en dur d'avant divergeaient
	// d'un écran à l'autre — le vent y était bleu ici, vert ailleurs.
	fire: "bg-element-feu/15 text-element-feu border-element-feu/40",
	forest: "bg-element-foret/15 text-element-foret border-element-foret/40",
	mountain: "bg-element-montagne/15 text-element-montagne border-element-montagne/40",
	neutral: "bg-outline-variant/20 text-on-surface-variant border-outline-variant/40",
	wind: "bg-element-vent/15 text-element-vent border-element-vent/40",
};

const ELEMENT_LABELS: Record<string, string> = {
	fire: "Feu",
	forest: "Forêt",
	mountain: "Montagne",
	neutral: "Neutre",
	wind: "Vent",
};

const STRING_ID_PREFIX_LABEL: Record<string, string> = {
	bcps: "Cust. équipe",
	bmps: "Mixi équipe",
	cps: "Personnalisé",
	hps: "Héros",
	mps: "Miximax",
	ps: "Joueur",
	ss: "Soul",
	swap: "Swap",
};

function instancePrefix(stringId: string): string {
	const m = stringId.match(/^([a-z_]+)/);
	const prefix = m ? m[1].replace(/_$/, "") : "";
	return STRING_ID_PREFIX_LABEL[prefix] ?? prefix;
}

/**
 * Carte affichant une famille de passifs nie-data (128 familles, ~1-32 instances chacune).
 * Affiche le texte FR résolu de la première instance + toutes les valeurs.
 */
export function NierPassiveFamilyCard({ family }: { family: NierPassiveFamily }) {
	const { instances } = family;
	if (instances.length === 0) return null;

	const representative = instances[0];
	const textFr = representative.description.fr ?? representative.description.en ?? representative.description.ja ?? "";
	const elementName = representative.element_name;
	const elementColor = ELEMENT_COLORS[elementName] ?? ELEMENT_COLORS.neutral;
	const elementLabel = ELEMENT_LABELS[elementName] ?? elementName;

	// Grouper les instances par préfixe (ps, mps, cps, hps, ss, swap…)
	const byPrefix = new Map<string, NierPassiveInstance[]>();
	for (const inst of instances) {
		const prefix = instancePrefix(inst.string_id);
		if (!byPrefix.has(prefix)) byPrefix.set(prefix, []);
		byPrefix.get(prefix)!.push(inst);
	}

	return (
		<div className="rounded-2xl border border-outline-variant/30 bg-surface-container-low hover:bg-surface-container transition-colors p-4 space-y-3">
			{/* Description FR résolue (lien vers la fiche détail) */}
			<div className="flex items-start justify-between gap-2">
				<Link
					href={`/passive/${representative.passive_id}`}
					className="text-sm font-semibold text-on-surface leading-snug whitespace-pre-line flex-1 hover:text-primary transition-colors"
				>
					{textFr}
				</Link>
				{elementName !== "neutral" && (
					<span
						className={cn(
							"inline-flex items-center shrink-0 px-2 py-0.5 rounded-full text-[10px] font-bold uppercase tracking-wider border",
							elementColor
						)}
					>
						{elementLabel}
					</span>
				)}
			</div>

			{/* Tableau des valeurs par type de passif (chaque valeur → fiche détail) */}
			<div className="space-y-2">
				{[...byPrefix.entries()].map(([prefix, insts]) => (
					<div key={prefix}>
						<div className="text-[10px] font-bold uppercase tracking-wider text-on-surface-variant mb-1">
							{prefix}
						</div>
						<div className="flex flex-wrap gap-1">
							{insts.map((inst) => (
								<Link
									key={inst.string_id}
									href={`/passive/${inst.passive_id}`}
									className="inline-flex items-center gap-1 px-2 py-0.5 rounded bg-surface-container-highest text-xs font-mono text-on-surface hover:bg-primary/15 hover:text-primary transition-colors"
									title={inst.string_id}
								>
									{inst.main_value !== null ? `+${inst.main_value}%` : inst.string_id}
								</Link>
							))}
						</div>
					</div>
				))}
			</div>

			{/* string_id de référence */}
			<div className="text-[10px] text-on-surface-variant/50 font-mono truncate">
				{representative.string_id}
			</div>
		</div>
	);
}

// ── Nie-data Team Passive Card ──

export function NierTeamPassiveCard({ passive }: { passive: NierTeamPassive }) {
	const text = passive.text.ja ?? passive.text.en ?? passive.text.fr ?? "";

	return (
		<div className="rounded-2xl border border-outline-variant/30 bg-surface-container-low hover:bg-surface-container transition-colors p-4 space-y-2">
			<p className="text-sm font-semibold text-on-surface leading-snug">{text}</p>
			<div className="flex items-center gap-2">
				<span className="text-[10px] font-bold uppercase tracking-wider px-1.5 py-0.5 rounded bg-blue-500/15 text-blue-600">
					Équipe
				</span>
				<span className="text-xs font-mono text-on-surface-variant">
					{passive.value_min} – {passive.value_max}
				</span>
			</div>
		</div>
	);
}

// ── Coordinator Passive Card ──

export function CoordinatorPassiveCard({ passive }: { passive: CoordinatorPassiveData }) {
	const reqFr = REQUIREMENT_FR[passive.requirement] || passive.requirement;
	const statFr = STAT_FR[passive.stat] || passive.stat;
	const playstyleFr = PLAYSTYLE_FR[passive.playstyle] || null;
	const playstyleColor = PLAYSTYLE_COLORS[passive.playstyle] || "text-on-surface-variant";
	const playstyleSprite = PLAYSTYLE_SPRITES[passive.playstyle] || null;

	const roles = [
		{
			color: "bg-amber-500/20 text-amber-600",
			label: "Manager (L)",
			value: passive.managerLegendary,
		},
		{ color: "bg-slate-500/20 text-slate-600", label: "Manager (C)", value: passive.managerCommon },
		{
			color: "bg-purple-500/20 text-purple-600",
			label: "Coord. (L)",
			value: passive.coordinatorLegendary,
		},
		{
			color: "bg-slate-500/20 text-slate-600",
			label: "Coord. (C)",
			value: passive.coordinatorCommon,
		},
	].filter((r) => r.value);

	return (
		<div className="rounded-2xl border border-outline-variant/30 bg-surface-container-low hover:bg-surface-container transition-colors p-4 space-y-3">
			<div className="flex items-start justify-between gap-2">
				<div>
					<h3 className="font-bold text-sm text-on-surface leading-tight">{statFr}</h3>
					<p className="text-xs text-on-surface-variant mt-0.5">{reqFr}</p>
				</div>
				{playstyleFr && (
					<span
						className={cn(
							"inline-flex items-center gap-1 px-2 py-1 rounded-full text-[10px] font-bold uppercase tracking-wider bg-surface-container-highest",
							playstyleColor
						)}
					>
						{playstyleSprite && <CommonSpriteIcon name={playstyleSprite} scale={0.2} />}
						{playstyleFr}
					</span>
				)}
			</div>
			<div className="space-y-1">
				{roles.map(({ label, value, color }) => (
					<div key={label} className="flex items-center gap-2">
						<span
							className={cn(
								"text-[10px] font-bold uppercase tracking-wider w-20 shrink-0 px-1.5 py-0.5 rounded text-center",
								color
							)}
						>
							{label}
						</span>
						<span className="text-xs font-mono font-bold text-on-surface">{value}</span>
					</div>
				))}
			</div>
		</div>
	);
}
