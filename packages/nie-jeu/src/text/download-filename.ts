/**
 * Nom de fichier de téléchargement lisible à partir d'un nom affiché (perso/technique/etc.) —
 * jamais le code interne du jeu (évite d'exposer `c01001900.glb`, `wks042.png`, …).
 */
export function downloadName(name: string, fallback = "fichier"): string {
	const slug = name
		.toLowerCase()
		.normalize("NFD")
		.replaceAll(/[\u0300-\u036f]/g, "")
		.replaceAll(/[^a-z0-9]+/g, "-")
		.replaceAll(/^-+|-+$/g, "");
	return slug || fallback;
}
