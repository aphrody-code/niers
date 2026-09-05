// Deplace dans @niers/game (paquet neutre, 2026-09-05) — re-export pour ne rien casser.
// Seul l'atlas change d'hote : le wiki Azalee le sert depuis le CDN Rose Griffon.
export * from "@niers/game/text/gaiji";
import { GAIJI_ATLAS as ATLAS_NEUTRE } from "@niers/game/text/gaiji";

/** Atlas décodé (PNG) servi par le CDN Rose Griffon + ses dimensions natives. */
export const GAIJI_ATLAS = {
	...ATLAS_NEUTRE,
	url: "https://cdn.rosegriffon.fr/dx11/font/fr/gaiji_game2.png",
} as const;
