/** Translate passive effect strings from EN to FR programmatically */
export function translatePassiveEffect(text: string): string {
	let s = text;
	// Conditions
	s = s.replace(/^For nearby players, /, "Proximité : ");
	s = s.replace(/^For players? of the same positions?, /, "Même poste : ");
	s = s.replace(/^For player of the same position, /, "Même poste : ");
	s = s.replace(/^For players of different positions, /, "Postes différents : ");
	s = s.replace(/^For players of the same element, /, "Même élément : ");
	s = s.replace(/^For players of different elements, /, "Éléments différents : ");
	s = s.replace(/^On the opposition'?s? half of the pitch, /, "Camp adverse : ");
	s = s.replace(/^On your half of the pitch, /, "Camp allié : ");
	s = s.replace(/^When a player of the same element is nearby, /, "Même élément à proximité : ");
	s = s.replace(
		/^When a player of a different element is nearby, /,
		"Élément différent à proximité : "
	);
	s = s.replace(/^When making a pass, /, "Lors d'une passe : ");
	s = s.replace(/^When winning a Focus or Scramble Battle, /, "Victoire d'affrontement : ");
	s = s.replace(/^When outside the zone, /, "Hors zone : ");
	// Scope
	s = s.replaceAll(/\bOwn\b/g, "Personnel");
	s = s.replaceAll(/\bTeam\b/g, "Équipe");
	// Stats
	s = s.replaceAll(/\bFocus AT & DF\b/g, "Affrontement ATT & DEF");
	s = s.replaceAll(/\bScramble AT & DF\b/g, "Mêlée ATT & DEF");
	s = s.replaceAll(/\bShot AT\b/g, "Tir ATT");
	s = s.replaceAll(/\bCastle Wall DF\b/g, "Mur de défense DEF");
	s = s.replaceAll(/\bAT\/DF\b/g, "ATT/DEF");
	s = s.replaceAll(/\bBond Power\b/g, "Puissance de lien");
	s = s.replaceAll(/\bSave rate\b/g, "Taux d'arrêt");
	s = s.replaceAll(/\bBreach Rate\b/g, "Taux de brèche");
	s = s.replaceAll(/\bDash Foul Rate\b/g, "Taux de faute en course");
	s = s.replaceAll(/\bTension Breach Cost\b/g, "Coût tension brèche");
	s = s.replaceAll(/\bTension\b/g, "Tension");
	// Multiline
	s = s.replace(
		/Transforms into an ally player on the field.*/,
		"Se transforme en joueur allié sur le terrain"
	);
	return s;
}

export const HISSATSU_TYPE_FR: Record<string, string> = {
	Defense: "Défense",
	Dribble: "Dribble",
	Keep: "Arrêt",
	Offense: "Attaque",
	Shoot: "Tir",
};

export const HISSATSU_ELEMENT_FR: Record<string, string> = {
	Fire: "Feu",
	Forest: "Forêt",
	Mountain: "Montagne",
	Void: "Néant",
	Wind: "Vent",
	"山 (Mountain)": "Montagne",
	"林 (Forest)": "Forêt",
	"火 (Fire)": "Feu",
	"無 (Void)": "Néant",
	"風 (Wind)": "Vent",
};

export const BUFF_EFFECT_FR: Record<string, string> = {
	"Awakenings, Keshin, Totems and Armed last 30s, Miximax lasts 20s":
		"Éveils, Esprits Guerriers, Totems et Armures : 30s. Miximax : 20s",
	"Totems/Souls": "Totems",
};
