// Encyclopédie — données de jeu STATIQUES, décodées EN DIRECT du VFS par les vrais parseurs
// typés de `nie-data` (`src-tauri/src/game_data.rs`). Aucun miroir wiki, aucune requête réseau :
// les `.cfg.bin` du jeu monté, et rien d'autre.
//
// ## Ce que cette vue a de plus que l'encyclopédie du wiki
//
//  1. **Vingt-quatre familles**, dont huit que le wiki n'expose nulle part : la fiche complète des
//     personnages (identité, série, équipe, techniques apprises), les équipes adverses, les
//     vidéos, la bande-son, le dictionnaire in-game, la courbe d'expérience, le butin et les taux
//     de tirage des capsules.
//  2. Un **tableau virtualisé triable** (`ui/data-grid`, fenêtre glissante) au lieu d'une liste :
//     6 000 personnages tiennent dans la même page que 108 pistes musicales.
//  3. Un **export CSV/JSON** de ce qui est à l'écran (filtre ET tri appliqués).
//  4. Le **détail de la ligne** systématique — et, quand l'entité porte un code interne, l'éditeur
//     de propriétés complet (ses fichiers, ses `.cfg.bin` éditables, les fonctions de `nie.exe`
//     qui la manipulent), ce qu'aucune page web ne peut faire.
//
// ## Registre déclaratif
//
// Chaque famille se décrit en une entrée de [`FAMILLES`] : son chargement, ses colonnes, son code
// interne, ses champs de recherche. Le rendu est UNIQUE — ajouter une famille n'ajoute pas une
// branche de `if` de plus (c'était le défaut de la version précédente : 16 blocs `kind === …`
// copiés-collés, chacun avec sa propre mise en page).
import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { toast } from "sonner";

import {
  api,
  type Activity,
  type Aura,
  type BelongTeam,
  type CapsuleRate,
  type Chara,
  type DictionaryEntry,
  type Drop,
  type Emblem,
  type ExpLevel,
  type Formation,
  type Gallery,
  type Item,
  type Movie,
  type Music,
  type OpponentTeam,
  type Passive,
  type Quest,
  type Shop,
  type Skill,
  type SpecialTactics,
  type Stadium,
  type Trick,
  type Trophy,
  type Uniform,
} from "@/lib/api";
import { useSettings } from "@/lib/settings";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { DataGrid, type DataGridColumn } from "@/components/ui/data-grid";
import { Icon } from "@/components/ui/Icon";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
// Cartes du wiki, migrées dans l'application (`components/wiki/`) : le mode grille de
// l'encyclopédie rend EXACTEMENT les mêmes composants que les pages du site.
import { AuraCard } from "@/components/wiki/AuraCard";
import { CharaCard } from "@/components/wiki/CharaCard";
import { GalleryCard } from "@/components/wiki/GalleryCard";
import { ItemCard } from "@/components/wiki/ItemCard";
import { MoveCard } from "@/components/wiki/MoveCard";
import { StadiumCard } from "@/components/wiki/StadiumCard";
import { TacticCard } from "@/components/wiki/TacticCard";
import { PropertyEditor } from "@/components/PropertyEditor";
import { StatCalculator } from "@/components/tools/StatCalculator";
import { estAbsent } from "@/lib/valeurs";
import { cn } from "@/lib/utils";

/** Valeur affichable d'une cellule — `null` rend une cellule vide, jamais la chaîne « null ». */
type Cellule = string | number | boolean | null;

interface Colonne<T> {
  key: string;
  label: string;
  /** Largeur CSS dans `grid-template-columns` (défaut : `8rem`). */
  width?: string;
  valeur: (ligne: T) => Cellule;
  /** Info-bulle de l'en-tête. */
  title?: string;
}

interface Famille<T> {
  cle: string;
  libelle: string;
  groupe: string;
  icone: string;
  /** Une phrase : ce que la famille contient et d'où elle vient. */
  source: string;
  charger: (gameDir?: string) => Promise<T[]>;
  colonnes: Colonne<T>[];
  /** Code interne ouvrant l'éditeur de propriétés — `null` quand l'entité n'a aucun asset. */
  code?: (ligne: T) => string | null;
  /** Champs texte balayés par le filtre (défaut : toutes les colonnes). */
  recherche?: (ligne: T) => (string | null | undefined)[];
  /** Carte du wiki (`components/wiki/*`) pour le mode grille — absente = tableau seul. */
  carte?: (ligne: T, selectionnee: boolean, onSelect: () => void) => ReactNode;
}

/** Raccourci : déclare une famille en gardant l'inférence de `T` (sinon tout tombe en `unknown`). */
function famille<T>(f: Famille<T>): Famille<T> {
  return f;
}

const oui = (b: boolean, libelle = "oui") => (b ? libelle : null);

// ─── Registre ────────────────────────────────────────────────────────────────────────────────

const FAMILLES = [
  famille<Chara>({
    cle: "charas",
    libelle: "Personnages",
    groupe: "Personnages",
    icone: "person",
    source: "chara_param + chara_base + chara_text + chara_series + belong_team + skill_config",
    charger: (d) => api.gameDataCharas(d),
    code: (c) => c.internal_code || null,
    colonnes: [
      { key: "name", label: "Nom", width: "14rem", valeur: (c) => c.name },
      { key: "internal_code", label: "Code", width: "8rem", valeur: (c) => c.internal_code },
      { key: "element", label: "Élément", width: "7rem", valeur: (c) => c.element },
      { key: "main_position", label: "Poste", width: "5rem", valeur: (c) => c.main_position },
      { key: "sub_position", label: "2ᵉ poste", width: "5rem", valeur: (c) => c.sub_position },
      { key: "team", label: "Équipe", width: "12rem", valeur: (c) => c.team },
      { key: "series", label: "Série", width: "10rem", valeur: (c) => c.series },
      { key: "skill_count", label: "Techniques", width: "6rem", valeur: (c) => c.skill_count },
      { key: "gender", label: "Genre", width: "5rem", valeur: (c) => (c.gender === 2 ? "F" : c.gender === 1 ? "M" : String(c.gender)) },
      { key: "description", label: "Description", width: "24rem", valeur: (c) => c.description },
    ],
    recherche: (c) => [c.name, c.internal_code, c.team, c.series, c.element, c.main_position],
    carte: (c, actif, choisir) => (
      <CharaCard
        code={c.internal_code}
        name={c.name}
        element={c.element}
        mainPosition={c.main_position}
        subPosition={c.sub_position}
        team={c.team}
        series={c.series}
        total={c.stats.total ?? undefined}
        skillCount={c.skill_count ?? undefined}
        selected={actif}
        onClick={choisir}
      />
    ),
  }),
  famille<DictionaryEntry>({
    cle: "dictionary",
    libelle: "Dictionnaire",
    groupe: "Personnages",
    icone: "menu_book",
    source: "dictionary_config — le bestiaire in-game, habitats résolus par map_text",
    charger: (d) => api.gameDataDictionary(d),
    colonnes: [
      { key: "view_dict_no", label: "N°", width: "4rem", valeur: (e) => e.view_dict_no },
      { key: "name", label: "Personnage", width: "14rem", valeur: (e) => e.name ?? e.chara_id },
      { key: "habitat", label: "Habitat", width: "12rem", valeur: (e) => e.habitat },
      { key: "habitat_file", label: "Carte", width: "10rem", valeur: (e) => e.habitat_file },
      { key: "category", label: "Catégorie", width: "6rem", valeur: (e) => e.category },
      { key: "sub_category", label: "Sous-cat.", width: "6rem", valeur: (e) => e.sub_category },
      { key: "is_battle", label: "Affrontable", width: "7rem", valeur: (e) => oui(e.is_battle) },
      { key: "observation_count", label: "Observations", width: "7rem", valeur: (e) => e.observation_count },
      { key: "chara_id", label: "chara_id", width: "9rem", valeur: (e) => e.chara_id },
    ],
    recherche: (e) => [e.name, e.habitat, e.habitat_file, e.chara_id],
  }),

  famille<Skill>({
    cle: "skills",
    libelle: "Techniques",
    groupe: "Combat",
    icone: "bolt",
    source: "skill_config + skill_text (FR)",
    charger: (d) => api.gameDataSkills(d),
    code: (s) => s.skill_id_str || null,
    colonnes: [
      { key: "name", label: "Nom", width: "14rem", valeur: (s) => s.name ?? s.skill_id_str },
      { key: "skill_id_str", label: "Code", width: "8rem", valeur: (s) => s.skill_id_str },
      { key: "element", label: "Élément", width: "7rem", valeur: (s) => s.element },
      { key: "category", label: "Type", width: "7rem", valeur: (s) => s.category },
      { key: "power_min", label: "Puis. min", width: "6rem", valeur: (s) => s.power_min },
      { key: "power_max", label: "Puis. max", width: "6rem", valeur: (s) => s.power_max },
      { key: "consume_tp", label: "TP", width: "4rem", valeur: (s) => s.consume_tp },
      { key: "recast_time", label: "Recharge", width: "6rem", valeur: (s) => s.recast_time },
      { key: "eldorado", label: "Eldorado", width: "6rem", valeur: (s) => oui(s.eldorado) },
      { key: "description", label: "Description", width: "24rem", valeur: (s) => s.description },
    ],
    recherche: (s) => [s.name, s.skill_id_str, s.element, s.category],
    carte: (s, actif, choisir) => (
      <div
        role="button"
        tabIndex={0}
        onClick={choisir}
        onKeyDown={(e) => e.key === "Enter" && choisir()}
        className={actif ? "rounded-xl ring-2 ring-accent" : ""}
      >
        <MoveCard
          id={s.skill_id_str}
          name={s.name ?? s.skill_id_str}
          powerMin={s.power_min}
          powerMax={s.power_max}
          tensionCost={s.consume_tp}
          element={s.element}
          category={s.category}
        />
      </div>
    ),
  }),
  famille<SpecialTactics>({
    cle: "tactics",
    libelle: "Tactiques",
    groupe: "Combat",
    icone: "strategy",
    source: "special_tactics_config",
    charger: (d) => api.gameDataSpecialTactics(d),
    code: (t) => t.internal_code || null,
    colonnes: [
      { key: "name", label: "Nom", width: "14rem", valeur: (t) => t.name ?? t.internal_code },
      { key: "internal_code", label: "Code", width: "8rem", valeur: (t) => t.internal_code },
      { key: "element", label: "Élément", width: "7rem", valeur: (t) => t.element },
      { key: "power", label: "Puissance", width: "6rem", valeur: (t) => t.power },
      { key: "partner_count", label: "Partenaires", width: "7rem", valeur: (t) => t.partner_count },
      { key: "description", label: "Description", width: "24rem", valeur: (t) => t.description },
    ],
    recherche: (t) => [t.name, t.internal_code, t.element],
    carte: (t, actif, choisir) => (
      <div
        role="button"
        tabIndex={0}
        onClick={choisir}
        onKeyDown={(e) => e.key === "Enter" && choisir()}
        className={actif ? "rounded-xl ring-2 ring-accent" : ""}
      >
        <TacticCard id={t.internal_code} name={t.name ?? t.internal_code} categoryLabel={t.element} />
      </div>
    ),
  }),
  famille<Passive>({
    cle: "passives",
    libelle: "Passifs",
    groupe: "Combat",
    icone: "auto_awesome",
    source: "passive_config",
    charger: (d) => api.gameDataPassives(d),
    colonnes: [
      { key: "name", label: "Nom", width: "14rem", valeur: (p) => p.name ?? p.passive_id },
      { key: "scope", label: "Portée", width: "8rem", valeur: (p) => p.scope },
      { key: "boost_type", label: "Effet", width: "10rem", valeur: (p) => p.boost_type },
      { key: "rarity", label: "Rareté", width: "5rem", valeur: (p) => p.rarity },
      { key: "description", label: "Description", width: "24rem", valeur: (p) => p.description },
      { key: "passive_id", label: "ID", width: "9rem", valeur: (p) => p.passive_id },
    ],
    recherche: (p) => [p.name, p.description, p.scope, p.boost_type],
  }),
  famille<Trick>({
    cle: "tricks",
    libelle: "Feintes",
    groupe: "Combat",
    icone: "sports_soccer",
    source: "trick_config",
    charger: (d) => api.gameDataTricks(d),
    code: (t) => t.trick_id_name || null,
    colonnes: [
      { key: "trick_name", label: "Nom", width: "14rem", valeur: (t) => t.trick_name || t.trick_id_name },
      { key: "trick_id_name", label: "Code", width: "10rem", valeur: (t) => t.trick_id_name },
      { key: "category", label: "Catégorie", width: "8rem", valeur: (t) => t.category },
      { key: "event_id_name", label: "Événement", width: "12rem", valeur: (t) => t.event_id_name },
      { key: "has_fail_event", label: "Échec scripté", width: "8rem", valeur: (t) => oui(t.has_fail_event) },
    ],
    recherche: (t) => [t.trick_name, t.trick_id_name, t.category],
  }),
  famille<Aura>({
    cle: "auras",
    libelle: "Avatar / Keshin",
    groupe: "Combat",
    icone: "flare",
    source: "aura_config + text FR",
    charger: (d) => api.gameDataAuras(d),
    code: (a) => a.asset_code || null,
    colonnes: [
      { key: "name", label: "Nom", width: "14rem", valeur: (a) => a.name },
      { key: "asset_code", label: "Code", width: "9rem", valeur: (a) => a.asset_code },
      { key: "element", label: "Élément", width: "7rem", valeur: (a) => a.element },
      { key: "sub_type", label: "Type", width: "8rem", valeur: (a) => a.sub_type },
      { key: "description", label: "Description", width: "24rem", valeur: (a) => a.description },
    ],
    recherche: (a) => [a.name, a.asset_code, a.element, a.sub_type],
    carte: (a, actif, choisir) => (
      <div
        role="button"
        tabIndex={0}
        onClick={choisir}
        onKeyDown={(e) => e.key === "Enter" && choisir()}
        className={actif ? "rounded-xl ring-2 ring-accent" : ""}
      >
        <AuraCard
          id={a.aura_id}
          name={a.name}
          description={a.description ?? undefined}
          assetCode={a.asset_code ?? undefined}
          subType={a.sub_type}
          category={a.sub_type}
          element={{ fr: a.element }}
        />
      </div>
    ),
  }),

  famille<Item>({
    cle: "items",
    libelle: "Objets",
    groupe: "Objets & butin",
    icone: "inventory_2",
    source: "item_config + item_text (FR)",
    charger: (d) => api.gameDataItems(d),
    code: (i) => i.internal_code,
    colonnes: [
      { key: "name", label: "Nom", width: "16rem", valeur: (i) => i.name },
      { key: "category", label: "Catégorie", width: "10rem", valeur: (i) => i.category },
      { key: "price", label: "Prix", width: "7rem", valeur: (i) => i.price },
      { key: "internal_code", label: "Code", width: "9rem", valeur: (i) => i.internal_code },
      { key: "description", label: "Description", width: "26rem", valeur: (i) => i.description },
    ],
    recherche: (i) => [i.name, i.category, i.internal_code],
    carte: (i, actif, choisir) => (
      <div
        role="button"
        tabIndex={0}
        onClick={choisir}
        onKeyDown={(e) => e.key === "Enter" && choisir()}
        className={actif ? "rounded-xl ring-2 ring-accent" : ""}
      >
        <ItemCard
          id={i.item_id}
          name={i.name}
          category={i.category}
          price={i.price}
          internalCode={i.internal_code ?? undefined}
        />
      </div>
    ),
  }),
  famille<Shop>({
    cle: "shops",
    libelle: "Boutiques",
    groupe: "Objets & butin",
    icone: "storefront",
    source: "shop_config + shop_text, inventaire résolu contre item_config",
    charger: (d) => api.gameDataShops(d),
    colonnes: [
      { key: "name", label: "Boutique", width: "16rem", valeur: (s) => s.name ?? s.shop_id },
      { key: "item_count", label: "Objets", width: "5rem", valeur: (s) => s.item_count },
      { key: "items", label: "Inventaire", width: "40rem", valeur: (s) => s.items.join(" · ") },
    ],
    recherche: (s) => [s.name, s.shop_id, ...s.items],
  }),
  famille<Drop>({
    cle: "drops",
    libelle: "Butin",
    groupe: "Objets & butin",
    icone: "casino",
    source: "soccer_drop_config — table des esprits tirables, poids convertis en pourcentage",
    charger: (d) => api.gameDataDrops(d),
    colonnes: [
      { key: "name", label: "Personnage", width: "16rem", valeur: (d) => d.name ?? d.chara_id },
      { key: "weight", label: "Poids", width: "6rem", valeur: (d) => d.weight },
      { key: "share_pct", label: "Part", width: "6rem", valeur: (d) => `${(d.share_pct ?? 0).toFixed(3)} %` },
      { key: "run_cond", label: "Condition", width: "20rem", valeur: (d) => d.run_cond },
      { key: "chara_id", label: "chara_id", width: "9rem", valeur: (d) => d.chara_id },
    ],
    recherche: (d) => [d.name, d.chara_id, d.run_cond],
  }),
  famille<CapsuleRate>({
    cle: "capsules",
    libelle: "Capsules",
    groupe: "Objets & butin",
    icone: "egg",
    source: "capsule_config — taux de tirage par rang, part calculée sur le total de la table",
    charger: (d) => api.gameDataCapsuleRates(d),
    colonnes: [
      { key: "table_id", label: "Table", width: "10rem", valeur: (c) => c.table_id },
      { key: "rank", label: "Rang", width: "5rem", valeur: (c) => c.rank },
      { key: "rate", label: "Taux brut", width: "7rem", valeur: (c) => c.rate },
      { key: "share_pct", label: "Part", width: "7rem", valeur: (c) => `${(c.share_pct ?? 0).toFixed(2)} %` },
    ],
    recherche: (c) => [c.table_id],
  }),

  famille<BelongTeam>({
    cle: "teams",
    libelle: "Équipes",
    groupe: "Équipes",
    icone: "groups",
    source: "belong_team_config + team_text (FR)",
    charger: (d) => api.gameDataBelongTeams(d),
    colonnes: [
      { key: "name", label: "Équipe", width: "18rem", valeur: (t) => t.name ?? t.team_id },
      { key: "seasons", label: "Saisons", width: "18rem", valeur: (t) => t.seasons.join(" · ") },
      { key: "binder_order", label: "Ordre", width: "5rem", valeur: (t) => t.binder_order },
      { key: "emblem_id_v", label: "Écusson", width: "10rem", valeur: (t) => t.emblem_id_v },
      { key: "team_id", label: "ID", width: "10rem", valeur: (t) => t.team_id },
    ],
    recherche: (t) => [t.name, t.team_id, ...t.seasons],
  }),
  famille<OpponentTeam>({
    cle: "opponents",
    libelle: "Adversaires",
    groupe: "Équipes",
    icone: "swords",
    source: "opponent_team_config, noms joints par belong_team + team_text",
    charger: (d) => api.gameDataOpponentTeams(d),
    colonnes: [
      { key: "team_name", label: "Équipe", width: "18rem", valeur: (o) => o.team_name ?? o.team_id },
      { key: "difficulty_type", label: "Difficulté", width: "6rem", valeur: (o) => o.difficulty_type },
      { key: "team_type", label: "Type", width: "5rem", valeur: (o) => o.team_type },
      { key: "flag_no", label: "Flag", width: "5rem", valeur: (o) => o.flag_no },
      { key: "open_cond", label: "Condition d'ouverture", width: "20rem", valeur: (o) => o.open_cond },
      { key: "formation_cond", label: "Condition de formation", width: "16rem", valeur: (o) => o.formation_cond },
      { key: "bg_texture_name", label: "Fond", width: "14rem", valeur: (o) => o.bg_texture_name },
      { key: "opponent_id", label: "ID", width: "10rem", valeur: (o) => o.opponent_id },
    ],
    recherche: (o) => [o.team_name, o.opponent_id, o.open_cond, o.bg_texture_name],
  }),
  famille<Formation>({
    cle: "formations",
    libelle: "Formations",
    groupe: "Équipes",
    icone: "grid_view",
    // `formation_text.cfg.bin` n'existe pas dans cette version : les libellés RESTENT des hachages.
    source: "formation_config — sans table de texte dans cette version du jeu, les noms sont des hachages",
    charger: (d) => api.gameDataFormations(d),
    colonnes: [
      { key: "form_id", label: "Formation", width: "10rem", valeur: (f) => f.form_id },
      { key: "positions", label: "Placements", width: "18rem", valeur: (f) => f.positions.join("-") || "aucun" },
      { key: "power_offense", label: "Attaque", width: "6rem", valeur: (f) => f.power_offense },
      { key: "power_defense", label: "Défense", width: "6rem", valeur: (f) => f.power_defense },
      { key: "placement_count", label: "Postes", width: "5rem", valeur: (f) => f.placement_count },
      { key: "noun_id", label: "Libellé (hash)", width: "10rem", valeur: (f) => f.noun_id },
    ],
    recherche: (f) => [f.form_id, f.noun_id],
  }),
  famille<Uniform>({
    cle: "uniforms",
    libelle: "Uniformes",
    groupe: "Équipes",
    icone: "checkroom",
    source: "uniform_config (character/) — tranches de modèles résolues",
    charger: (d) => api.gameDataUniforms(d),
    colonnes: [
      { key: "name_id", label: "Uniforme", width: "10rem", valeur: (u) => u.name_id },
      { key: "type_id", label: "Type", width: "5rem", valeur: (u) => u.type_id },
      { key: "resolved_count", label: "Modèles", width: "6rem", valeur: (u) => `${u.resolved_count} / ${u.model_count}` },
      { key: "fielder_model_id", label: "Joueur", width: "10rem", valeur: (u) => u.fielder_model_id },
      { key: "keeper_model_id", label: "Gardien", width: "10rem", valeur: (u) => u.keeper_model_id },
    ],
    recherche: (u) => [u.name_id, u.fielder_model_id, u.keeper_model_id],
  }),
  famille<Emblem>({
    cle: "emblems",
    libelle: "Écussons",
    groupe: "Équipes",
    icone: "shield",
    source: "emblem_config",
    charger: (d) => api.gameDataEmblems(d),
    code: (e) => (e.is_template ? null : e.emblem_name || null),
    colonnes: [
      { key: "emblem_name", label: "Écusson", width: "16rem", valeur: (e) => e.emblem_name || e.emblem_id },
      { key: "is_template", label: "Gabarit", width: "6rem", valeur: (e) => oui(e.is_template) },
      { key: "large_file_path", label: "Fichier", width: "30rem", valeur: (e) => e.large_file_path },
      { key: "emblem_id", label: "ID", width: "10rem", valeur: (e) => e.emblem_id },
    ],
    recherche: (e) => [e.emblem_name, e.emblem_id, e.large_file_path],
  }),

  famille<Stadium>({
    cle: "stadiums",
    libelle: "Stades",
    groupe: "Monde",
    icone: "stadium",
    source: "stadium_config",
    charger: (d) => api.gameDataStadiums(d),
    code: (s) => s.image_path.split("/").pop() ?? null,
    colonnes: [
      { key: "name", label: "Stade", width: "18rem", valeur: (s) => s.name },
      { key: "field_id", label: "Terrain", width: "10rem", valeur: (s) => s.field_id },
      { key: "locked", label: "À débloquer", width: "7rem", valeur: (s) => oui(s.locked) },
      { key: "image_path", label: "Image", width: "30rem", valeur: (s) => s.image_path },
    ],
    recherche: (s) => [s.name, s.field_id, s.image_path],
    carte: (s, actif, choisir) => (
      <div
        role="button"
        tabIndex={0}
        onClick={choisir}
        onKeyDown={(e) => e.key === "Enter" && choisir()}
        className={actif ? "rounded-xl ring-2 ring-accent" : ""}
      >
        <StadiumCard id={s.field_id} code={s.field_id} title={s.name} index={s.index ?? undefined} />
      </div>
    ),
  }),
  famille<Quest>({
    cle: "quests",
    libelle: "Quêtes",
    groupe: "Monde",
    icone: "task_alt",
    source: "quest_config + quest_title_text (FR)",
    charger: (d) => api.gameDataQuests(d),
    colonnes: [
      { key: "title", label: "Quête", width: "30rem", valeur: (q) => q.title },
      { key: "phase", label: "Phase", width: "5rem", valeur: (q) => q.phase },
      { key: "quest_id", label: "ID", width: "10rem", valeur: (q) => q.quest_id },
    ],
    recherche: (q) => [q.title, q.quest_id],
  }),
  famille<Activity>({
    cle: "activities",
    libelle: "Activités",
    groupe: "Monde",
    icone: "account_tree",
    source: "activity_config — arbre des sous-tâches",
    charger: (d) => api.gameDataActivities(d),
    colonnes: [
      { key: "name", label: "Activité", width: "24rem", valeur: (a) => a.name },
      { key: "is_root", label: "Racine", width: "6rem", valeur: (a) => oui(a.is_root) },
      { key: "parent_id", label: "Parent", width: "10rem", valeur: (a) => (a.is_root ? null : a.parent_id) },
      { key: "kind", label: "Type", width: "5rem", valeur: (a) => a.kind },
      // Le blob `data` (base64) n'est pas décodé : sa sémantique n'est établie par aucune source.
      { key: "data_len", label: "Données", width: "7rem", valeur: (a) => `${a.data_len} o.` },
    ],
    recherche: (a) => [a.name, a.id],
  }),
  famille<Trophy>({
    cle: "trophies",
    libelle: "Succès",
    groupe: "Monde",
    icone: "emoji_events",
    source: "trophy_config + trophy_text (FR)",
    charger: (d) => api.gameDataTrophies(d),
    code: (t) => t.code || null,
    colonnes: [
      { key: "name", label: "Succès", width: "20rem", valeur: (t) => t.name },
      { key: "unlock_kind", label: "Déblocage", width: "10rem", valeur: (t) => t.unlock_kind },
      { key: "story_episode", label: "Épisode", width: "6rem", valeur: (t) => t.story_episode },
      { key: "description", label: "Description", width: "30rem", valeur: (t) => t.description },
      { key: "code", label: "Code", width: "9rem", valeur: (t) => t.code },
    ],
    recherche: (t) => [t.name, t.description, t.code],
  }),
  famille<Gallery>({
    cle: "gallery",
    libelle: "Galerie",
    groupe: "Monde",
    icone: "image",
    source: "gallery_config",
    charger: (d) => api.gameDataGallery(d),
    code: (g) => g.img_path || null,
    colonnes: [
      { key: "img_path", label: "Illustration", width: "26rem", valeur: (g) => g.img_path },
      { key: "unlock_kind", label: "Déblocage", width: "10rem", valeur: (g) => g.unlock_kind },
      { key: "story_episode", label: "Épisode", width: "6rem", valeur: (g) => g.story_episode },
      { key: "flg_no", label: "Flag", width: "5rem", valeur: (g) => g.flg_no },
      { key: "thumb_path", label: "Vignette", width: "26rem", valeur: (g) => g.thumb_path },
    ],
    recherche: (g) => [g.img_path, g.thumb_path, g.unlock_kind],
    carte: (g, actif, choisir) => (
      <div className={actif ? "rounded-xl ring-2 ring-accent" : ""}>
        <GalleryCard
          id={g.gallery_id}
          title={g.img_path.split("/").pop() ?? g.gallery_id}
          // Les illustrations vivent dans le VFS : `ui/image.tsx` décode le `.g4tx` lui-même.
          thumb={g.thumb_path || g.img_path || null}
          categoryLabel={g.unlock_kind}
          onOpen={choisir}
        />
      </div>
    ),
  }),

  famille<Movie>({
    cle: "movies",
    libelle: "Vidéos",
    groupe: "Médias",
    icone: "movie",
    source: "movie_playing_config",
    charger: (d) => api.gameDataMovies(d),
    code: (m) => m.movie_path.split("/").pop() ?? null,
    colonnes: [
      { key: "movie_path", label: "Vidéo", width: "28rem", valeur: (m) => m.movie_path },
      { key: "has_subtitles", label: "Sous-titres", width: "7rem", valeur: (m) => oui(m.has_subtitles) },
      { key: "bgm_name", label: "BGM", width: "10rem", valeur: (m) => m.bgm_name },
      { key: "staffroll_data_name", label: "Générique", width: "14rem", valeur: (m) => m.staffroll_data_name },
      { key: "fade_in", label: "Fondu ⇢", width: "6rem", valeur: (m) => m.fade_in },
      { key: "fade_out", label: "Fondu ⇠", width: "6rem", valeur: (m) => m.fade_out },
    ],
    recherche: (m) => [m.movie_path, m.subtitle_text_path, m.staffroll_data_name],
  }),
  famille<Music>({
    cle: "musics",
    libelle: "Bande-son",
    groupe: "Médias",
    icone: "music_note",
    source: "music_app_config + music_name_text (FR)",
    charger: (d) => api.gameDataMusics(d),
    colonnes: [
      { key: "sort_index", label: "N°", width: "4rem", valeur: (m) => m.sort_index },
      { key: "name", label: "Titre", width: "22rem", valeur: (m) => m.name ?? m.entry_id },
      { key: "app_category", label: "Catégorie", width: "6rem", valeur: (m) => m.app_category },
      { key: "track_no", label: "Piste", width: "5rem", valeur: (m) => m.track_no },
      { key: "variant", label: "Variante", width: "6rem", valeur: (m) => m.variant },
      { key: "volume", label: "Volume", width: "6rem", valeur: (m) => m.volume },
      { key: "has_path", label: "Audio", width: "5rem", valeur: (m) => oui(m.has_path) },
      { key: "music_id", label: "music_id", width: "10rem", valeur: (m) => m.music_id },
    ],
    recherche: (m) => [m.name, m.music_id, m.entry_id],
  }),

  famille<ExpLevel>({
    cle: "exp",
    libelle: "Expérience",
    groupe: "Système",
    icone: "trending_up",
    source: "chara_exp_table_config — cumul calculé depuis le niveau 1",
    charger: (d) => api.gameDataExpTable(d),
    colonnes: [
      { key: "level", label: "Niveau", width: "6rem", valeur: (e) => e.level },
      { key: "need_exp", label: "EXP du palier", width: "9rem", valeur: (e) => e.need_exp },
      { key: "cumulative", label: "EXP cumulée", width: "10rem", valeur: (e) => e.cumulative },
    ],
    recherche: (e) => [String(e.level)],
  }),
] as const;

/** Type effacé : le rendu est générique, chaque famille garde son typage à la déclaration. */
type FamilleAnonyme = Famille<unknown>;
const REGISTRE = FAMILLES as unknown as readonly FamilleAnonyme[];

/** Clé de la famille « calculateur », rendue à part (formulaire, pas tableau). */
const STATS = "stats";

const GROUPES = [...new Set(REGISTRE.map((f) => f.groupe))];

// ─── Utilitaires ─────────────────────────────────────────────────────────────────────────────

function texte(v: Cellule): string {
  if (v === null || v === undefined) return "";
  if (typeof v === "boolean") return v ? "oui" : "";
  if (typeof v === "number") return Number.isInteger(v) ? v.toLocaleString("fr-FR") : String(v);
  // Une sentinelle d'absence ne s'affiche pas telle quelle, cf. `lib/valeurs.ts`.
  return estAbsent(v) ? "" : v;
}

/** Échappement CSV (RFC 4180) — une virgule ou un guillemet dans un nom du jeu casserait le fichier. */
function csvCell(v: Cellule): string {
  const s = v === null || v === undefined ? "" : typeof v === "boolean" ? (v ? "oui" : "non") : String(v);
  return /[",;\n]/.test(s) ? `"${s.replaceAll('"', '""')}"` : s;
}

// ─── Vue ─────────────────────────────────────────────────────────────────────────────────────

export function GameDataView({ onOpenFile }: { onOpenFile?: (path: string) => void }) {
  const settings = useSettings();
  const [cle, setCle] = useState<string>(REGISTRE[0].cle);
  /**
   * Les lignes portent la clé de LEUR famille — jamais un simple tableau.
   *
   * Sans cet appariement, changer de famille produisait un rendu intermédiaire où `cle` était
   * déjà la nouvelle (donc les nouvelles colonnes) alors que `lignes` tenait encore les anciennes
   * : la colonne « Inventaire » d'une boutique lisait `s.items.join` sur un personnage, et
   * l'application se vidait sur `TypeError: Cannot read properties of undefined (reading 'join')`.
   * Reproduit UNIQUEMENT dans le build de production, jamais en développement — l'effet y tourne
   * assez vite pour masquer la frame fautive.
   */
  const [donnees, setDonnees] = useState<{ cle: string; lignes: unknown[] }>({ cle: "", lignes: [] });
  /** Lignes de la famille COURANTE, et rien d'autre : vide tant que le chargement n'a pas rendu. */
  const lignes = donnees.cle === cle ? donnees.lignes : [];
  const setLignes = (r: unknown[]) => setDonnees({ cle, lignes: r });
  const [chargement, setChargement] = useState(true);
  const [erreur, setErreur] = useState<string | null>(null);
  const [filtre, setFiltre] = useState("");
  const [tri, setTri] = useState<{ key: string; dir: "asc" | "desc" } | null>(null);
  const [selection, setSelection] = useState<number | null>(null);
  const [hauteur, setHauteur] = useState(480);
  /** Tableau (toutes les colonnes, triable) ou cartes (mise en page du wiki). */
  const [vue, setVue] = useState<"table" | "cartes">("table");
  /** Cartes affichées d'un coup — 6 101 personnages en DOM figeraient la fenêtre. */
  const [limiteCartes, setLimiteCartes] = useState(120);
  const zone = useRef<HTMLDivElement | null>(null);

  // Cache par (racine du jeu, famille) : revenir sur un onglet déjà vu est instantané, et
  // changer de racine invalide tout — un décodage de `chara_param` coûte plusieurs secondes.
  const cache = useRef(new Map<string, unknown[]>());
  useEffect(() => {
    cache.current.clear();
  }, [settings.gameDir]);

  const famille = REGISTRE.find((f) => f.cle === cle);

  useEffect(() => {
    if (!famille) return;
    setSelection(null);
    setTri(null);
    const cacheKey = `${settings.gameDir}::${famille.cle}`;
    const dejaLa = cache.current.get(cacheKey);
    if (dejaLa) {
      setLignes(dejaLa);
      setChargement(false);
      setErreur(null);
      return;
    }
    let annule = false;
    setChargement(true);
    setErreur(null);
    setLignes([]);
    famille
      .charger(settings.gameDir)
      .then((r) => {
        if (annule) return null;
        cache.current.set(cacheKey, r);
        setLignes(r);
        return null;
      })
      .catch((e) => {
        if (!annule) setErreur(String(e));
      })
      .finally(() => {
        if (!annule) setChargement(false);
      });
    return () => {
      annule = true;
    };
  }, [famille, settings.gameDir]);

  // Hauteur réelle de la grille : sans mesure, `DataGrid` retomberait sur ses 384 px par défaut
  // et laisserait la moitié de la fenêtre vide.
  useEffect(() => {
    const el = zone.current;
    if (!el) return;
    const ro = new ResizeObserver(([entry]) => setHauteur(Math.max(160, entry.contentRect.height)));
    ro.observe(el);
    return () => ro.disconnect();
  }, [cle]);

  const colonnes = famille?.colonnes ?? [];
  const gridColumns = useMemo<DataGridColumn[]>(
    () => colonnes.map((c) => ({ key: c.key, label: c.label, width: c.width ?? "8rem", title: c.title })),
    [colonnes],
  );

  // Filtre : champs déclarés par la famille, à défaut toutes ses colonnes.
  const indices = useMemo(() => {
    const q = filtre.trim().toLowerCase();
    const tous = lignes.map((_, i) => i);
    if (!q || !famille) return tous;
    const champs = famille.recherche ?? ((l: unknown) => colonnes.map((c) => texte(c.valeur(l))));
    return tous.filter((i) =>
      champs(lignes[i]).some((v) => (v ?? "").toString().toLowerCase().includes(q)),
    );
  }, [lignes, filtre, famille, colonnes]);

  const ordre = useMemo(() => {
    if (!tri || !famille) return indices;
    const col = colonnes.find((c) => c.key === tri.key);
    if (!col) return indices;
    const sens = tri.dir === "asc" ? 1 : -1;
    return [...indices].sort((a, b) => {
      const va = col.valeur(lignes[a]);
      const vb = col.valeur(lignes[b]);
      // Les vides sont toujours relégués en fin de liste, quel que soit le sens : un tri
      // décroissant qui commence par 40 cellules vides ne montre rien.
      const va_vide = va === null || va === undefined || va === "";
      const vb_vide = vb === null || vb === undefined || vb === "";
      if (va_vide || vb_vide) return va_vide && vb_vide ? 0 : va_vide ? 1 : -1;
      if (typeof va === "number" && typeof vb === "number") return (va - vb) * sens;
      return texte(va).localeCompare(texte(vb), "fr", { numeric: true }) * sens;
    });
  }, [indices, tri, colonnes, lignes, famille]);

  function basculerTri(key: string) {
    setTri((t) => (t?.key === key ? (t.dir === "asc" ? { key, dir: "desc" } : null) : { key, dir: "asc" }));
  }

  async function exporter(format: "csv" | "json") {
    if (!famille) return;
    const nom = `${famille.cle}-${ordre.length}.${format}`;
    const dest = await save({ defaultPath: nom, filters: [{ name: format.toUpperCase(), extensions: [format] }] });
    if (!dest) return;
    const contenu =
      format === "csv"
        ? [
            colonnes.map((c) => csvCell(c.label)).join(","),
            ...ordre.map((i) => colonnes.map((c) => csvCell(c.valeur(lignes[i]))).join(",")),
          ].join("\n")
        : JSON.stringify(
            ordre.map((i) => Object.fromEntries(colonnes.map((c) => [c.key, c.valeur(lignes[i])]))),
            null,
            2,
          );
    try {
      await api.writeTextFile(dest, contenu);
      toast.success(`${ordre.length.toLocaleString("fr-FR")} ligne(s) → ${dest}`);
    } catch (e) {
      toast.error(String(e));
    }
  }

  const ligneSelectionnee = selection !== null ? lignes[selection] : null;
  const codeSelectionne =
    ligneSelectionnee && famille?.code ? famille.code(ligneSelectionnee) : null;

  return (
    <div className="flex h-full min-h-0">
      {/* Catégories — 24 familles ne tiennent pas dans une barre d'onglets lisible. */}
      <ScrollArea className="w-[188px] min-w-[188px] border-r border-app-line">
        <div className="space-y-3 p-2">
          {GROUPES.map((groupe) => (
            <div key={groupe} className="space-y-0.5">
              <div className="px-1.5 pb-1 text-tiny font-semibold uppercase tracking-wide text-ink-faint">
                {groupe}
              </div>
              {REGISTRE.filter((f) => f.groupe === groupe).map((f) => (
                <button
                  key={f.cle}
                  type="button"
                  onClick={() => setCle(f.cle)}
                  title={f.source}
                  className={cn(
                    "flex w-full items-center gap-2 rounded-md px-1.5 py-1 text-sm transition-colors",
                    cle === f.cle
                      ? "bg-accent text-white"
                      : "text-ink-dull hover:bg-app-selected/20 hover:text-ink",
                  )}
                >
                  <Icon name={f.icone} size={16} />
                  <span className="flex-1 truncate text-left">{f.libelle}</span>
                </button>
              ))}
            </div>
          ))}
          <div className="space-y-0.5">
            <div className="px-1.5 pb-1 text-tiny font-semibold uppercase tracking-wide text-ink-faint">
              Calcul
            </div>
            <button
              type="button"
              onClick={() => setCle(STATS)}
              title="Stats d'un personnage à un niveau et une rareté donnés (tables de croissance embarquées)"
              className={cn(
                "flex w-full items-center gap-2 rounded-md px-1.5 py-1 text-sm transition-colors",
                cle === STATS ? "bg-accent text-white" : "text-ink-dull hover:bg-app-selected/20 hover:text-ink",
              )}
            >
              <Icon name="calculate" size={16} />
              <span className="flex-1 truncate text-left">Calculateur de stats</span>
            </button>
          </div>
        </div>
      </ScrollArea>

      {cle === STATS ? (
        <div className="min-h-0 flex-1 p-2">
          <StatCalculator />
        </div>
      ) : (
        <div className="flex min-h-0 flex-1 flex-col gap-2 p-2">
          <div className="flex flex-wrap items-center gap-2">
            <Input
              placeholder={`Filtrer ${famille?.libelle.toLowerCase() ?? ""}…`}
              value={filtre}
              onChange={(e) => setFiltre(e.target.value)}
              className="max-w-xs"
            />
            <span className="type-label-small text-on-surface-variant">
              {chargement
                ? "décodage du VFS…"
                : `${ordre.length.toLocaleString("fr-FR")} / ${lignes.length.toLocaleString("fr-FR")}`}
            </span>
            {tri && (
              <Badge variant="outline" className="gap-1">
                tri {colonnes.find((c) => c.key === tri.key)?.label} {tri.dir === "asc" ? "↑" : "↓"}
              </Badge>
            )}
            <div className="ml-auto flex items-center gap-1">
              {famille?.carte && (
                <div className="mr-1 flex overflow-hidden rounded-md border border-app-line">
                  <button
                    type="button"
                    onClick={() => setVue("table")}
                    title="Tableau : toutes les colonnes, triables"
                    className={cn("px-2 py-1", vue === "table" ? "bg-accent text-white" : "text-ink-dull")}
                  >
                    <Icon name="table_rows" size={14} />
                  </button>
                  <button
                    type="button"
                    onClick={() => setVue("cartes")}
                    title="Cartes : la mise en page du wiki, images du VFS"
                    className={cn("px-2 py-1", vue === "cartes" ? "bg-accent text-white" : "text-ink-dull")}
                  >
                    <Icon name="grid_view" size={14} />
                  </button>
                </div>
              )}
              <Button size="sm" variant="secondary" disabled={!ordre.length} onClick={() => exporter("csv")}>
                <Icon name="download" size={14} /> CSV
              </Button>
              <Button size="sm" variant="secondary" disabled={!ordre.length} onClick={() => exporter("json")}>
                <Icon name="download" size={14} /> JSON
              </Button>
            </div>
          </div>

          <p className="type-label-small text-ink-faint" title={famille?.source}>
            {famille?.source}
          </p>

          {erreur && (
            <Alert variant="destructive">
              <AlertTitle>Échec du décodage</AlertTitle>
              <AlertDescription>{erreur}</AlertDescription>
            </Alert>
          )}

          <div className="flex min-h-0 flex-1 gap-2">
            {famille?.carte && vue === "cartes" ? (
              <ScrollArea className="min-h-0 min-w-0 flex-1 rounded-2xl border border-app-line bg-app-dark-box">
                <div className="grid grid-cols-[repeat(auto-fill,minmax(190px,1fr))] gap-2 p-2">
                  {ordre.slice(0, limiteCartes).map((i) => (
                    <div key={i}>{famille.carte?.(lignes[i], selection === i, () => setSelection(i))}</div>
                  ))}
                </div>
                {ordre.length > limiteCartes && (
                  <div className="flex justify-center p-3">
                    <Button size="sm" variant="secondary" onClick={() => setLimiteCartes((n) => n + 240)}>
                      Afficher 240 de plus ({(ordre.length - limiteCartes).toLocaleString("fr-FR")} restantes)
                    </Button>
                  </div>
                )}
                {ordre.length === 0 && (
                  <p className="p-4 type-body-small text-on-surface-variant">
                    {chargement ? "décodage du VFS…" : "Aucun résultat ne correspond."}
                  </p>
                )}
              </ScrollArea>
            ) : (
            <div ref={zone} className="min-h-0 min-w-0 flex-1">
              <DataGrid
                columns={gridColumns}
                rowCount={lignes.length}
                order={ordre}
                sort={tri}
                onSortChange={basculerTri}
                height={hauteur}
                rowHeaderLabel="#"
                rowHeaderWidth="4rem"
                rowHeader={(row, i) => (
                  <button
                    type="button"
                    onClick={() => setSelection(row)}
                    className={cn(
                      "w-full px-2 text-left tabular-nums",
                      selection === row ? "text-accent" : "text-ink-faint",
                    )}
                  >
                    {i + 1}
                  </button>
                )}
                cell={(row, column) => {
                  const col = colonnes.find((c) => c.key === column.key);
                  const v = col ? col.valeur(lignes[row]) : null;
                  const s = texte(v);
                  return (
                    <button
                      type="button"
                      onClick={() => setSelection(row)}
                      title={s}
                      className={cn(
                        "block w-full truncate px-2 text-left",
                        selection === row && "bg-accent/15",
                      )}
                    >
                      {s}
                    </button>
                  );
                }}
                empty={
                  chargement ? "décodage du VFS…" : filtre ? "Aucun résultat ne correspond." : "Aucune donnée."
                }
              />
            </div>
            )}

            {/* Détail — toujours utile : la fiche clé/valeur complète, et l'éditeur de propriétés
             * quand l'entité porte un code interne (ses fichiers, ses cfg.bin, ses fonctions). */}
            <div className="flex h-full w-[340px] min-w-[340px] flex-col gap-2">
              {ligneSelectionnee ? (
                <>
                  <ScrollArea className="max-h-[45%] rounded-lg border border-app-line bg-app-box/60">
                    <dl className="divide-y divide-app-line text-xs">
                      {colonnes.map((c) => {
                        const s = texte(c.valeur(ligneSelectionnee));
                        return (
                          <div key={c.key} className="flex gap-2 px-3 py-1.5">
                            <dt className="w-28 shrink-0 text-ink-faint">{c.label}</dt>
                            <dd className="min-w-0 flex-1 break-words text-ink">{s || "—"}</dd>
                          </div>
                        );
                      })}
                    </dl>
                  </ScrollArea>
                  {codeSelectionne ? (
                    <div className="min-h-0 flex-1 overflow-hidden rounded-lg border border-app-line bg-app-box/60">
                      <PropertyEditor code={codeSelectionne} className="h-full p-3" onOpenFile={onOpenFile} />
                    </div>
                  ) : (
                    <div className="flex flex-1 items-center justify-center rounded-lg border border-dashed border-app-line px-4 text-center text-xs text-ink-faint">
                      Cette entrée n'a pas de code interne : aucun asset ne s'y rattache dans le VFS.
                    </div>
                  )}
                </>
              ) : (
                <div className="flex h-full items-center justify-center rounded-lg border border-dashed border-app-line px-4 text-center text-xs text-ink-faint">
                  {vue === "cartes" && famille?.carte
                    ? "Choisissez une carte pour voir sa fiche complète, et l'éditeur de propriétés (fichiers, données, moteur) si elle porte un code interne."
                    : "Sélectionnez une ligne pour voir sa fiche complète, et l'éditeur de propriétés (fichiers, données, moteur) si elle porte un code interne."}
                </div>
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
