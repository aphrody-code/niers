/**
 * Construction du **manifeste** d'un cut-in d'event, côté client (réseau seulement).
 *
 * Mêmes données que la section cut-in existante (`SkillCutinSection`/`CutinEffects`) :
 *  - listing du dossier d'effets via `GET /api/cpk?path=<effectsDir>` ;
 *  - filtrage des `.g4pk` → `CutinLayerDescriptor` (le nom encode subject/phase/plan) ;
 *  - repérage du `*_camera.g4cm` → URL `/raw` du fichier ;
 *  - mapping modelGlbUrl/telops/soundRawUrl depuis l'entrée `skills-cutin`.
 *
 * Module pur / client-safe : aucun import server-only ; tout passe par `fetch` et les
 * helpers d'URL purs de `lib/cpk/shared`.
 */
import { cpkAssetUrl, cpkRawUrl } from "@rosegriffon/azalee/cpk/shared";
import type { CpkFile } from "@rosegriffon/azalee/cpk/shared";
import type { SkillCutin } from "@rosegriffon/azalee/game/skills-cutin";
import type { CutinLayerDescriptor, CutinManifest } from "./types";

/** Origine du CDN qui sert les GLB waza assemblés (nie-model-serve). */
const CDN = "https://cdn.rosegriffon.fr";

/**
 * Découpe `ev60_00340_c000101_s00_p00_c0010.g4pk` → `{subject, phase, plan}`.
 *
 * Le préfixe `<eventId>_` est retiré ; le reste (sans extension) est tokenisé. Les
 * jetons `sNN` (phase), `pNN` (plan technique, ignoré) et `cNNNN` final (plan caméra)
 * sont reconnus ; le reste (potentiellement multi-segments, ex. `point_eff`) forme le
 * sujet. Tolérant : tout champ non trouvé reste `null`/`"main"`.
 */
export function parseLayerName(
	eventId: string,
	fileName: string,
): { subject: string; phase: string | null; plan: string | null } {
	// Retire l'extension puis le préfixe `<eventId>_`.
	let stem = fileName.replace(/\.g4pk$/i, "");
	if (stem.startsWith(`${eventId}_`)) stem = stem.slice(eventId.length + 1);

	const tokens = stem.split("_").filter(Boolean);
	let phase: string | null = null;
	let plan: string | null = null;
	const subjectParts: string[] = [];

	for (const tok of tokens) {
		if (/^s\d{2,}$/i.test(tok)) {
			phase = tok.toLowerCase();
		} else if (/^p\d{2,}$/i.test(tok)) {
			// `pNN` = plan technique interne, non exposé.
		} else if (/^c\d{4}$/i.test(tok)) {
			plan = tok.toLowerCase();
		} else {
			subjectParts.push(tok);
		}
	}

	// Le sujet `<eventId>` répété (couche « main ») est normalisé.
	const subject = subjectParts.length === 0 || subjectParts.join("_") === eventId
		? "main"
		: subjectParts.join("_");
	return { subject, phase, plan };
}

/** Réponse minimale de `GET /api/cpk?path=<dir>` consommée ici. */
interface CpkListingResponse {
	dir: string;
	files?: CpkFile[];
}

/**
 * Construit le manifeste de l'event depuis l'entrée `skills-cutin`.
 *
 * @throws si l'API `/api/cpk` répond non-OK.
 */
export async function fetchCutinManifest(
	cutin: SkillCutin,
	signal?: AbortSignal,
): Promise<CutinManifest> {
	const dir = cutin.effects_dir.replace(/\/$/, "");
	const res = await fetch(`/api/cpk?path=${encodeURIComponent(dir)}`, { signal });
	if (!res.ok) {
		throw new Error(`Listing CPK indisponible (${res.status}) pour ${dir}`);
	}
	const json = (await res.json()) as CpkListingResponse;
	const files = json.files ?? [];

	// Couches = tous les .g4pk de l'event (renderable résolu plus tard par decode.ts).
	const layers: CutinLayerDescriptor[] = files
		.filter((f) => f.name.toLowerCase().endsWith(".g4pk"))
		.map((f) => {
			const { subject, phase, plan } = parseLayerName(cutin.event_id_name, f.name);
			return {
				id: f.name.replace(/\.g4pk$/i, ""),
				vfsPath: f.path,
				rawUrl: cpkRawUrl(f.path),
				subject,
				phase,
				plan,
				renderable: false,
			} satisfies CutinLayerDescriptor;
		});

	// Caméra : repérée par suffixe de nom (plus robuste que reconstruire le chemin).
	const cameraFile = files.find((f) => f.name.toLowerCase().endsWith("_camera.g4cm"));
	const cameraG4cmUrl = cameraFile ? cpkRawUrl(cameraFile.path) : null;

	// Telops par langue (g4tx → png via le CDN). On ignore les langues sans URL mappable.
	const telops = cutin.telop_by_lang
		.map(([lang, p]) => ({ lang, pngUrl: cpkAssetUrl(p) }))
		.filter((t): t is { lang: string; pngUrl: string } => t.pngUrl !== null);

	return {
		eventId: cutin.event_id_name,
		effectsDir: dir,
		modelGlbUrl: `${CDN}/model-chr/waza/${cutin.event_id_name}.glb`,
		cameraG4cmUrl,
		layers,
		telops,
		soundRawUrl: cutin.sound_cfg ? cpkRawUrl(cutin.sound_cfg) : null,
	};
}
