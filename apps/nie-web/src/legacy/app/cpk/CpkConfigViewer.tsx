"use client";

/**
 * Viewer d'un `cfg.bin` : tente d'abord le décodage TYPÉ (`/typed` → structures de jeu
 * nommées via nie-data), avec un rendu spécialisé par famille (terrain pour `formation`),
 * sinon l'arbre JSON de la donnée typée, sinon le RDBN/T2B brut (`/typed` générique ou
 * repli `/cfg`). Remplace l'ancien dump `<pre>` brut.
 */
import { useEffect, useState } from "react";
import { Icon } from "@/components/ui/Icon";
import { CpkFormationViewer } from "@/app/cpk/CpkFormationViewer";
import { CpkHexViewer } from "@/app/cpk/CpkHexViewer";
import { CpkJsonTree } from "@/app/cpk/CpkJsonTree";
import { CpkTypedTable } from "@/app/cpk/CpkTypedTable";
import { cpkCfgUrl, cpkRawUrl } from "@rosegriffon/azalee/cpk/shared";
import { cfgbinTyped } from "@/lib/cpk-wasm";

/**
 * Libellé humain d'une famille de jeu (badge).
 *
 * Les clés sont celles que rend le décodeur (`nie_data::typed::decode_by_key`), pas des noms
 * abrégés : la recherche est une égalité stricte (`FAMILY_LABEL[family]`). La table précédente
 * était keyée sur des raccourcis (`skill`, `item`, `formation`…) dont **une seule sur quinze**
 * (`chara_param`) existait réellement — les quatorze autres ne pouvaient jamais correspondre,
 * et toutes ces familles s'affichaient donc avec leur clé technique. Le décodeur nomme
 * `skill_config`, `item_config`, `formation_config`.
 *
 * Une famille absente d'ici retombe sur sa clé technique, ce qui reste juste. C'est
 * volontaire pour celles dont le nom ne permet pas de conclure (`inacode_config`,
 * `msa999999_trigger`, `flag_config`…) : mieux vaut une clé brute qu'un libellé inventé.
 */
const FAMILY_LABEL: Record<string, string> = {
	// Personnages
	chara_base: "Personnages — base",
	chara_param: "Personnages — paramètres",
	chara_details_config: "Personnages — détails",
	chara_description_text: "Personnages — descriptions",
	chara_series_config: "Personnages — séries",
	chara_costume: "Costumes",
	chara_exp_table_config: "Tables d'expérience",
	chara_menu_resource: "Ressources de menu personnage",
	basara_chara_config: "Personnages Basara",
	ctrl_chara_config: "Personnages jouables",
	// Techniques, auras, passifs
	skill_config: "Techniques (hissatsu)",
	skill_technic_config: "Techniques — technique",
	skill_view_preset_config: "Techniques — préréglages d'affichage",
	override_skill_config: "Techniques de remplacement",
	real_skill_config: "Techniques réelles",
	aura_skill_config: "Auras / esprits guerriers",
	change_aura_skill_config: "Changements d'aura",
	passive_skill_config: "Compétences passives",
	// Objets et économie
	item_config: "Objets",
	item_emission_rarity_table_config: "Objets — table de rareté",
	capsule_config: "Capsules",
	shop_config: "Boutiques",
	craft_obj_config: "Artisanat — objets",
	craft_theme_config: "Artisanat — thèmes",
	delivery_config: "Livraisons",
	delivery_list_config: "Livraisons — listes",
	// Équipes
	formation_config: "Formations",
	belong_team_config: "Équipes d'appartenance",
	opponent_team_config: "Équipes adverses",
	enjoy_mode_team_config: "Équipes du mode détente",
	uniform_config: "Uniformes",
	emblem_resource: "Emblèmes",
	boost_player_group_config: "Groupes de joueurs renforcés",
	// Match
	soccer_game_config: "Match — règles",
	soccer_game_additional_config: "Match — règles complémentaires",
	soccer_game_map_enviroment_config: "Match — environnement de terrain",
	soccer_chara_placement: "Match — placement des joueurs",
	soccer_technic_config: "Match — techniques",
	soccer_drop_config: "Match — butin",
	soccer_rank_config: "Match — classement",
	soccer_player_record_config: "Match — records de joueur",
	soccer_opponent_info: "Match — infos sur l'adversaire",
	soccer_suggest_config: "Match — suggestions",
	soccer_club_room_config: "Salle de club",
	soccer_basic_effect_config: "Match — effets de base",
	soccer_focus_battle_effect_config: "Match — effets de duel",
	soccer_fixed_reward_spirit_config: "Match — récompenses d'esprits",
	soccer_cmd_action: "Match — actions",
	soccer_cmd_event: "Match — événements",
	// Intelligence artificielle
	ai_type_config: "IA — types",
	soccer_ai_cmd_config: "IA — commandes de match",
	soccer_user_ai_config: "IA — joueur",
	strategy_ai_config: "IA — stratégie",
	tactics_ai_config: "IA — tactique",
	// Combat RPG
	rpg_battle_rule_config: "Combat RPG — règles",
	rpg_battle_party_config: "Combat RPG — équipe",
	rpg_battle_status_pattern_config: "Combat RPG — états",
	rpg_battle_add_status_config: "Combat RPG — états ajoutés",
	rpg_battle_cmd_set_config: "Combat RPG — jeux de commandes",
	rpg_battle_cmd_obj_config: "Combat RPG — objets de commande",
	rpg_battle_cmd_event_config: "Combat RPG — événements de commande",
	rpg_battle_chara_swap_motion_config: "Combat RPG — relais de personnage",
	rpg_cmd_action: "RPG — actions",
	rpg_cmd_event: "RPG — événements",
	// Progression, collection
	mission_config: "Missions",
	quest_config: "Quêtes",
	game_quest_config: "Quêtes de jeu",
	trophy_config: "Trophées",
	record_config: "Records",
	gallery_config: "Galerie",
	dictionary_config: "Dictionnaire",
	scene_archive_config: "Archives de scènes",
	extend_story_data_config: "Histoire étendue",
	players_universe_config: "Univers des joueurs",
	players_universe_event_config: "Univers des joueurs — événements",
	// Modes et lieux
	chronicle_top_caravan_config: "Chronique — caravane",
	chronicle_vs_route_config: "Chronique — parcours de match",
	friendmap_config: "Carte d'amitié",
	fast_travel_config: "Voyage rapide",
	party_departure: "Départ de groupe",
	// Interface, système
	setting_list_config: "Réglages",
	help_list_config: "Aide",
	tutorial_banner_config: "Bannières de tutoriel",
	post_notice_config: "Annonces",
	update_notice_config: "Notice de mise à jour",
	system_unlock_window_config: "Déblocages système",
	info_bookmark_config: "Signets d'information",
	search_word_config: "Mots-clés de recherche",
	user_name_plate_config: "Plaques de nom",
	password_list_config: "Mots de passe",
	guest_limit_config: "Limites d'invités",
	phase_title_config: "Titres de phase",
	// Médias, effets, périphériques
	movie_playing_config: "Lecture de vidéos",
	music_app_config: "Application musique",
	photo_mode_random_pose_config: "Mode photo — poses aléatoires",
	light_overwrite_config: "Éclairage — surcharges",
	weather_convert: "Météo",
	event_map_tag_config: "Événements — étiquettes de carte",
	happen_event_npc_common: "Événements — PNJ",
	chara_cmd_event_common: "Personnages — événements de commande",
	chat_emote_config: "Émotes de discussion",
	chat_emote_def_set_config: "Émotes de discussion — jeux",
	advent_calendar_config: "Calendrier de l'avent",
	nfc_lottery_config: "Loterie NFC",
	adaptive_trigger_def: "Gâchettes adaptatives",
	haptic_feedback_def: "Retour haptique",
	vibration_def: "Vibration",
	trial_take_over_config: "Reprise d'essai",
	gimmick_system_num_config: "Gimmicks",
};

interface State {
	status: "loading" | "typed" | "generic" | "error";
	family?: string | null;
	data?: unknown;
	/** Origine d'un rendu `generic` : `wasm` = RDBN/T2B brut, `cfg` = décodage serveur /cfg. */
	source?: "wasm" | "cfg";
	/** Octets bruts déjà téléchargés (rendu hex sans 2e téléchargement sur binaire opaque). */
	bytes?: Uint8Array;
}

export function CpkConfigViewer({ path }: { path: string }) {
	const [state, setState] = useState<State>({ status: "loading" });

	useEffect(() => {
		let cancelled = false;
		setState({ status: "loading" });

		(async () => {
			const filename = path.split("/").pop() ?? path;
			// 1. Décodage typé NATIF IN-BROWSER (wasm nie-data) depuis les octets bruts (/raw).
			//    Mêmes parseurs Rust que le jeu — aucune dépendance à la route serveur /typed.
			let rawBytes: Uint8Array | undefined;
			try {
				const res = await fetch(cpkRawUrl(path));
				if (res.ok) {
					rawBytes = new Uint8Array(await res.arrayBuffer());
					const body = await cfgbinTyped(rawBytes, filename);
					if (cancelled) return;
					if (body.family) {
						setState({ status: "typed", family: body.family, data: body.data });
					} else {
						setState({ status: "generic", family: null, data: body.generic, source: "wasm" });
					}
					return;
				}
			} catch {
				/* octets non RDBN/T2B ou wasm indispo → repli serveur /cfg */
			}
			// 2. Repli serveur : /cfg (formats hiérarchiques hors RDBN/T2B à listes).
			try {
				const res = await fetch(cpkCfgUrl(path));
				if (!res.ok) throw new Error(String(res.status));
				const body = (await res.json()) as unknown;
				if (!cancelled) setState({ status: "generic", family: null, data: body, source: "cfg" });
			} catch {
				// Binaire opaque : aperçu hex à partir des octets DÉJÀ en main (aucun 2e fetch).
				if (!cancelled) setState({ status: "error", bytes: rawBytes });
			}
		})();

		return () => {
			cancelled = true;
		};
	}, [path]);

	if (state.status === "loading") {
		return (
			<div className="flex items-center justify-center p-10">
				<div className="animate-spin rounded-full size-7 border-b-2 border-primary" />
			</div>
		);
	}

	if (state.status === "error") {
		// Pas un cfg.bin RDBN/T2B (ex. .objbin/.fxbin/.bin binaire opaque) → dump hexadécimal
		// utile plutôt qu'un cul-de-sac. Le bouton de téléchargement vit dans CpkFilePreview.
		// On réutilise les octets déjà fetchés (state.bytes) ; repli /raw si le 1er fetch a échoué.
		return (
			<div>
				<div className="px-4 pt-3">
					<span className="
       inline-flex items-center gap-1.5 rounded-full bg-surface-container-high px-3 py-1 text-xs text-on-surface-variant
     ">
						<Icon name="memory" size={14} /> Binaire opaque — aperçu hexadécimal
					</span>
				</div>
				{state.bytes ? (
					<CpkHexViewer bytes={state.bytes} />
				) : (
					<CpkHexViewer url={cpkRawUrl(path)} />
				)}
			</div>
		);
	}

	const { family, data } = state;

	// Rendu spécialisé : terrain de formation.
	if (state.status === "typed" && family === "formation") {
		return (
			<div>
				<FamilyBadge family={family} />
				<CpkFormationViewer data={data as never} />
			</div>
		);
	}

	// Famille typée reconnue → table (si liste d'objets) ou arbre JSON (sinon).
	if (state.status === "typed" && family) {
		return (
			<div>
				<FamilyBadge family={family} />
				<CpkTypedTable data={data} />
			</div>
		);
	}

	// Générique → arbre JSON. Le libellé reflète l'origine réelle du décodage.
	return (
		<div>
			<div className="px-4 pt-3">
				<span className="
      inline-flex items-center gap-1.5 rounded-full bg-surface-container-high px-3 py-1 text-xs text-on-surface-variant
    ">
					<Icon name="data_object" size={14} />
					{state.source === "cfg"
						? "Config décodée (serveur)"
						: "Config binaire brute (RDBN / T2B)"}
				</span>
			</div>
			<CpkJsonTree data={data} rootLabel="config" />
		</div>
	);
}

function FamilyBadge({ family }: { family: string }) {
	return (
		<div className="px-4 pt-3">
			<span className="
     inline-flex items-center gap-1.5 rounded-full bg-primary/15 text-primary px-3 py-1 text-xs font-semibold
   ">
				<Icon name="auto_awesome" size={14} />
				{FAMILY_LABEL[family] ?? family} · décodé par niers
			</span>
		</div>
	);
}
