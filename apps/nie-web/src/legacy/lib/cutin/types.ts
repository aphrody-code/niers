/**
 * Modèle de données PUR du viewer de cut-in (fiche skill azalee).
 *
 * Zéro dépendance (ni three/@theatre, ni bun:sqlite, ni Node) → importable depuis
 * une page serveur comme depuis un îlot `"use client"`. Toutes les structures sont
 * sérialisables (le manifeste peut traverser la frontière server→client).
 *
 * Pipeline : `listing.ts` (réseau) produit un {@link CutinManifest} ; `decode.ts`
 * décode LAZY chaque {@link CutinLayerDescriptor} g4pk en {@link CutinGlbLayer}
 * (paire g4md+g4mg → GLB), le mesh waza principal devenant lui aussi une couche.
 */

/** Famille d'un sous-fichier g4pk (cf. nie-formats). `string` = magic non listé. */
export type G4Magic = "G4MD" | "G4MG" | "G4MT" | "G4MA" | "G4VS" | "G4SK" | string;

/**
 * Une couche = un g4pk d'effet de l'event. Décomposée du nom
 * `ev60_00340_<subject>_<phase>_p00_<plan>.g4pk`.
 */
export interface CutinLayerDescriptor {
	/** Clé stable = basename sans extension. */
	id: string;
	/** Chemin index complet (`data/common/event/.../*.g4pk`). */
	vfsPath: string;
	/** URL des octets bruts (`https://cdn.rosegriffon.fr/raw/<vfsPath>`). */
	rawUrl: string;
	/** Sujet de la couche : `"main"` | `"c000101"` | `"b000001"` | `"point_eff"` | … */
	subject: string;
	/** Phase : `"s00"` | `"s01"` | `"s80"` | `"s90"` | `null`. */
	phase: string | null;
	/** Plan : `"c0010"`..`"c0050"` | `null`. */
	plan: string | null;
	/** Décidé après décode : `true` si une paire g4md+g4mg a été trouvée. */
	renderable: boolean;
}

/** Manifeste de l'event, produit par `listing.ts` (réseau) — sérialisable. */
export interface CutinManifest {
	/** `event_id_name` (ex. `"ev60_00340"`). */
	eventId: string;
	/** `cutin.effects_dir`. */
	effectsDir: string;
	/** `/model-chr/waza/<eventId>.glb` (mesh principal, assemblé serveur). */
	modelGlbUrl: string;
	/** `/raw/<...>_camera.g4cm` si présent, sinon `null`. */
	cameraG4cmUrl: string | null;
	/** Couches d'effet g4pk de l'event. */
	layers: CutinLayerDescriptor[];
	/** Telops (noms rendus) par langue → URL PNG (CDN). */
	telops: { lang: string; pngUrl: string }[];
	/** `/raw/<sound_cfg>` (cfg de cues, audio best-effort), sinon `null`. */
	soundRawUrl: string | null;
}

/** Couche décodée et prête à monter dans la scène R3F. */
export interface CutinGlbLayer {
	id: string;
	/** URL.createObjectURL(Blob GLB) — à révoquer au démontage (no-op si réutilisée). */
	blobUrl: string;
	/** `false` = mesh waza principal, `true` = effet g4pk décodé. */
	fromEffect: boolean;
	/**
	 * `true` si `blobUrl` a été créé via `URL.createObjectURL` (donc à révoquer au
	 * démontage). `false` pour le mesh waza (URL CDN directe, révocation no-op).
	 */
	revocable: boolean;
}

/** État d'une couche pour les toggles UI. */
export interface CutinLayerToggle {
	id: string;
	/** `subject` + `plan` (ex. `"main"`, `"c000101 · c0010"`). */
	label: string;
	visible: boolean;
	/** `renderable` && décodé sans erreur (sinon toggle désactivé). */
	available: boolean;
}
