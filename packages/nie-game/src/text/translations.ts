/** Shared translations for shops and tactics */

export const SHOP_FR: Record<string, string> = {
	Chronicle: "Galerie Chronique",
	"Chronicle Department Store": "Galerie Chronique",
	"G-Mart (Arcade Branch)": "G-Mart (Succursale Arcade)",
	"G-Mart (Odaiba)": "G-Mart (Odaiba)",
	"Kool Kit (Odaiba Branch)": "Kool Kit (Succursale Odaiba)",
	"Legendary Chest": "Coffre légendaire",
	"Magic Moves (Odaiba Branch)": "Super Techniques (Succursale Odaiba)",
	"Special Training Booth": "Stand d'entraînement spécial",
	Spirit: "Marché des Esprits",
	"Spirit Market": "Marché des Esprits",
	VS: "Boutique VS",
	"Vs Store": "Boutique VS",
};

/** Tactic names EN→FR — extracted from game text files */
export const TACTIC_FR: Record<string, string> = {
	"3D Reflector": "Réflecteur 3D",
	"Absolute Barrier": "Barrière absolue",
	"Absolute Knights": "Chevaliers absolus",
	"Against All Odds": "Contre toute attente",
	"Amazon Wave": "Vague amazonienne",
	"Andres Antlion": "Piège de sable des Andes",
	"Bond Protocol": "Protocole kizuna",
	"Boost 15": "Boost 15",
	"Box Lock": "Bloc défensif",
	"Bull Horns": "Cornes de taureau",
	"Catenaccio Counter": "Contre-attaque catenaccio",
	"Caucus Race": "Course au caucus",
	"Cavalry Advance": "L'avance de la cavalerie",
	"Circular Drive": "Impulsion circulaire",
	Claymore: "Claymore",
	"Dark Thunder": "Tonnerre obscur",
	"Diamond Defence": "Défense de diamant",
	"Dual Typhoon": "Typhons jumeaux",
	"Enchanted Forest": "Forêt enchantée",
	"Flame Fortress": "Forteresse de flammes",
	"Ghost Lock": "Blocage spectral",
	"Grand Plan": "Plan parfait",
	"Grid Guard": "Attaque cadrillée",
	"Invincible Lance": "Lance invincible",
	"Invisible Link": "Lien invisible",
	"Keyman Lockdown": "Enfermement du joueur-clé",
	"Mark of the Berserker": "L'Empreinte du Fauve",
	"Mount Fuji": "Mont Fuji",
	"Omega Maelstrom": "Maelstrom oméga",
	"Perfect Zone Press": "Zone de pressing parfait",
	"Puppet Master": "Mystification",
	"Roar of Resolve": "Rugissement déterminé",
	"Rolling Thunder": "Roulement de tonnerre",
	"Royal Runaround": "Relais royal",
	"Sandstorm Skid": "Glisse sur le sable",
	"Shinano Formation": "Formation Shinano",
	"Sideline Spears": "Lances latérales",
	"Sky's the Limit": "Route du ciel",
	"Stalwart Suppression": "Capture de l'éclair",
	"Star Path": "Chemin étoilé",
	"Suction Vortex": "Maelström",
	"Swiord Superiority": "Le Glaive",
	"Three-Pronged Attack": "Attaque sur trois fronts",
	Thunderbolt: "Tonnerre ultime",
	"Titan Wall": "Muraille titanesque",
	"Tortoiseshell Pattern": "Formation carapace",
	"Track Cycling": "Attaque cyclique",
	"Trinity Force": "Triangle parfait",
	"Twin Wings": "Ailes jumelles",
	"Upsy-Daisy": "Passes volantes",
	Virtuoso: "Tacticien céleste",
	"Virtuoso Vulcano": "Tacticien céleste volcanique",
	"Waxing Moon": "Lune croissante",
	"Zoned Out": "Défense hermétique",
};

/** All 74 tactic effects from DB — exact match EN→FR */
export const EFFECT_FR: Record<string, string> = {
	// --- Stats globales ---
	"AT +15%": "ATT +15%",
	"Shot AT +10%": "Tir ATT +10%",
	"Move Speed +10%": "Vitesse +10%",
	"Enemy Move Speed -10%": "Vitesse adverse -10%",
	"Enemy Team DF -10%": "DEF adverse -10%",
	"Enemy Team DF -15%": "DEF adverse -15%",
	"Special Move Power +15%": "Puissance technique +15%",
	"Tension depletion -25%": "Perte de tension -25%",
	"Foul Rate -5%": "Taux de faute -5%",
	"Breach rate +5%": "Taux de percée +5%",
	"Save rate +5%": "Taux d'arrêt +5%",

	// --- Techniques offensives ---
	"Shot Move Power +10%": "Puissance de tir +10%",
	"Shot Move Power +20%": "Puissance de tir +20%",
	"Dribble Move Power +20%": "Puissance de dribble +20%",
	"Combination Move Power +20%": "Puissance de combinaison +20%",
	"Enemy Shot Move Power -20%": "Puissance de tir adverse -20%",
	"Enemy Dribble Move Power -20%": "Puissance de dribble adverse -20%",

	// --- Focus / Scramble ---
	"Focus AT/DF +25%": "Concentration ATT/DEF +25%",
	"Scramble AT/DF +25%": "Mêlée ATT/DEF +25%",
	"Decrease the Focus AT & DF of your Focus Battle opponent to match your own team's":
		"Réduit la Concentration ATT/DEF adverse au niveau de votre équipe",
	"When passing, Bond Power +100%": "Lors d'une passe, Puissance de lien +100%",
	"Foul rate when dashing -50%": "Taux de faute en sprint -50%",

	// --- Overall ---
	"Overall: Shot AT +10%": "Global : Tir ATT +10%",
	"Overall: DF +10%": "Global : DEF +10%",
	"Overall: KP +10%": "Global : PG +10%",
	"Overall: Move Speed +10%": "Global : Vitesse +10%",
	"Overall: Enemy Move Speed -10%": "Global : Vitesse adverse -10%",
	"Overall: Focus AT/DF +20%": "Global : Concentration ATT/DEF +20%",
	"Overall: Focus, Scramble AT/DF +10%": "Global : Concentration, Mêlée ATT/DEF +10%",
	"Overall: Scramble AT/DF +25%": "Global : Mêlée ATT/DEF +25%",
	"Overall: Rough Attack AT/DF +25%": "Global : Charge ATT/DEF +25%",
	"Overall: Bond Power Loss -100%": "Global : Perte de lien -100%",
	"Overall: Tension acquisition +25%": "Global : Gain de tension +25%",
	"Overall: Foul rate +25%": "Global : Taux de faute +25%",
	"Overall: Foul rate when dashing -10%": "Global : Taux de faute en sprint -10%",
	"Overall: When passing, Bond Power+100%": "Global : Lors d'une passe, Puissance de lien +100%",
	"Overall: When team is scattered, Bond Power gain increases by 25%":
		"Global : Équipe dispersée, gain de lien +25%",

	// --- Géoglyphes ---
	"Geoglyph: Stun opponent with the ball": "Géoglyphe : Étourdit le porteur du ballon adverse",
	"Geoglyph: Opponent loses ball": "Géoglyphe : L'adversaire perd le ballon",
	"Geoglyph: Dash speed +50%": "Géoglyphe : Vitesse de sprint +50%",
	"Geoglyph: Dash Stamina Cost -100%": "Géoglyphe : Coût d'endurance du sprint -100%",
	"Geoglyph: DF +10%": "Géoglyphe : DEF +10%",
	"Geoglyph: DF +40%": "Géoglyphe : DEF +40%",
	"Geoglyph: DF+40%": "Géoglyphe : DEF +40%",
	"Geoglyph: DF +60%": "Géoglyphe : DEF +60%",
	"Geoglyph: Focus AT/DF +40%": "Géoglyphe : Concentration ATT/DEF +40%",
	"Geoglyph: Own Special Move Cooldown -25%": "Géoglyphe : Recharge technique -25%",
	"Geoglyph: When passing, Bond Power +200%":
		"Géoglyphe : Lors d'une passe, Puissance de lien +200%",
	"Geoglyph: Foul rate when dashing -50%": "Géoglyphe : Taux de faute en sprint -50%",
	"Geoglyph: Tension depletion -25%": "Géoglyphe : Perte de tension -25%",
	"Geoglyph: When Tension is below 30%, Focus AT/DF +40%":
		"Géoglyphe : Tension < 30%, Concentration ATT/DEF +40%",
	"Geoglyph: When Tension is below 30%, Scramble AT/DF +40%":
		"Géoglyphe : Tension < 30%, Mêlée ATT/DEF +40%",
	// Typo variants from DB
	"Geogylph: AT +15%": "Géoglyphe : ATT +15%",
	"Geogylph: DF +30%": "Géoglyphe : DEF +30%",
	"Geogylph: Dribble speed +50%": "Géoglyphe : Vitesse de dribble +50%",

	// --- Positionnels ---
	"On opposition's half of pitch, Opponent DF -30%": "Camp adverse : DEF adverse -30%",
	"On opposition's half of the pitch, Team AT +30%": "Camp adverse : ATT équipe +30%",
	"On your half of the pitch, Team DF +30%": "Votre camp : DEF équipe +30%",
	"On your half of the pitch, Opponent AT -30%": "Votre camp : ATT adverse -30%",

	// --- Ciblage ---
	"Target: AT -50%": "Cible : ATT -50%",
	"Target: DF -50%": "Cible : DEF -50%",
	"Target: DF - 50%": "Cible : DEF -50%",
	"Target: AT DF -50%": "Cible : ATT DEF -50%",
	"Target: Move Speed -10%": "Cible : Vitesse -10%",
	"Target: Pass lines to target disappear": "Cible : Lignes de passe vers la cible disparaissent",

	// --- Effets spéciaux ---
	"Disable opponents near ball": "Neutralise les adversaires proches du ballon",
	"No pass intercepts": "Aucune interception de passe",
	"No pass intercepts & skip Focus Battles": "Aucune interception ni duel de concentration",
	"Deactivate opponent's totems/souls": "Désactive les totems adverses",
	"Divide Enemy players into left and right": "Sépare les adversaires à gauche et à droite",
	"Form up, ignore Focus, and push forward": "Formation serrée, ignore la concentration, avance",
	"Opposition more likely to miss passes": "L'adversaire rate plus souvent ses passes",
	"Temporarily disables enemy Dash": "Désactive temporairement le sprint adverse",
	"Temporarily disables enemy Focus Fields":
		"Désactive temporairement les zones de concentration adverses",
};

/** Translate an effect string — exact match from EFFECT_FR */
export function translateEffect(effect: string): string {
	if (!effect) {
		return effect;
	}
	return EFFECT_FR[effect] || effect;
}

/** Slugify a tactic name for URL usage */
export function tacticSlug(name: string): string {
	return name
		.toLowerCase()
		.replaceAll(/['']/g, "")
		.replaceAll(/[^a-z0-9]+/g, "-")
		.replaceAll(/^-+|-+$/g, "");
}
