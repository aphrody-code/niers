//! Hôte de menu Lua — `MenuState` + `install_menu_host` + `run_menu`.
//!
//! Reproduit le comportement de `GameLuaHost.cs` (iecode) : enregistre les
//! fonctions hôtes comme globals Lua, chacune mutant un [`MenuState`] partagé.
//!
//! ## Dispatch `funcLuaMenuCommand`
//!
//! Le jeu appelle `funcLuaMenuCommand(cmdId, layerId, …args)` où `cmdId` est
//! un hash 32 bits d'un nom de commande interne du moteur C++. Ces hashes sont
//! reversés depuis `nie.exe` et documentés dans `re/lua/funclua-cmdids.json`
//! (iecode) ; les valeurs confirmées sont codées en dur ici.
//!
//! Pour les `cmdId` non encore reversés, l'appel est journalisé et ignoré
//! (retour `0.0`) — le script continue sans crash, conformément au comportement
//! du stub `DefaultLuaHost`.
//!
//! ## Hashes connus (source : `GameLuaHostTests.cs`)
//!
//! | Hash         | Opération          |
//! |:-------------|:-------------------|
//! | `0x2A64B198` | `SetObjectVisible` |
//! | `0xE15FD945` | `SetSprite`        |
//! | `0x4096E67E` | `SetText`          |

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use mlua::{Function, Lua, MultiValue, Table, Value, Variadic};

use crate::LuaError;

// ---------------------------------------------------------------------------
// Modèle MenuState (miroir de MenuState.cs / MenuLayerState.cs / MenuObjectState.cs)
// ---------------------------------------------------------------------------

/// Entrée de liste (sous-item) d'un objet-liste de menu.
///
/// Modèle des **sous-items virtuels** d'une liste : un objet-liste (ex. la barre d'onglets
/// `200360024` du main_menu, ou une liste d'options de soccer_top) n'a pas un objet par item
/// dans l'objbin — il porte N items, chacun décrit par des valeurs PAR INDEX que le moteur
/// stocke en tableaux parallèles (`[sub+0x70C-0x250]/-0x128/+0`, cf. handler `0x140CB0240`).
///
/// Peuplé par :
/// - `SetListItemValues(layerId, objId, <table>)` — une colonne (`values[0]` par item) ;
/// - `SetListItemValuesMulti(layerId, objId, <table>×N)` — N colonnes parallèles (`values[0..N]`) ;
/// - `SetItemParam(objId, itemIndex, key, value)` — un paramètre keyé (`params[key] = value`).
#[derive(Debug, Clone, Default)]
pub struct MenuListItem {
    /// Valeurs positionnelles par colonne, dans l'ordre des tables passées aux commandes de
    /// liste (hash ou entier selon la colonne ; le moteur les stocke comme dwords par item).
    pub values: Vec<i32>,
    /// Paramètres keyés (`SetItemParam` : hash de clé → valeur).
    pub params: BTreeMap<u32, i32>,
}

/// État d'un objet (widget) dans un layer de menu, muté par `funcLuaMenuCommand`.
///
/// Les champs `Option<_>` ne sont renseignés que lorsqu'une commande les a
/// touchés — un consommateur en aval n'applique que les mutations réelles.
#[derive(Debug, Clone)]
pub struct MenuObjectState {
    /// Hash CRC32 de l'objet (clé dans le layer).
    pub id: u32,
    /// Nom résolu de l'objet (ou `None` si inconnu).
    pub name: Option<String>,
    /// Visibilité (`SetObjectVisible` / `SetObjectFlag`). Défaut : `true`.
    pub visible: bool,
    /// Actif / interactif (`SetObjectActive` / `SetButtonEnabled`). Défaut : `true`.
    pub active: bool,
    /// Hash de texture du sprite (`SetSprite` / `SetIconTexture`). Pour `SetIconSprite` c'est le
    /// hash du CHEMIN g4tx (`GetTexturePath…()`), à apparier avec [`Self::sprite_region_hash`].
    pub sprite_texture_hash: Option<u32>,
    /// Hash du NOM de région/texture dans l'atlas (arg `textureNameCrc` de `SetIconSprite`). Avec
    /// `sprite_texture_hash` (chemin), forme la paire (chemin g4tx, région) résoluble en texture
    /// réelle via le dico CRC32 du corpus Lua décompilé → render-from-runtime.
    pub sprite_region_hash: Option<u32>,
    /// Index de frame dans l'atlas (`SetSprite`).
    pub frame: Option<i32>,
    /// Teinte couleur, hash de palette (`SetSprite` / `SetColorTint`).
    pub color_hash: Option<u32>,
    /// Couleur RGBA explicite, packée `0xRRGGBBAA` (`SetObjectColorRGBA`).
    pub color_rgba: Option<u32>,
    /// Texte affiché (`SetText` / `SetTextMulti`). Hash stocké en hex si non résolu.
    pub text: Option<String>,
    /// Valeur numérique (`SetNumericDisplay` / `SetObjectNum`).
    pub number: Option<i32>,
    /// Valeur entière générique du moteur, champ `[obj+0x140]` (`SetObjectValue` `0x988B5B82`).
    /// Distincte de `number` (`[obj+0x148]`, compte d'items) : ici une valeur/hash arbitraire.
    pub value: Option<i32>,
    /// Index de défilement (`SetScrollIndex`).
    pub scroll_index: Option<i32>,
    /// Index SÉLECTIONNÉ / curseur dans la liste (`SetSelectedIndex` `0x6A06BC75`, handler
    /// `0x140CE6B20` reversé : écrit `word [obj+0x154]`, clampé au compte `[obj+0x150]`). Distinct
    /// de `scroll_index` (position de scroll) : ici l'item mis en surbrillance.
    pub selected_index: Option<i32>,
    /// Échelle (`SetObjectScale`).
    pub scale: Option<f32>,
    /// Valeur de badge (`SetBadge`).
    pub badge: Option<i32>,
    /// Barre de progression (`SetProgressBar`).
    pub progress: Option<f32>,
    /// Sous-items (entrées de liste) par index, peuplés par les commandes de liste
    /// (`SetListItemValues` / `SetListItemValuesMulti` / `SetItemParam`). Vide pour un objet
    /// non-liste. Clé = index d'item (0-based), valeur = [`MenuListItem`].
    pub sub_items: BTreeMap<i32, MenuListItem>,
    /// Visibilité **par instance**, quand la commande en désigne une.
    ///
    /// Un écran de menu réplique un même objet-gabarit une fois par item : l'éditeur d'avatar
    /// porte 51 exemplaires de `avatar01_64_recipe_item_type01`, 32 de `avatar01_13_gauge_bar`.
    /// Comme l'état est indexé par `crc32(nom)`, tous ces exemplaires partageaient un unique
    /// booléen — masquer le troisième les masquait tous les cinquante-et-un, ce que la mesure
    /// montrait en bloc (51 → 0 visibles, 16 → 16). Les commandes portent pourtant un `index`
    /// (2ᵉ argument de `SetObjectVisible`, `SetPartVisible`), jusqu'ici ignoré. On le retient ici ;
    /// [`Self::visible`] reste le défaut, pour les instances qu'aucune commande ne nomme.
    pub visible_par_index: BTreeMap<i32, bool>,
    /// Visibilité d'une part/enfant adressée par son hash (handler Kizuna `0x140CCC930`).
    /// Distincte de `visible_par_index` : le natif résout ici un sous-élément par identifiant.
    pub part_visible: BTreeMap<u32, bool>,
    /// Couleur flottante RGBA d'une part/enfant adressée par son hash (handler Kizuna
    /// `0x140CDF9F0`). Les quatre canaux restent en `f32`, comme le vecteur transmis au moteur.
    pub part_color_rgba: BTreeMap<u32, [f32; 4]>,
    /// Arguments numériques bruts des mutations Kizuna dont la structure native reste à préciser.
    pub part_texture_args: BTreeMap<u32, Vec<u32>>,
    pub part_param_args: BTreeMap<u32, Vec<u32>>,
    pub part_flag_args: BTreeMap<u32, Vec<u32>>,
}

impl MenuObjectState {
    fn new(id: u32) -> Self {
        Self {
            id,
            name: None,
            visible: true,
            active: true,
            sprite_texture_hash: None,
            sprite_region_hash: None,
            frame: None,
            color_hash: None,
            color_rgba: None,
            text: None,
            number: None,
            value: None,
            scroll_index: None,
            selected_index: None,
            scale: None,
            badge: None,
            progress: None,
            sub_items: BTreeMap::new(),
            visible_par_index: BTreeMap::new(),
            part_visible: BTreeMap::new(),
            part_color_rgba: BTreeMap::new(),
            part_texture_args: BTreeMap::new(),
            part_param_args: BTreeMap::new(),
            part_flag_args: BTreeMap::new(),
        }
    }

    /// Récupère ou crée l'entrée de liste (sous-item) à l'index `idx`.
    pub fn sub_item(&mut self, idx: i32) -> &mut MenuListItem {
        self.sub_items.entry(idx).or_default()
    }
}

/// État d'un layer de menu (fenêtre/écran logique). Contient ses objets mutés.
#[derive(Debug, Clone)]
pub struct MenuLayerState {
    /// Hash CRC32 du layer.
    pub id: u32,
    /// Nom résolu du layer (ou `None` si inconnu).
    pub name: Option<String>,
    /// Visibilité du layer (`SetLayerVisible`). Défaut : `true`.
    pub visible: bool,
    /// Activé (`SetLayerEnabled`). Défaut : `true`.
    pub enabled: bool,
    /// Index focus courant (`SetFocus`).
    pub focus: Option<i32>,
    /// Item courant sélectionné (`SetCurrentItem`).
    pub current_item: Option<i32>,
    /// Objets du layer, par hash.
    pub objects: BTreeMap<u32, MenuObjectState>,
}

impl MenuLayerState {
    fn new(id: u32) -> Self {
        Self {
            id,
            name: None,
            visible: true,
            enabled: true,
            focus: None,
            current_item: None,
            objects: BTreeMap::new(),
        }
    }

    /// Récupère ou crée l'état d'un objet par son hash.
    pub fn obj(&mut self, object_id: u32) -> &mut MenuObjectState {
        self.objects
            .entry(object_id)
            .or_insert_with(|| MenuObjectState::new(object_id))
    }
}

/// État de menu reconstruit en exécutant la logique Lua du jeu.
///
/// Équivalent Rust de l'état que `nie.exe` construit en mémoire — consommé
/// ensuite par le rendu (azalee) pour afficher le menu interactif piloté par
/// les vrais scripts.
#[derive(Debug, Clone, Default)]
pub struct MenuState {
    /// Layers par hash, dans l'ordre d'insertion (BTreeMap = ordre déterministe).
    pub layers: BTreeMap<u32, MenuLayerState>,
    /// Attributs de scène par objet/layer (clé = hash CRC32 ; valeur = entier).
    ///
    /// Lu par `GetObjectAttr` (`0x4612788B`) — notamment le **nombre d'item-buttons** d'un
    /// layer-list (ce que la fonction de script `GetItemButtonNum` interroge). C'est la couche
    /// de données scène que le moteur C++ fournit normalement (et que le menu seul n'a pas) :
    /// l'appelant la renseigne depuis les vraies données d'écran (slots `AttachLocator` des
    /// objbin) AVANT de piloter, sinon `GetObjectAttr` renvoie 0 et le menu reste vide.
    pub object_attr: BTreeMap<u32, i32>,
    /// Table des libellés localisés fournie par `menu_text.cfg.bin` au runtime.
    pub text_by_id: BTreeMap<u32, String>,
    /// Conditions de jeu visibles par `funcLuaCommand(IsConditionActive)`.
    pub condition_flags: BTreeMap<u32, bool>,
    /// Comptes de listes du jeu visibles par `funcLuaCommand(GetListCount)`.
    /// La seconde valeur indique si l'identifiant de liste a été résolu.
    pub list_counts: BTreeMap<u32, (i32, bool)>,
    /// IDs de ressources disponibles pour le prédicat général du moteur.
    pub resource_ids: BTreeSet<u32>,
    /// Valeur entière écrite par `funcLuaCommand(0x449E298B, value)`.
    ///
    /// Le handler de `nie.exe` stocke cette valeur dans le slot moteur global
    /// `context+0x2728` et renvoie `true` dès qu'un paramètre est présent. Le
    /// consommateur natif de ce slot n'appartient pas à l'état de menu, mais
    /// conserver la dernière écriture permet aux couches supérieures de
    /// l'injecter ou de l'inspecter sans perdre l'effet observable du script.
    pub engine_int_2728: Option<i32>,
    /// Visibilité par groupe (`SetGroupVisible`).
    pub groups: BTreeMap<u32, bool>,
    /// Journal des appels `funcLuaMenuCommand` non reconnus :
    /// `(cmdId, layerId, args_repr)` pour la découverte de nouveaux hashes.
    pub unknown_cmd_log: Vec<(u32, u32, String)>,
    /// Journal séparé des appels `funcLuaCommand` généraux non reconnus.
    /// Ils ne doivent pas être confondus avec les commandes de rendu du menu.
    pub unknown_general_cmd_log: Vec<(u32, u32, String)>,
    /// Journal de TOUS les appels connus (nom, layerId) — télémétrie légère.
    pub known_cmd_log: Vec<(String, u32)>,
    /// Layer « courant » : cible par défaut des commandes d'objet sans layerId explicite.
    /// Mis à jour par `SetLayerActive(layerId, true)`. Défaut 0.
    pub current_layer: u32,
}

impl MenuState {
    /// Récupère ou crée l'état d'un layer par son hash.
    pub fn layer(&mut self, layer_id: u32) -> &mut MenuLayerState {
        self.layers
            .entry(layer_id)
            .or_insert_with(|| MenuLayerState::new(layer_id))
    }

    /// Renseigne un attribut de scène (lu par `GetObjectAttr`), typiquement le nombre
    /// d'item-buttons d'un layer-list. À appeler avant le pilotage du menu.
    pub fn set_object_attr(&mut self, id: u32, value: i32) {
        self.object_attr.insert(id, value);
    }

    /// Renseigne un libellé localisé lu depuis le VFS.
    pub fn set_text(&mut self, id: u32, text: String) {
        self.text_by_id.insert(id, text);
    }

    /// Injecte une condition moteur pour les scripts exécutés hors du jeu complet.
    pub fn set_condition(&mut self, id: u32, active: bool) {
        self.condition_flags.insert(id, active);
    }

    /// Injecte le résultat d'une requête de liste (`count`, `resolved`).
    pub fn set_list_count(&mut self, id: u32, count: i32, resolved: bool) {
        self.list_counts.insert(id, (count, resolved));
    }

    /// Déclare une ressource comme résolue par le runtime moteur.
    pub fn set_resource_available(&mut self, id: u32, available: bool) {
        if available {
            self.resource_ids.insert(id);
        } else {
            self.resource_ids.remove(&id);
        }
    }

    /// Injecte directement la valeur du slot entier moteur observé par le
    /// handler `0x449E298B`.
    pub fn set_engine_int_2728(&mut self, value: i32) {
        self.engine_int_2728 = Some(value);
    }
}

// ---------------------------------------------------------------------------
// Hashes de commandes reversés (source : GameLuaHostTests.cs)
// ---------------------------------------------------------------------------

// cmdId reversés du dispatch `funcLuaMenuCommand` de nie.exe (handler 0x140C91B30 →
// dispatcher 0x140C8CC00 → table descripteurs 0x141BDFD90, 1109 entrées ; cf.
// `data/re/funclua-cmdids.json` + DESIGN.md §13). Layouts d'args **vérifiés** sur le vrai
// désassemblage + les appels réels émis par `title_menu_2` (le 2ᵉ arg N'EST PAS universellement
// le layerId : c'est l'objId pour les commandes d'objet, le layerId pour les commandes de layer).
// — setters (mutent l'état rendu par le menu) —
const CMD_SET_OBJECT_VISIBLE: u32 = 0x2A64_B198; // (objId, index, visible, [layerId])
const CMD_GENERAL_GET_TEXT: u32 = 0xF2C1_3584; // funcLuaCommand(textId) -> string
// Build `nie.exe` présent sur le VFS local : le handler 0x140CA60C0 est appelé par ce hash
// (lecture d'un ou deux IDs puis push d'une chaîne). L'ancien hash reste accepté pour les
// corpus issus du build précédent.
const CMD_GENERAL_GET_TEXT_CURRENT: u32 = 0xF2D9_F802;
const CMD_GENERAL_IS_CONDITION_ACTIVE: u32 = 0x0196_FA01; // () -> bool
const CMD_GENERAL_GET_LIST_COUNT: u32 = 0x77E4_6CA8; // (listId, ...) -> (count, resolved)
// Handler local 0x140CA6690 : recherche dans le registre moteur puis `setne` + push bool.
const CMD_GENERAL_RESOURCE_AVAILABLE: u32 = 0xAFB0_FE77; // (resourceId) -> bool
// Handler courant 0x140C90300 : configure une ressource/UI et renvoie AL=1 si un paramètre est
// présent (AL=0 pour l'arité vide). L'effet graphique interne reste hors MenuState.
const CMD_GENERAL_APPLY_UI_EFFECT: u32 = 0x724F_633E;
// Handler courant 0x140CA35F0 : lit un identifiant et pose une valeur d'état moteur, puis
// pousse true ; l'arité vide est rejetée (false). La table d'état C++ n'est pas dans MenuState.
const CMD_GENERAL_APPLY_STATE: u32 = 0x1319_3DE3;
// Handler courant 0x140BFC910 : sans argument renvoie AL=0 ; sinon convertit le premier
// paramètre numérique, l'écrit dans `[0x1421F6258]+0x2728`, puis renvoie AL=1. Le consommateur
// natif du slot est hors du MenuState, mais la dernière valeur reste observable ici.
const CMD_GENERAL_SET_GLOBAL_INT: u32 = 0x449E_298B;
// Handlers généraux Kizuna du build courant :
// 0x140CA3950 lit un identifiant et renvoie le booléen produit par le registre moteur ;
// 0x140C6EEE0 résout un état/index et renvoie un booléen ;
// 0x140C60370 renvoie le couple (succès, valeur) après consultation du registre ;
// 0x140C42B90 applique l'état courant et renvoie true sur tous ses chemins.
const CMD_GENERAL_QUERY_BOOL_A: u32 = 0x0FC9_E076;
const CMD_GENERAL_QUERY_BOOL_B: u32 = 0x2A2B_AD8F;
const CMD_GENERAL_QUERY_STATE: u32 = 0x36AA_3F1B;
const CMD_GENERAL_APPLY_CURRENT_STATE: u32 = 0xD5BE_A3D4;
// Ces handlers sont atteints après l'activation du mode Kizuna :
// AD0AA37E -> bool de configuration courante, B8F922E7 -> chaîne courante,
// DCBE6334 -> entier d'état courant. Les valeurs de save/configuration sont injectables
// ultérieurement ; les types et défauts neutres du moteur sont conservés ici.
const CMD_GENERAL_QUERY_CONFIG_BOOL: u32 = 0xAD0A_A37E;
const CMD_GENERAL_GET_CURRENT_TEXT: u32 = 0xB8F9_22E7;
const CMD_GENERAL_GET_CURRENT_INT: u32 = 0xDCBE_6334;
// Handler 0x140CBFCF0 (chaîné avec 0x140CBFD10) : remet le slot moteur global
// `0x142294600` à zéro puis renvoie AL=1. `kizuna_town_mainmenu` l'appelle sans
// paramètre pendant `OnInit`; le slot est hors MenuState, mais le retour doit
// être fidèle pour que la branche Lua de démarrage soit prise.
const CMD_KIZUNA_TOWN_RESET: u32 = 0xA143_C0FC;
// Handlers Kizuna extraits de la table du build courant :
// 0x140CCC930 résout (objet, part) puis écrit le flag visible de la part ;
// 0x140CDF9F0 résout (objet, index, part) puis écrit le vecteur couleur RGBA.
const CMD_KIZUNA_SET_PART_VISIBLE: u32 = 0x2E9B_F339;
const CMD_KIZUNA_SET_PART_COLOR: u32 = 0xDB1F_D4EB;
// Appels UI déclenchés par les nouvelles branches de Kizuna après le chargement de l'état :
// les handlers ont une garde d'arité puis retournent `true`; leur mutation est dans le manager
// natif hors MenuState, mais le protocole de retour doit rester fidèle.
const CMD_KIZUNA_APPLY_PART_FLAGS: u32 = 0x510B_8B99;
const CMD_KIZUNA_SET_PART_PARAM: u32 = 0x894E_7710;
const CMD_KIZUNA_SET_PART_TEXTURE: u32 = 0xF6F4_D7E9;
const CMD_SET_SPRITE: u32 = 0xE15F_D945; // (objId, index, cellId, frame, color, [layerId])
const CMD_SET_TEXT: u32 = 0x4096_E67E; // (objId, index, textHash|string, …, [layerId])
const CMD_SET_COLOR: u32 = 0x1401_6F35; // (objId, part, colorId, [cellIndex])
const CMD_SET_LAYER_ACTIVE: u32 = 0x5CE7_F1AE; // (layerId, active)
const CMD_SET_PART_VISIBLE: u32 = 0x69C9_F55C; // (objId, index, partId, enabled, [layerId])
// — getters (lisent l'état moteur ; on renvoie un défaut sûr, à affiner) —
const CMD_GET_NODE_FLOAT: u32 = 0x45E9_070A; // (objId, index, nodeKey, [layerId]) -> float
const CMD_GET_OBJECT_ATTR: u32 = 0x4612_788B; // (objId, [layerId]) -> int
const CMD_GET_SPRITE_CELL_INDEX: u32 = 0x509F_BBC2; // (objId, cellId, [subIndex]) -> int
const CMD_GET_GLOBAL_STATE_B: u32 = 0x9580_2985; // () -> bool
const CMD_GET_GLOBAL_STATE_A: u32 = 0xA4B1_D1BC; // () -> bool
const CMD_GET_OBJECT_ACTIVE: u32 = 0xB641_D667; // (objId, [index]) -> bool
// () -> bool=TRUE ; handler 0x140CBF150 reversé (désassemblage nie.exe ce cycle) : lit un octet de
// config global `[ [0x1421107A8]+0x69C8 ]+0x2CAA1E`, l'applique via 0x1405CF860(ctx, byte), puis
// renvoie **AL=1 inconditionnellement**. Appelé sans argument dans `OnInit` de main_menu. niers ne
// peut pas répliquer la mutation d'état moteur (0x1405CF860), MAIS la valeur de retour CORRECTE est
// **1** (le défaut getter `0.0` serait FAUX si le script teste le retour). Reversé ce cycle (le 1ᵉʳ
// des cmdId « semantics TBD » résolu par désassemblage local).
const CMD_APPLY_GLOBAL_CONFIG_TRUE: u32 = 0x65E8_25B1; // () -> bool (toujours true)
// Même FAMILLE « apply → return true » (no-arg, reversés ce cycle par désassemblage nie.exe) :
// 0xC3135B00 (handler 0x140CEEE80) : `call 0x1410F1B10`(query bool) → `apply 0x1405CF730(ctx,bool)`
//   → `mov al,1; ret` INCONDITIONNEL. Utilisé par `shop`.
const CMD_APPLY_QUERY_TRUE: u32 = 0xC313_5B00; // () -> bool (toujours true)
// 0xB9FFF3C9 (handler 0x140C96A50) : branche sur le flag global `[0x141D842F0]` ; **cas par défaut
//   (flag==0, = l'état frais de menu de niers)** : `apply 0x1405CF9E0(ctx,0)` → `mov al,1; ret`.
//   (La branche flag!=0 appelle 0x1416935D0 — non atteinte hors save chargée.) Utilisé par `title02`.
const CMD_APPLY_DEFAULT_TRUE: u32 = 0xB9FF_F3C9; // () -> bool (true en état par défaut)
// 0x74578BF4 (handler 0x140CEECE0, trouvé via la table de dispatch extraite — cf.
//   scripts/extract_funclua_table.py) : lit arg0, pose le FLAG GLOBAL `[0x1421AE9C3] = (arg0 != 0)`,
//   `mov al,1; ret` (al=0 seulement si AUCUN arg). Effet = drapeau moteur global, HORS layout ;
//   retour CONSTANT 1 dès qu'un arg est passé (toujours le cas : `shop` l'appelle avec un bool).
//   Même forme « set engine flag → return 1 » que la famille ci-dessus, mais à 1 arg.
const CMD_SET_GLOBAL_FLAG_TRUE: u32 = 0x7457_8BF4; // (bool) -> 1 (pose un flag moteur global)
// (objId, hash, _, count) -> bool ; handler 0x140CD8E30 reversé : lit 4 args, appelle le manager
// d'items 0x1410C18D0 (via 0x140CF5B60), renvoie al=1 (0 si <4 args). **Le MÊME manager 0x140CF5B60
// est lu par GetObjectAttr (handler 0x140CF4F90)** → le `count` (arg3) ENREGISTRÉ ici est CELUI que
// GetItemButtonNum relit. Donc le nombre d'items vient du SCRIPT, pas (seulement) du save-state.
// Confirmé : args réels `(2250456639,…,8)` == golden `object_attr[2250456639]=8`. Modèle niers :
// `object_attr[objId]=count` → débloque GetItemButtonNum depuis les données du script.
const CMD_REGISTER_ITEM_LIST_COUNT: u32 = 0x16C1_C4C0; // (objId, hash, _, count) -> bool
// (objId, index, [bool], [bool]) -> bool=true ; handler 0x140CE6B20 reversé : `FindObject(mgr, objId)`
// (0x14051B5D0) puis écrit `word [obj+0x154] = index` (clampé/bouclé au compte `[obj+0x150]`), pose le
// flag dirty `[obj+0x161]=1` si changé, renvoie al=1. = sélection/curseur d'item de liste. Présent
// dans title02 ET shop. Modèle niers : `obj.selected_index = index`.
const CMD_SET_SELECTED_INDEX: u32 = 0x6A06_BC75; // (objId, index, ...) -> bool
// (objId, itemIndex, bool) -> bool=true ; handlers 0x140CC69F0 / 0x140CC7670 reversés : find-object
// (mgr 0x140CF5B60 + 0x14051B5D0) puis posent un FLAG par-item à `itemIndex`, renvoient al=1 (al=0 si
// <3 args). Les DEUX commandes les plus fréquentes du shop (×26 / ×25 = un appel par item de liste).
// Champ par-item précis non modélisé → on enregistre le sous-item (motif liste établi). Distinctes =
// 2 flags par-item différents (ex. visible/enabled).
const CMD_SET_ITEM_FLAG_A: u32 = 0x838B_3427; // (objId, itemIndex, bool) -> bool
const CMD_SET_ITEM_FLAG_B: u32 = 0x32F6_5AA1; // (objId, itemIndex, bool) -> bool

// ---------------------------------------------------------------------------
// cmdId résiduels reversés sur le VRAI désassemblage (handlers + helpers d'args ANCRÉS sur les
// setters connus : ReadNum=0x1405D0240, ReadBool=0x1405D0120, GetArgCount=0x1405CF2A0,
// GetCurrentLayerObj=0x140CF5B60, FindLayerById=0x1404E0C30, FindObjectInLayer=0x14051B5D0).
// Émis par title_menu_2 / main_menu au runtime. Convention : `objId` = arg0 partout ; le layer
// cible est le layer COURANT sauf si un layerId optionnel est passé en dernière position.
// CONFIRMÉ = layout + champ écrit lus sur le désasm ; INFÉRÉ = classe de champ déduite.
// — setters graphiques (sprite/icône) —
const CMD_SET_ICON_SPRITE: u32 = 0x214D_A123; // (objId, h1, h2, h3, n4, [idx], [en], [layer]) ; handler 0x140CE74D0 -> 0x14053EB00 (nœud graphique clé-hash). CONFIRMÉ layout ; champ INFÉRÉ (h1 = hash graphique primaire). Le plus fréquent (×38 title / ×20 mainmenu).
const CMD_SET_NODE_SPRITE: u32 = 0x72DC_82EA; // (objId, index, spriteHash) ; handler 0x140CDEEC0 -> 0x14101A280 (écrit dword clé-hash sur sous-nœud). CONFIRMÉ layout ; champ sprite INFÉRÉ.
const CMD_SET_NODE_SPRITE_EN: u32 = 0x497E_D10D; // (objId, index, spriteHash, enabled) ; handler 0x140CDE7C0 -> 0x14101A280 + 0x14101A5A0 (hash + flag). CONFIRMÉ layout ; champ sprite/visible INFÉRÉ.
// — setters de visibilité (part / enfant) —
const CMD_SET_PART_ENABLED: u32 = 0xCAE6_622C; // (objId, index, partHash, enabled, [layer]) ; handler 0x140CC77C0 : écrit BYTE [part+0x90] sur la part repérée par hash. CONFIRMÉ (classe visibilité).
const CMD_SET_ALL_PARTS_ENABLED: u32 = 0x20DD_A040; // (objId, index, enabled) ; handler 0x140CDE670 : propage le flag à TOUS les enfants (0x14101A5A0). CONFIRMÉ (classe visibilité).
const CMD_SET_CHILD_VISIBLE: u32 = 0xCB02_96B4; // (objId, childHash, visible, [index]) ; handler 0x140CE7CB0 -> 0x140540E90 (chemin de visibilité, même check 0x14053EE50 que SetObjectVisible). CONFIRMÉ (classe visibilité).
const CMD_SET_OBJECT_ACTIVE_S: u32 = 0xD1B5_1DF0; // (objId, active, [layer]) ; handler 0x140CF3940 -> 0x14051A6B0 (écrit un flag d'octet). CONFIRMÉ layout ; champ active INFÉRÉ.
// — setter numérique —
const CMD_SET_ITEM_COUNT: u32 = 0xC1DE_BA99; // (objId, count, [a], [b]) ; handler 0x140CE6D50 -> 0x14053E550 (écrit DWORD [obj+0x148] = count, dirty [obj+0x160]=1). CONFIRMÉ (champ numérique).
// — référence d'objet (mutation de sous-nœud non encore modélisée) —
const CMD_NODE_PARAM: u32 = 0xD72B_5ED5; // (objId, index, v2, v3, [flag]) ; handler 0x140CDF220 -> 0x14101A140 (écrit BYTE [node+0xB1]). Layout CONFIRMÉ ; sémantique du champ NON modélisée -> on référence l'objet seulement.
const CMD_OBJECT_ACTION: u32 = 0x2581_DC5C; // (objId, index, [layer]) ; handler 0x140CF48C0 -> 0x14051E970 (appel virtuel sur l'objet). Layout CONFIRMÉ ; action NON modélisée -> on référence l'objet seulement.

// ---------------------------------------------------------------------------
// cmdId résiduels — VAGUE 3 (peuplent title/mainmenu). Reversés sur le VRAI désasm + vérifiés
// sur les appels runtime (`zz_argcap`/`--menu … --runtime`). ANCRES : GetCurrentLayerObj=
// 0x140CF5B60, ReadNum=0x1405D0240, ReadBool=0x1405D0120, GetArgCount=0x1405CF2A0, FindLayerById=
// 0x1404E0C30, FindObjectInLayer=0x14051B5D0, PushRet=0x1405CF860. Convention : `objId` = arg0
// (sauf list-pop : arg0 = layerId, arg1 = objId). CONFIRMÉ = layout + champ lus sur le désasm ET
// recoupés sur les args réels ; INFÉRÉ = sémantique du champ déduite.
// — title —
const CMD_SET_OBJECT_VALUE: u32 = 0x988B_5B82; // (objId, valueHash, flag) ; 0x140CE6580 : FindObjectInLayer puis DWORD [obj+0x140]=valueHash + BYTE [obj+0x172]=flag. CONFIRMÉ ; valueHash -> `value`, flag non modélisé.
const CMD_SET_ITEM_PARAM: u32 = 0x513C_6C70; // (objId, itemIndex, key, value, [en]) ; 0x140C96E80 : FindObjectInLayer(obj, itemIndex) puis pose key->value par item (émis pour itemIndex 0..N). CONFIRMÉ layout (peuple la liste) ; param par item non modélisé -> objet enregistré.
const CMD_SET_SUBOBJECT_ENABLED: u32 = 0xFC56_9E77; // (objId, index, enabled) ; 0x140CCD490 -> sous-objet [obj+0x10], setter bool 0x14054B610. CONFIRMÉ (classe visibilité).
const CMD_SET_NODE_INDEX: u32 = 0x9CAB_2E41; // (objId, index, value, [layerId]) ; 0x140CD0470 -> 0x140540990(obj,value) + sous-nœud(value+1). CONFIRMÉ layout (layerId optionnel) ; champ INFÉRÉ -> objet enregistré.
const CMD_OBJECT_ACTION_BY_ID: u32 = 0x4BE9_C865; // (objId, [index], [layerId]) ; 0x140CF4400 -> 0x14051E970 (MÊME action virtuelle que ObjectAction 0x2581DC5C). CONFIRMÉ ; action non modélisée -> objet enregistré.
// — mainmenu —
const CMD_SET_NODE_VALUE: u32 = 0x8B1D_38C4; // (objId, index, subId, value) ; 0x140CEA2C0 : sous-nœud par index (0x140540400), écrit DWORD [sub+0xC8]=value + dirty. CONFIRMÉ layout ; champ INFÉRÉ -> objet enregistré.
const CMD_SET_NODE_PARAM_BLOCK: u32 = 0xBE2A_7145; // (objId, index, hashA, hashB, [nil]) ; 0x140CE3240 construit un bloc descripteur local (consts 0x101/0x0e) depuis les args. CONFIRMÉ partiel ; bloc non modélisé -> objet enregistré.
const CMD_SET_PART_PARAM_I: u32 = 0x2044_7515; // (objId, partId, v, [v3]) ; 0x140CEE9A0 itère les enfants pour trouver `partId` ([child+0xA8]) et écrit un entier. CONFIRMÉ layout ; champ par-part non modélisé -> objet enregistré.
const CMD_SET_PART_PARAM_F: u32 = 0x5F21_01DB; // (objId, partId, vi, vf, ...) ; 0x140CEE4E0 lit arg[3] en FLOAT (cvtsd2ss), arg[5] int, arg[6] bool. CONFIRMÉ layout (arg float) ; champ non modélisé -> objet enregistré.
const CMD_SET_SUBNODE_ENABLED: u32 = 0x80AB_69F3; // (objId, subId, enabled, [v]) ; 0x140CE7A50 -> 0x140540FC0(obj, subId, enabled). CONFIRMÉ (classe visibilité).
const CMD_SET_OBJECT_FLAG: u32 = 0x816C_D673; // (objId, flag, [layerId]) ; 0x140CCBBC0 résout l'objet dans la collection [layer+0x130] et écrit un bool. CONFIRMÉ layout ; champ non modélisé -> objet enregistré.
const CMD_SET_ELEMENT_COLOR: u32 = 0x2FC4_7DA5; // (objId, _, hash, _, r, g, …) ; 0x140CC33F0 (trouvé via la table de dispatch) : lit ≥6 args dont plusieurs FLOATS (cvtsd2ss xmm7/8/9, défaut 1.0f), résout un SOUS-ÉLÉMENT (0x14051B5D0) et lui applique une COULEUR RGBA. CONFIRMÉ layout (couleur) ; sous-élément+canaux non modélisés -> objet enregistré.
const CMD_SET_LIST_ITEM_VALUES: u32 = 0x1AF6_1E89; // (layerId, objId, <table>, [tag]) ; 0x140CB0240 : FindLayerById(layerId), lit la table (0x1404B0CA0) dans des tableaux de valeurs PAR ITEM de l'objet-liste ([sub+0x70C-0x250]/-0x128/+0). CONFIRMÉ layout ; tables non modélisées -> objet-liste enregistré.
const CMD_SET_LIST_ITEM_VALUES_MULTI: u32 = 0x83B4_F0AC; // (layerId, objId, <table>×N) ; 0x140CB0460 : MÊME famille (FindLayerById + lecteurs de table 0x1404B0CA0/…BD0/…D80), plusieurs tableaux. CONFIRMÉ layout ; tables non modélisées -> objet-liste enregistré.
// — getter —
const CMD_GET_NODE_INDEX_BY_HASH: u32 = 0x06B1_9AFF; // (objId, index, hash) -> int ; 0x140CF0340 : FindObjectInLayer puis recherche un sous-nœud par `hash` (0x1405427C0) et renvoie son index via PushRet. CONFIRMÉ getter ; renvoie 0 par défaut (lookup moteur non simulé).

/// Famille « set/apply état moteur → return 1 », reversée en **BATCH via la table de dispatch**
/// (`scripts/extract_funclua_table.py` → handler, puis désassemblage r2). Chaque handler lit ses
/// args, applique une valeur à l'état moteur (`call 0x1405CF730`/`0x1404Exxx`) et renvoie **AL=1**
/// sur le chemin principal (garde no-arg → 0). Critère de sûreté vérifié sur CHAQUE entrée : **≤ 2
/// `ret` et `main_return = mov al,1`** (le `ret` non-principal = la garde no-arg `xor al,al`). niers
/// ne réplique pas la mutation moteur, mais le RETOUR correct est **1** (le défaut getter `0` serait
/// FAUX si le script teste le retour). Exclus : handlers à 3+ ret, à retour principal 0, ou à `al`
/// écrit conditionnellement (`sete al`). Observés inconnus sur `shop`.
///
/// Les 4 dernières ont un `setXX` mais qui écrit un AUTRE registre (bpl/r14b/dil) = la **valeur
/// APPLIQUÉE**, pas le retour (`main_return` reste `mov al,1`, vérifié) — donc tout aussi portables.
/// Handlers (cmdId → VA) : `0x061919E0→0x140CF38B0` `0x2145E72C→0x140CC6C30` `0x32565F92→0x140CF4610`
/// `0x36830727→0x140CE5BD0` `0x3CB1C712→0x140CD07C0` `0x546C3F5D→0x140CC21A0` `0x72D88B24→0x140CE5CD0`
/// `0x84FCEF86→0x140CF5A90` `0x9021B6E8→0x140CB1BA0` `0x9B2AAF08→0x140CE6A10` `0x9BAD0175→0x140CC2260`
/// `0xA1D31171→0x140CE5EE0` `0x58E879A0→0x140CD0600` `0x59B7A7B2→0x140CE9C30` `0x7A7EFBE7→0x140CE9670`
/// `0x9D688EB3→0x140CF4510`.
const REVERSED_RETURN1: &[u32] = &[
    // 0x140CB0E20 : appelle le service de configuration avec la valeur d'un
    // octet calculée par le moteur, puis termine par `mov al,1`.
    0x4054_AD7F,
    // 0x140CAE4F0 : calcule un état local puis l'applique via 0x1405E7DB0 ;
    // le seul retour atteint est `mov al,1` à 0x140CAE58B (les `xor al,al`
    // intermédiaires ne sortent jamais directement de la fonction).
    0xAA99_33B4,
    0x0619_19E0,
    0x2145_E72C,
    0x3256_5F92,
    0x3683_0727,
    0x3CB1_C712,
    0x546C_3F5D,
    0x72D8_8B24,
    0x84FC_EF86,
    0x9021_B6E8,
    0x9B2A_AF08,
    0x9BAD_0175,
    0xA1D3_1171,
    // setXX pour la valeur appliquée (≠ retour) ; main_return = mov al,1 vérifié :
    0x58E8_79A0,
    0x59B7_A7B2,
    0x7A7E_FBE7,
    0x9D68_8EB3,
];

/// cmdId `funcLuaMenuCommand` reversés sur le binaire **COURANT** (`nie_eacpatched.exe`, 3 juin 2026
/// — build distinct de celui des reversals historiques : les VAs de handler ont glissé, mais les
/// cmdId, eux, sont stables car hash du nom de commande). Repérés par le triage déterministe
/// `scripts/triage_funclua_handlers.py` (iced-x86 + bornes `.pdata` à chunks chaînés) : setters à
/// **ret unique**, **AUCUNE définition `al/eax = 0` dans le corps**, **définition finale `al = 1`**
/// → renvoient **1 inconditionnellement** (classe `RETURN_1_SAFE` du triage ; z0=0). Les args
/// numériques arrivent en f64 ; le champ moteur écrit n'est pas modélisé dans `MenuObjectState`
/// (comme nombre de setters déjà portés) → on renvoie 1, même sémantique que [`REVERSED_RETURN1`].
/// Deux sous-ensembles, tous deux `RETURN_1_SAFE` :
/// - **sans aucune déf `al=0`** (renvoient 1 sur tout chemin) : `0x804ACF1A→0x140CB0320`
///   `0xA30EF40C→0x140CA56A0` `0x1246829F→0x140C8BDC0` `0x0FA7DBFF→0x140CAFDE0` `0x2BC23608→0x140CAFB70`.
/// - **`al=0` confiné au bloc d'échec de la garde d'arité** (`cmp edx,N ; jae MAIN ; …xor al,al;ret`),
///   vérifié en flux-de-contrôle : ce bloc est mort à l'exécution (les scripts livrés passent ≥N args,
///   sinon le jeu ne tournerait pas) → renvoient 1 au runtime : `0x86544EF0→0x140C8C520` (N=3)
///   `0x4B438A8F→0x140C87E80` (N=2) `0xF53E842E→0x140C8C190` (N=1) `0x66F84ED3→0x140C8BCC0` (N=1)
///   `0xD9CFE5C9→0x140CAF0B0` (N=2). Détecteur : `scripts/triage_funclua_handlers.py` (classe
///   `RETURN_1_SAFE`, colonne `zero=none|guard`).
const ARG_GUARDED_RETURN1: &[u32] = &[
    0x804A_CF1A,
    0xA30E_F40C,
    0x1246_829F,
    0x0FA7_DBFF,
    0x2BC2_3608,
    0x8654_4EF0,
    0x4B43_8A8F,
    0xF53E_842E,
    0x66F8_4ED3,
    0xD9CF_E5C9,
    // 2ᵉ lot (triage top-40, freq 22-27 ; mêmes critères CF, spot-check `0x85179093` = `cmp edx,2;
    // jae` → fallthrough `xor al,al;ret` seul 0, corps → `mov al,1;ret`) :
    0x8517_9093, // h 0x140C979F0 (zero=guard n=2)
    0x4350_26E5, // h 0x140CB0EF0 (zero=guard n=1)
    0xE652_E999, // h 0x140CB05D0 (zero=none)
    0xD07D_9BAE, // h 0x140C8BAD0 (zero=none)
    0x038D_9994, // h 0x140C99F80 (zero=guard n=3)
    0x71AB_6035, // h 0x140CAFC50 (zero=none)
    0x56A5_DCC3, // h 0x140CB0DD0 (zero=guard n=1)
    // 3ᵉ lot (triage top-60, freq 13-21 ; mêmes critères CF `RETURN_1_SAFE`) :
    0xA671_0517, // h 0x140CAF6B0 (zero=none)
    0x8E65_8E4A, // h 0x140C8BFE0 (zero=guard n=1)
    0xE833_C122, // h 0x140C96370 (zero=guard n=2)
    0x5C79_799E, // h 0x140CAA380 (zero=guard n=3)
    0x7EF0_B9C7, // h 0x140CB0CF0 (zero=none)
    0xA117_EB12, // h 0x140CAD5C0 (zero=guard n=3)
    0x785E_9A3C, // h 0x140C7E9E0 (zero=guard n=2)
    0x701E_F8D3, // h 0x140C78AB0 (zero=guard n=1)
    0x8868_506B, // h 0x140C8B120 (zero=guard n=1)
    // 4ᵉ lot (triage top-90, freq 8-12 — traîne, mêmes critères CF `RETURN_1_SAFE`) :
    0x346C_6F21,
    0x3DEA_5990,
    0x8DC6_915F,
    0x10E5_D8F7,
    0xA62A_42F6,
    0xCE98_7192,
    0x8330_11BA,
    0x6E33_C050,
    0xC423_2044,
    0x5E61_58CE,
    0xED9F_084F,
    0x46CC_4A4E,
    // 5ᵉ lot (triage EXHAUSTIF, `TOP_N=544` = tous les cmdId distincts du corpus Lua décompilé
    // local `data/lua_scripts/decompiled` — plus une fréquence-top partielle : la QUEUE COMPLÈTE
    // de fréquence 1 à 8, 157 cmdId, mêmes critères CF `RETURN_1_SAFE` que les lots précédents
    // (0 dupli vérifié contre `REVERSED_RETURN1`/`ARG_GUARDED_RETURN1`/tous les `const CMD_*`,
    // scripté). Générée depuis CE binaire (`nie.exe`, poste local — build distinct du VPS,
    // cf. note de dérive sur `extract_funclua_table.py` : cmdId stables, VAs handler non
    // reproduites ici pour la même raison qu'ailleurs) :
    0x704D_3F44,
    0x9C38_0A20,
    0xA3F8_CBC2,
    0x27CE_10D5,
    0x1A1E_8481,
    0xB560_76D5,
    0x43B4_15CB,
    0x0402_8181,
    0xDC38_D0CE,
    0x87C6_526A,
    0x9067_1259,
    0x47BE_CF41,
    0x7D2E_F53D,
    0x2985_85E8,
    0x9231_3999,
    0xAEBA_2FCC,
    0x2B24_A5F2,
    0xE279_A319,
    0x0779_7481,
    0xD2AE_5D71,
    0x26E2_984E,
    0xB22E_7075,
    0x6397_69A0,
    0xE203_F5CB,
    0x21D8_254D,
    0x7865_D899,
    0x491A_9686,
    0x11D3_F2B0,
    0x3B42_7E07,
    0x6000_F8CC,
    0xC491_114F,
    0xF2C9_3CBF,
    0x4744_8D22,
    0x3C13_B1A9,
    0xBC8A_A17C,
    0xD1AD_E9EC,
    0x86BF_FC00,
    0xD3C8_2DF1,
    0x151B_2A75,
    0x325F_AD2D,
    0xD7F9_7333,
    0xE47C_F232,
    0xCCEF_905F,
    0x3ACD_9BDD,
    0x4D46_2061,
    0x683C_63BE,
    0x1113_7C0A,
    0x7423_16EA,
    0x5E4A_F876,
    0xC226_E0F5,
    0xBC66_644D,
    0x67EF_F3D5,
    0x7452_1FA8,
    0xF34C_605A,
    0x53EE_5D32,
    0x6FFF_EF0D,
    0x74C1_60DC,
    0x9513_85F7,
    0x29AA_03D0,
    0x94F2_6D33,
    0xB5D2_AB40,
    0x201A_52F4,
    0xC62B_C0D5,
    0x174D_AA6F,
    0x46EC_3D1F,
    0xE128_306E,
    0xAD65_4670,
    0x15E0_78F6,
    0x619B_6CED,
    0x0DC6_BA89,
    0xD5F9_5DB4,
    0xEB49_D3AF,
    0xEB5F_D66B,
    0x27C6_E1BF,
    0xCA24_5B91,
    0x17F9_CB58,
    0xB6B5_858A,
    0x722D_7998,
    0x20C2_BA1E,
    0xFAEF_37F2,
    0x967B_7AF6,
    0xF6F2_2985,
    0xDF24_7C88,
    0x6127_005B,
    0xEAF4_C7BB,
    0x0781_09CF,
    0x2717_6B13,
    0x9449_FE94,
    0x99A4_EE6E,
    0x0B43_2863,
    0x53C1_ED53,
    0x0435_994B,
    0xFCD9_A689,
    0x5ADD_E5A1,
    0x4D34_5E45,
    0x0BD4_BBAB,
    0xD481_4C29,
    0xEBC9_20E9,
    0x9720_E23E,
    0x1894_3A27,
    0xD084_539E,
    0x7B78_E1F8,
    0x68F4_3836,
    0x2950_2913,
    0x959E_3B57,
    0xDC41_A109,
    0x28D9_F70C,
    0xE3E1_EFC9,
    0x22AD_7E4E,
    0x2BED_3772,
    0x74B5_7B1B,
    0x692A_0860,
    0x7F24_53AD,
    0x7AC2_7410,
    0x372D_7405,
    0xE601_A78E,
    0xA755_D37A,
    0x66A3_B0F0,
    0x4C7B_AB54,
    0x4814_6670,
    0xE696_EC94,
    0x5E09_B029,
    0xE505_E9BE,
    0x8942_2FF6,
    0x0908_4D9D,
    0xB712_39A7,
    0xFC69_B83A,
    0x236C_57DA,
    0x132B_557E,
    0xE0CB_B108,
    0x3F8F_C487,
    0xA935_727C,
    0xBF65_B63A,
    0x65F3_7D62,
    0xD7BB_C76D,
    0xBAA1_E3BA,
    0xDE11_A9E0,
    0xF4DF_6713,
    0xEF67_FDD0,
    0xA712_8BFD,
    0xDB26_3BED,
    0xA400_D7B7,
    0x8365_AC99,
    0xAA28_9F8C,
    0xCBEA_CB36,
    0xF994_7B44,
    0x106C_DBDE,
    0x853E_9EBB,
    0xD834_8143,
    0x094D_A7EF,
    0x3B76_DD51,
    0x6568_3DB5,
    0x83BD_04A7,
    0xD824_9AD1,
    0xE2B2_8B6F,
    0xE283_312E,
    0xF318_85AE,
    // 6ᵉ lot (mode `victory-road`, 2026-08-15) : 29 des 32 scripts Lua du mode n'étaient pas
    // encore dans le corpus `data/lua_scripts/decompiled` (seuls les 3 `fake_vroad_*` y étaient)
    // — décompilés via `luadec-all` pour cette session, ce qui a fait apparaître 31 cmdId
    // `funcLuaMenuCommand` anonymes propres au mode. Triés avec le même détecteur CF que les
    // lots précédents (`scripts/triage_funclua_handlers.py`, bornes `.pdata` chunkées) : 14 sont
    // `RETURN_1_SAFE` (0 confiné au bloc mort de garde d'arité, ou absent). Les 17 autres cmdId
    // du mode restent délibérément NON portés — soit un vrai retour 0 hors garde (`ARG_GUARD_TRUE` :
    // 0x19D76302, 0x1601208F, 0xD99E96A0, 0xCF5F6FA8, 0x03F4DAA9, 0x73C56CAA, 0xAAD018F6), soit un
    // argument flottant (`HAS_FLOAT_ARG`, setter à modéliser, PAS un `=>1` : 0x724F633E, 0x940EE4C3,
    // 0x5FCBCD5D, 0xBD0FBC0B, 0x0AC1B4A8, 0xE61C42AE, 0x526DFDBC, 0x5447BD41, 0x24579EE3,
    // 0x9AF7D6FA) — porter un retour conditionnel comme constante est interdit (cf. CLAUDE.md).
    0x1319_3DE3, // h 0x140CA35F0 (zero=guard n=1)
    0x53CD_EEE6, // h 0x140BEA8B0 (zero=guard n=1)
    0x95BE_9623, // h 0x140C738B0 (zero=guard n=1)
    0x0785_E3F0, // h 0x140CB17F0 (zero=none)
    0xE849_1B82, // h 0x140CA61C0 (zero=none)
    0xA631_3C13, // h 0x140C41C90 (zero=guard n=1)
    0xDAF8_8496, // h 0x140C98FC0 (zero=none)
    0x1726_222F, // h 0x140CB1000 (zero=none)
    0xFA25_DA56, // h 0x140CB1780 (zero=none)
    0x539C_F9D6, // h 0x140CAFEC0 (zero=guard n=1)
    0x0C5D_5B56, // h 0x140CAE350 (zero=guard n=2)
    0x37E8_6356, // h 0x140BFC9C0 (zero=guard n=1)
    0x9D7B_D6EF, // h 0x140CB1440 (zero=guard n=1)
    0xC879_6B78, // h 0x140CA6FD0 (zero=guard n=1)
    // 7ᵉ lot (éditeur d'avatar `chara_edit`, 2026-08-20) : ses scripts ne sont pas dans le corpus
    // décompilé local, donc le triage automatique ne les voyait pas. `0x5245F000` pèse à lui seul
    // **1 062 des 1 254 appels non gérés** des 42 écrans de l'éditeur (85 %).
    //
    // Le triage le classait `zero=BODY` — verdict à corriger, obtenu sur un fragment : le handler
    // est découpé en plusieurs entrées `.pdata` NON chaînées, et l'analyse s'arrêtait au premier
    // morceau. Flux de contrôle relu morceau par morceau : `cmp edx,3 ; jae CORPS` puis, sur le
    // chemin d'échec, un appel de rapport d'erreur suivi de `xor al,al ; ret` ; le morceau suivant
    // teste l'objet résolu et, s'il est nul, appelle LE MÊME rapport d'erreur avant `xor al,al ;
    // ret`. Les deux zéros sont donc des sorties d'erreur — arité insuffisante, objet introuvable —
    // et non une décision métier. Sur le chemin normal (3 args, objet existant), le corps rend 1.
    // Même sémantique que le reste de la liste, avec un cas d'échec de plus.
    0x5245_F000, // h 0x140D0E7F0 (2 sorties d'erreur, n=3)
    // Suite du même lot, flux relu fragment par fragment avec le même critère — un `ret` précédé
    // du rapport d'erreur `0x1405E7DB0` est une sortie d'échec, pas une décision métier :
    0xFDA3_6F2F, // h 0x140CFF2D0 — un SEUL ret, précédé du rapport d'erreur ; renvoie 1 sinon
    0xCA2E_6A00, // h 0x140CDA9F0 — 3 rets, les deux à 0 précédés du rapport d'erreur
    0x23AD_77AE, // h 0x140CD8600 — 3 rets, TOUS précédés du rapport d'erreur
    0xD8EE_0E5B, // h 0x140CD6250 — 4 rets, tous précédés du rapport d'erreur
    // Un seul `ret`, et aucune définition `al = 0` nulle part : renvoie 1 sur tout chemin. Le
    // triage le rangeait en `HAS_FLOAT_ARG` par prudence — il écrit un champ flottant que
    // `MenuObjectState` ne modélise pas, comme nombre de setters déjà portés ici. Le RETOUR, lui,
    // est ce sur quoi le script branche, et il est certain.
    0x940E_E4C3, // h 0x140D06870 (zero=none)
];

/// Nom lisible d'un `cmdId` `funcLuaMenuCommand` reversé, ou `None` si non encore identifié.
#[must_use]
pub fn command_name(cmd_id: u32) -> Option<&'static str> {
    Some(match cmd_id {
        CMD_SET_OBJECT_VISIBLE => "SetObjectVisible",
        CMD_SET_SPRITE => "SetSprite",
        CMD_SET_TEXT => "SetText",
        CMD_SET_COLOR => "SetColor",
        CMD_SET_LAYER_ACTIVE => "SetLayerActive",
        CMD_SET_PART_VISIBLE => "SetPartVisible",
        CMD_GET_NODE_FLOAT => "GetNodeFloat",
        CMD_GET_OBJECT_ATTR => "GetObjectAttr",
        CMD_GET_SPRITE_CELL_INDEX => "GetSpriteCellIndex",
        CMD_GET_GLOBAL_STATE_B => "GetGlobalStateB",
        CMD_GET_GLOBAL_STATE_A => "GetGlobalStateA",
        CMD_GET_OBJECT_ACTIVE => "GetObjectActive",
        CMD_KIZUNA_TOWN_RESET => "KizunaTownReset(=>1)",
        CMD_KIZUNA_SET_PART_VISIBLE => "KizunaSetPartVisible",
        CMD_KIZUNA_SET_PART_COLOR => "KizunaSetPartColorRGBA",
        CMD_KIZUNA_APPLY_PART_FLAGS => "KizunaApplyPartFlags(=>true)",
        CMD_KIZUNA_SET_PART_PARAM => "KizunaSetPartParam(=>true)",
        CMD_KIZUNA_SET_PART_TEXTURE => "KizunaSetPartTexture(=>true)",
        CMD_APPLY_GLOBAL_CONFIG_TRUE => "ApplyGlobalConfig(=>true)",
        CMD_APPLY_QUERY_TRUE => "ApplyQuery(=>true)",
        CMD_SET_GLOBAL_FLAG_TRUE => "SetGlobalFlag(=>1)",
        CMD_APPLY_DEFAULT_TRUE => "ApplyDefault(=>true)",
        CMD_REGISTER_ITEM_LIST_COUNT => "RegisterItemListCount",
        CMD_SET_SELECTED_INDEX => "SetSelectedIndex",
        CMD_SET_ITEM_FLAG_A => "SetItemFlagA",
        CMD_SET_ITEM_FLAG_B => "SetItemFlagB",
        // cmdId résiduels reversés
        CMD_SET_ICON_SPRITE => "SetIconSprite",
        CMD_SET_NODE_SPRITE => "SetNodeSprite",
        CMD_SET_NODE_SPRITE_EN => "SetNodeSpriteEnabled",
        CMD_SET_PART_ENABLED => "SetPartEnabled",
        CMD_SET_ALL_PARTS_ENABLED => "SetAllPartsEnabled",
        CMD_SET_CHILD_VISIBLE => "SetChildVisible",
        CMD_SET_OBJECT_ACTIVE_S => "SetObjectActive",
        CMD_SET_ITEM_COUNT => "SetItemCount",
        CMD_NODE_PARAM => "SetNodeParam",
        CMD_OBJECT_ACTION => "ObjectAction",
        // cmdId résiduels — vague 3
        CMD_SET_OBJECT_VALUE => "SetObjectValue",
        CMD_SET_ITEM_PARAM => "SetItemParam",
        CMD_SET_SUBOBJECT_ENABLED => "SetSubObjectEnabled",
        CMD_SET_NODE_INDEX => "SetNodeIndex",
        CMD_OBJECT_ACTION_BY_ID => "ObjectActionById",
        CMD_SET_NODE_VALUE => "SetNodeValue",
        CMD_SET_NODE_PARAM_BLOCK => "SetNodeParamBlock",
        CMD_SET_PART_PARAM_I => "SetPartParamI",
        CMD_SET_PART_PARAM_F => "SetPartParamF",
        CMD_SET_SUBNODE_ENABLED => "SetSubNodeEnabled",
        CMD_SET_OBJECT_FLAG => "SetObjectFlag",
        CMD_SET_ELEMENT_COLOR => "SetElementColor",
        CMD_SET_LIST_ITEM_VALUES => "SetListItemValues",
        CMD_SET_LIST_ITEM_VALUES_MULTI => "SetListItemValuesMulti",
        CMD_GET_NODE_INDEX_BY_HASH => "GetNodeIndexByHash",
        // Batch « apply état moteur → return 1 » (cf. REVERSED_RETURN1).
        c if REVERSED_RETURN1.contains(&c) => "ApplyReturn1(=>1)",
        // Setters à garde d'arité reversés sur le binaire courant (cf. ARG_GUARDED_RETURN1).
        c if ARG_GUARDED_RETURN1.contains(&c) => "ArgGuardedReturn1(=>1)",
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Helpers de décodage des arguments Lua (tous les nombres arrivent en f64)
// ---------------------------------------------------------------------------

fn lua_to_u32(v: Option<&Value>) -> u32 {
    match v {
        Some(Value::Number(n)) => *n as i64 as u32,
        Some(Value::Integer(i)) => *i as u32,
        _ => 0,
    }
}

fn lua_to_i32(v: Option<&Value>) -> i32 {
    match v {
        Some(Value::Number(n)) => *n as i64 as i32,
        Some(Value::Integer(i)) => *i as i32,
        _ => 0,
    }
}

fn lua_to_f32(v: Option<&Value>) -> f32 {
    match v {
        Some(Value::Number(n)) => *n as f32,
        Some(Value::Integer(i)) => *i as f32,
        _ => 0.0,
    }
}

fn lua_to_bool(v: Option<&Value>, default: bool) -> bool {
    match v {
        None => default,
        Some(Value::Boolean(b)) => *b,
        Some(Value::Number(n)) => *n != 0.0,
        Some(Value::Integer(i)) => *i != 0,
        Some(Value::Nil) => false,
        _ => default,
    }
}

fn lua_to_u32_or_none(v: Option<&Value>) -> Option<u32> {
    match v {
        Some(Value::Number(n)) => Some(*n as i64 as u32),
        Some(Value::Integer(i)) => Some(*i as u32),
        _ => None,
    }
}

/// Lit une table Lua séquentielle (`{v1, v2, …}`) en `Vec<i32>` (indices 1-based, ordre conservé).
/// Renvoie un vec vide si la valeur n'est pas une table. Les éléments non numériques valent 0.
fn lua_table_to_i32_vec(v: Option<&Value>) -> Vec<i32> {
    match v {
        Some(Value::Table(t)) => {
            let n = t.raw_len();
            (1..=n)
                .map(|i| t.raw_get::<Value>(i as i64).ok())
                .map(|e| lua_to_i32(e.as_ref()))
                .collect()
        }
        _ => Vec::new(),
    }
}

/// Représentation textuelle d'une valeur Lua (pour le journal de découverte).
fn value_repr(v: &Value) -> String {
    match v {
        Value::Nil => "nil".to_string(),
        Value::Boolean(b) => b.to_string(),
        Value::Integer(i) => format!("{i}"),
        Value::Number(n) => {
            let u = *n as i64 as u32;
            if u as f64 == *n {
                format!("0x{u:08X}")
            } else {
                format!("{n}")
            }
        }
        Value::String(s) => format!("{:?}", s.to_string_lossy()),
        Value::Table(_) => "<table>".to_string(),
        Value::Function(_) => "<function>".to_string(),
        _ => "<other>".to_string(),
    }
}

fn args_repr(args: &[Value]) -> String {
    args.iter().map(value_repr).collect::<Vec<_>>().join(", ")
}

// ---------------------------------------------------------------------------
// install_menu_host
// ---------------------------------------------------------------------------

/// Installe les fonctions hôtes de menu comme globals Lua et retourne le
/// [`MenuState`] partagé qu'elles vont muter.
///
/// Enregistre :
/// - `funcLuaMenuCommand(cmdId, layerId, …)` — dispatch principal menu.
/// - `funcLuaCommand(0xF2C13584, textId)` — résout un libellé injecté depuis `menu_text` ;
/// - `funcLuaCommand(0x0196FA01, conditionId)` — lit une condition injectée ;
/// - `funcLuaCommand(0x77E46CA8, listId, …)` — renvoie le compte et le statut de résolution ;
///   les autres commandes générales renvoient `0` et sont journalisées.
/// - `funcLuaActionCommand(…)` — no-op `0`.
/// - `funcLuaCameraCommand(…)` — no-op `0`.
/// - `funcLuaSpTacticsCommand(…)` — no-op `0`.
/// - `NameSettingBegin`, `AddNames`, `NameSettingEnd` — no-ops (retour nil).
/// - `IsCloseEndListLayer()` → `false`.
/// - `SetGuideStatusToLua`, `waitTrue`, `waitFalse` — no-ops.
/// - Stubs observés dans les scripts décompilés (iecode `LuaRuntime.cs`) :
///   `UpdateDetailWindowAttachBase`, `SaveAndShowWaitWindow`, `UploadSaveData`,
///   `OnCloseEndLayerCommon`, `OnChangeLayerGroupCommon`.
///
/// Note : `INCLUDE` n'est PAS installé ici — appelez [`crate::install_include`]
/// séparément pour connecter le résolveur VFS.
///
/// # Errors
/// [`mlua::Error`] si l'enregistrement d'un global échoue.
pub fn install_menu_host(lua: &Lua) -> mlua::Result<Rc<RefCell<MenuState>>> {
    let state = Rc::new(RefCell::new(MenuState::default()));

    // Le jeu fournit de nombreux modules de confort depuis son conteneur moteur (LISTVIEW,
    // GENERAL_WINDOW, MAIN_MENU, …), pas depuis chaque `.lua.bin`. Installer le même stub
    // tolérant que le runtime générique avant les globals connus permet aux scripts d'aller
    // jusqu'à leurs callbacks même lorsqu'un wrapper n'est pas encore porté ; les chemins
    // touchés restent inspectables via `_HOST_MISSING_PATHS`.
    crate::runtime::install_host_stubs(lua)?;

    // Ces namespaces sont fournis par les includes natifs du jeu, pas par les chunks Lua.
    // Les scripts Kizuna les utilisent dès les callbacks réseau/visiteur : laisser le proxy
    // global les fabriquer à la demande marquait à tort tout le module comme absent. Les deux
    // méthodes observées sont des réservations UI (sans valeur de retour utile) ; les méthodes
    // encore inconnues restent traçables au niveau `NAMESPACE.Method()`.
    lua.load(
        r#"
        local function known_namespace(name)
            local ns = {}
            setmetatable(ns, {
                __index = function(_, key)
                    local path = name .. "." .. tostring(key)
                    _HOST_MISSING_PATHS[path] = true
                    return function(...)
                        _HOST_MISSING_PATHS[path .. "()"] = true
                        return false
                    end
                end,
            })
            return ns
        end
        GENERAL_WINDOW = known_namespace("GENERAL_WINDOW")
        GENERAL_WINDOW.ReserveGeneralWindow = function(...) return true end
        GENERAL_WINDOW.CustomGeneralWindowBtn = function(...) return true end
        NETWORK_MENU = known_namespace("NETWORK_MENU")
        CHARA_EDIT_MENU = known_namespace("CHARA_EDIT_MENU")
        DETAIL_WINDOW = known_namespace("DETAIL_WINDOW")
        LISTVIEW = known_namespace("LISTVIEW")
        MAIN_MENU = known_namespace("MAIN_MENU")
        CHARA_FILTER_MENU = known_namespace("CHARA_FILTER_MENU")
        SPIRIT_FILTER_MENU = known_namespace("SPIRIT_FILTER_MENU")
        SOCCER_RESULT_MENU = known_namespace("SOCCER_RESULT_MENU")
        VICTORY_TOP_INC = known_namespace("VICTORY_TOP_INC")
        "#,
    )
    .exec()?;

    // Global moteur utilisé par de nombreux includes pour reconstruire les IDs de ressources.
    let crc32 =
        lua.create_function(|_, value: String| Ok(f64::from(crate::crc32(value.as_bytes()))))?;
    lua.globals().set("CRC32", crc32)?;

    // Constantes globales fournies par les includes natifs du jeu. Leurs valeurs sont les CRC32
    // retrouvés dans `data/re/menu-crc32-dictionary.json` du build courant ; les injecter évite
    // que la métatable de découverte les transforme en tables/stubs truthy, ce qui peut activer
    // ou désactiver des branches Lua de façon différente de `nie.exe`.
    let known_constants: &[(&str, u32)] = &[
        ("CHARA_EDIT_RECIPE_TYPE_FASHION", 0x6A09_9FEC),
        ("CHARA_EDIT_RECIPE_TYPE_IDEAL_PLAYER_IMAGE", 0x2067_0838),
        ("CHARA_EDIT_RECIPE_TYPE_PLAY_STYLE", 0x8258_E5CE),
        ("CHARA_EDIT_RECIPE_TYPE_SKILL_1", 0xD7F5_AB80),
        ("CHARA_EDIT_RECIPE_TYPE_SKILL_2", 0x4EFC_FA3A),
        ("CmdStateTypeNone", 0x27C6_9A90),
        ("EVEN_BONE_L21", 0xDCED_D4F1),
        ("EVEN_BONE_L22", 0x45E4_854B),
        ("EVEN_BONE_L23", 0x32E3_B5DD),
        ("EVEN_BONE_L24", 0xAC87_207E),
        ("EVEN_BONE_R21", 0xCA55_5A8B),
        ("EVEN_BONE_R22", 0x535C_0B31),
        ("EVEN_BONE_R23", 0x245B_3BA7),
        ("EVEN_BONE_R24", 0xBA3F_AE04),
        ("MAINMENU_TAB_TYPE_INVALID", 0x5AC0_6AC7),
        ("SOCCER_TUTORIAL_TYPE_CHANGE_CHARA", 0xCED9_5A6A),
        ("TEXTURE_NAME_CRC_EQUIP_ICON_MISANGA", 0x3D7C_B7C2),
        ("TEXTURE_NAME_CRC_EQUIP_ICON_PENDANT", 0x545A_3129),
        ("TEXTURE_NAME_CRC_EQUIP_ICON_SHOES", 0xE308_9601),
        ("TEXTURE_NAME_CRC_EQUIP_TYPE_MISANGA", 0xB762_8327),
        ("TEXTURE_NAME_CRC_EQUIP_TYPE_PENDANT", 0xDE44_05CC),
        ("TEXTURE_NAME_CRC_EQUIP_TYPE_SHOES", 0xAF2A_23CB),
        ("TEXTURE_NAME_CRC_EQUIP_TYPE_SPECIAL", 0xCC57_50F0),
        ("TEXTURE_NAME_RECIPE_TITLE_ICON16", 0x62E5_BAC1),
        ("TEXTURE_PATH_NAME_CRC_ICON_MISANGA", 0x8DD0_0905),
        ("TEXTURE_PATH_NAME_CRC_ICON_PENDANT", 0xE4F6_8FEE),
        ("TEXTURE_PATH_NAME_CRC_ICON_SHOES", 0x1902_5BCF),
        ("TEXT_NAME_CRC_DIFFICULTY_EXPLANATION_01", 0x9E9F_ABE8),
        ("TEXT_NAME_CRC_DIFFICULTY_EXPLANATION_02", 0x0796_FA52),
        ("TEXT_NAME_CRC_DIFFICULTY_EXPLANATION_03", 0x7091_CAC4),
        ("TEXT_NAME_CRC_DIFFICULTY_EXPLANATION_04", 0xEEF5_5F67),
        ("TYPE_INAZUMA_POST", 0x345D_5DB8),
    ];
    for &(name, value) in known_constants {
        lua.globals().set(name, f64::from(value))?;
    }

    // ── funcLuaMenuCommand(cmdId, layerId, …args) ─────────────────────────────
    {
        let state = Rc::clone(&state);
        let f = lua.create_function(move |_lua, args: Variadic<Value>| {
            let cmd_id = lua_to_u32(args.first());
            // `params` = TOUS les args après cmdId. Le layout (objId/layerId/index…) est
            // spécifique à chaque commande (cf. `dispatch_menu_command`) — il n'y a PAS de
            // layerId universel en position 1 (vérifié sur le vrai dispatch nie.exe).
            let params: Vec<Value> = args.into_iter().skip(1).collect();
            let ret = dispatch_menu_command(&mut state.borrow_mut(), cmd_id, &params);
            Ok(Value::Number(ret))
        })?;
        lua.globals().set("funcLuaMenuCommand", f)?;
    }

    // ── funcLuaCommand(cmdId, …args) — dispatch des commandes générales connues ──
    {
        let state = Rc::clone(&state);
        let f = lua.create_function(move |lua, args: Variadic<Value>| {
            let cmd_id = lua_to_u32(args.first());
            if matches!(cmd_id, CMD_GENERAL_GET_TEXT | CMD_GENERAL_GET_TEXT_CURRENT) {
                let text_id = lua_to_u32(args.get(1));
                let (layer, text) = {
                    let state = state.borrow();
                    (state.current_layer, state.text_by_id.get(&text_id).cloned())
                };
                state
                    .borrow_mut()
                    .known_cmd_log
                    .push(("GetText".to_string(), layer));
                return match text {
                    Some(text) => Ok(MultiValue::from_vec(vec![Value::String(
                        lua.create_string(text)?,
                    )])),
                    // Le handler local `0x140CA60C0` sélectionne une chaîne statique vide si
                    // la table ne contient pas l'ID, puis pousse toujours une string Lua.
                    None => Ok(MultiValue::from_vec(vec![Value::String(
                        lua.create_string("")?,
                    )])),
                };
            }
            if cmd_id == CMD_GENERAL_IS_CONDITION_ACTIVE {
                let condition_id = lua_to_u32(args.get(1));
                let (active, layer) = {
                    let state = state.borrow();
                    (
                        state
                            .condition_flags
                            .get(&condition_id)
                            .copied()
                            .unwrap_or(false),
                        state.current_layer,
                    )
                };
                state
                    .borrow_mut()
                    .known_cmd_log
                    .push(("IsConditionActive".to_string(), layer));
                return Ok(MultiValue::from_vec(vec![Value::Boolean(active)]));
            }
            if cmd_id == CMD_GENERAL_GET_LIST_COUNT {
                let list_id = lua_to_u32(args.get(1));
                let (count, resolved, layer) = {
                    let state = state.borrow();
                    let (count, resolved) = state
                        .list_counts
                        .get(&list_id)
                        .copied()
                        .unwrap_or((0, false));
                    (count, resolved, state.current_layer)
                };
                state
                    .borrow_mut()
                    .known_cmd_log
                    .push(("GetListCount".to_string(), layer));
                return Ok(MultiValue::from_vec(vec![
                    Value::Number(f64::from(count)),
                    Value::Boolean(resolved),
                ]));
            }
            if cmd_id == CMD_GENERAL_RESOURCE_AVAILABLE {
                let resource_id = lua_to_u32(args.get(1));
                let (available, layer) = {
                    let state = state.borrow();
                    (
                        state.resource_ids.contains(&resource_id),
                        state.current_layer,
                    )
                };
                state
                    .borrow_mut()
                    .known_cmd_log
                    .push(("ResourceAvailable".to_string(), layer));
                return Ok(MultiValue::from_vec(vec![Value::Boolean(available)]));
            }
            if cmd_id == CMD_GENERAL_APPLY_UI_EFFECT {
                let layer = state.borrow().current_layer;
                state
                    .borrow_mut()
                    .known_cmd_log
                    .push(("ApplyUiEffect".to_string(), layer));
                return Ok(MultiValue::from_vec(vec![Value::Number(f64::from(
                    u8::from(args.len() > 1),
                ))]));
            }
            if cmd_id == CMD_GENERAL_APPLY_STATE {
                let layer = state.borrow().current_layer;
                state
                    .borrow_mut()
                    .known_cmd_log
                    .push(("ApplyState".to_string(), layer));
                return Ok(MultiValue::from_vec(vec![Value::Number(f64::from(
                    u8::from(args.len() > 1),
                ))]));
            }
            if cmd_id == CMD_GENERAL_SET_GLOBAL_INT {
                let layer = state.borrow().current_layer;
                if args.len() <= 1 {
                    return Ok(MultiValue::from_vec(vec![Value::Number(0.0)]));
                }
                state
                    .borrow_mut()
                    .set_engine_int_2728(lua_to_i32(args.get(1)));
                state
                    .borrow_mut()
                    .known_cmd_log
                    .push(("SetGlobalInt".to_string(), layer));
                return Ok(MultiValue::from_vec(vec![Value::Number(1.0)]));
            }
            if cmd_id == CMD_GENERAL_QUERY_BOOL_A {
                let key = lua_to_u32(args.get(1));
                let active = state
                    .borrow()
                    .condition_flags
                    .get(&key)
                    .copied()
                    .unwrap_or(false);
                let layer = state.borrow().current_layer;
                state
                    .borrow_mut()
                    .known_cmd_log
                    .push(("GeneralQueryBoolA".to_string(), layer));
                return Ok(MultiValue::from_vec(vec![Value::Boolean(active)]));
            }
            if cmd_id == CMD_GENERAL_QUERY_BOOL_B {
                let key = lua_to_u32(args.get(1));
                let active = state
                    .borrow()
                    .condition_flags
                    .get(&key)
                    .copied()
                    .unwrap_or(false);
                let layer = state.borrow().current_layer;
                state
                    .borrow_mut()
                    .known_cmd_log
                    .push(("GeneralQueryBoolB".to_string(), layer));
                return Ok(MultiValue::from_vec(vec![Value::Boolean(active)]));
            }
            if cmd_id == CMD_GENERAL_QUERY_STATE {
                if args.len() < 2 {
                    return Ok(MultiValue::from_vec(vec![Value::Boolean(false)]));
                }
                let layer = state.borrow().current_layer;
                state
                    .borrow_mut()
                    .known_cmd_log
                    .push(("GeneralQueryState".to_string(), layer));
                // Le handler natif pousse d'abord le drapeau de succès, puis une valeur d'état.
                // Le registre d'état n'est pas encore alimenté par le save-data : 0 est donc le
                // défaut observable, mais le protocole de retour (bool, entier) est conservé.
                return Ok(MultiValue::from_vec(vec![
                    Value::Boolean(true),
                    Value::Number(0.0),
                ]));
            }
            if cmd_id == CMD_GENERAL_APPLY_CURRENT_STATE {
                let layer = state.borrow().current_layer;
                state
                    .borrow_mut()
                    .known_cmd_log
                    .push(("GeneralApplyCurrentState".to_string(), layer));
                return Ok(MultiValue::from_vec(vec![Value::Boolean(true)]));
            }
            if cmd_id == CMD_GENERAL_QUERY_CONFIG_BOOL {
                let layer = state.borrow().current_layer;
                state
                    .borrow_mut()
                    .known_cmd_log
                    .push(("GeneralQueryConfigBool".to_string(), layer));
                return Ok(MultiValue::from_vec(vec![Value::Boolean(false)]));
            }
            if cmd_id == CMD_GENERAL_GET_CURRENT_TEXT {
                let layer = state.borrow().current_layer;
                state
                    .borrow_mut()
                    .known_cmd_log
                    .push(("GeneralGetCurrentText".to_string(), layer));
                return Ok(MultiValue::from_vec(vec![Value::String(
                    lua.create_string("")?,
                )]));
            }
            if cmd_id == CMD_GENERAL_GET_CURRENT_INT {
                let layer = state.borrow().current_layer;
                state
                    .borrow_mut()
                    .known_cmd_log
                    .push(("GeneralGetCurrentInt".to_string(), layer));
                return Ok(MultiValue::from_vec(vec![Value::Number(0.0)]));
            }
            let layer = state.borrow().current_layer;
            state
                .borrow_mut()
                .unknown_general_cmd_log
                .push((cmd_id, layer, args_repr(&args)));
            Ok(MultiValue::from_vec(vec![Value::Number(0.0)]))
        })?;
        lua.globals().set("funcLuaCommand", f)?;
    }

    // ── funcLuaActionCommand / funcLuaCameraCommand / funcLuaSpTacticsCommand ─
    for name in &[
        "funcLuaActionCommand",
        "funcLuaCameraCommand",
        "funcLuaSpTacticsCommand",
    ] {
        let f = lua.create_function(|_lua, _args: Variadic<Value>| Ok(Value::Number(0.0)))?;
        lua.globals().set(*name, f)?;
    }

    // ── NameSettingBegin / AddNames / NameSettingEnd ──────────────────────────
    for name in &["NameSettingBegin", "AddNames", "NameSettingEnd"] {
        let f = lua.create_function(|_lua, _args: Variadic<Value>| Ok(()))?;
        lua.globals().set(*name, f)?;
    }

    // `LUA_MENU_DEF` est normalement fourni par le host C++ sous forme de namespace. Ces trois
    // entrées sont démontrées par les appels du corpus et correspondent aux no-ops globals
    // installés juste au-dessus ; les autres méthodes restent volontairement traçables tant
    // que leur effet moteur n'est pas reversé.
    let menu_def = lua.create_table()?;
    for name in &["NameSettingBegin", "AddNames", "NameSettingEnd"] {
        let function: Function = lua.globals().get(*name)?;
        menu_def.set(*name, function)?;
    }
    lua.globals().set("MENU_DEF", menu_def)?;

    // ── IsCloseEndListLayer() → false ─────────────────────────────────────────
    {
        let f = lua.create_function(|_lua, _args: ()| Ok(false))?;
        lua.globals().set("IsCloseEndListLayer", f)?;
    }

    // ── SetGuideStatusToLua / waitTrue / waitFalse — no-ops ───────────────────
    for name in &["SetGuideStatusToLua", "waitTrue", "waitFalse"] {
        let f = lua.create_function(|_lua, _args: Variadic<Value>| Ok(()))?;
        lua.globals().set(*name, f)?;
    }

    // ── Stubs supplémentaires observés dans les scripts (iecode LuaRuntime.cs) ─
    for name in &[
        "UpdateDetailWindowAttachBase",
        "SaveAndShowWaitWindow",
        "UploadSaveData",
        "OnCloseEndLayerCommon",
        "OnChangeLayerGroupCommon",
    ] {
        let f = lua.create_function(|_lua, _args: Variadic<Value>| Ok(()))?;
        lua.globals().set(*name, f)?;
    }

    Ok(state)
}

// ---------------------------------------------------------------------------
// Dispatch funcLuaMenuCommand
// ---------------------------------------------------------------------------

/// Dispatch principal : mute `state` en fonction du `cmd_id`, et retourne la valeur à renvoyer
/// au script Lua (0.0 pour les setters ; valeur calculée pour les getters).
///
/// Layouts d'args reversés du dispatch nie.exe (`data/re/funclua-cmdids.json`, DESIGN.md §13) :
/// `args` = tous les arguments **après** le cmdId. Il n'y a PAS de layerId universel en
/// position 0 ; chaque commande a son propre layout (objId/layerId/index selon la commande).
///
/// Les commandes non reversées sont journalisées (`unknown_cmd_log`) sans crasher le script.
fn dispatch_menu_command(state: &mut MenuState, cmd_id: u32, args: &[Value]) -> f64 {
    /// Layer cible d'une commande d'objet : arg explicite à `idx` sinon le layer courant.
    fn target_layer(state: &MenuState, args: &[Value], idx: usize) -> u32 {
        lua_to_u32_or_none(args.get(idx)).unwrap_or(state.current_layer)
    }

    // `kizuna_town_mainmenu` appelle ce reset sans argument pendant `OnInit`.
    // Le handler natif remet `[0x142294600]` à zéro puis renvoie AL=1 ; le
    // slot moteur ne fait pas partie de MenuState, mais le retour débloque
    // la branche Lua de démarrage.
    if cmd_id == CMD_KIZUNA_TOWN_RESET {
        state
            .known_cmd_log
            .push(("KizunaTownReset".to_string(), state.current_layer));
        return 1.0;
    }

    match cmd_id {
        // ── SetObjectVisible(objId, index, visible, [layerId]) ──────────────
        CMD_SET_OBJECT_VISIBLE => {
            let obj_id = lua_to_u32(args.first());
            let index = lua_to_i32(args.get(1));
            let visible = lua_to_bool(args.get(2), true);
            let layer = target_layer(state, args, 3);
            state
                .known_cmd_log
                .push(("SetObjectVisible".to_string(), layer));
            let o = state.layer(layer).obj(obj_id);
            o.visible = visible;
            o.visible_par_index.insert(index, visible);
        }

        // ── SetSprite(objId, index, cellId, frame, color, [layerId]) ────────
        CMD_SET_SPRITE => {
            let obj_id = lua_to_u32(args.first());
            let cell_id = lua_to_u32(args.get(2));
            let frame = lua_to_i32(args.get(3));
            let color = lua_to_u32_or_none(args.get(4));
            let layer = target_layer(state, args, 5);
            state.known_cmd_log.push(("SetSprite".to_string(), layer));
            let obj = state.layer(layer).obj(obj_id);
            obj.sprite_texture_hash = Some(cell_id);
            obj.frame = Some(frame);
            obj.color_hash = color;
        }

        // ── SetText(objId, index, textHash|string, …, [layerId]) ────────────
        CMD_SET_TEXT => {
            let obj_id = lua_to_u32(args.first());
            let text = match args.get(2) {
                Some(Value::String(s)) => Some(s.to_string_lossy()),
                Some(v @ Value::Number(_)) | Some(v @ Value::Integer(_)) => {
                    Some(format!("0x{:08X}", lua_to_u32(Some(v))))
                }
                _ => None,
            };
            let layer = state.current_layer;
            state.known_cmd_log.push(("SetText".to_string(), layer));
            state.layer(layer).obj(obj_id).text = text;
        }

        // ── SetColor(objId, part, colorId, [cellIndex]) ─────────────────────
        CMD_SET_COLOR => {
            let obj_id = lua_to_u32(args.first());
            let color_id = lua_to_u32(args.get(2));
            let layer = state.current_layer;
            state.known_cmd_log.push(("SetColor".to_string(), layer));
            state.layer(layer).obj(obj_id).color_hash = Some(color_id);
        }

        // ── SetLayerActive(layerId, active) — met aussi à jour le layer courant ─
        CMD_SET_LAYER_ACTIVE => {
            let layer_id = lua_to_u32(args.first());
            let active = lua_to_bool(args.get(1), true);
            state
                .known_cmd_log
                .push(("SetLayerActive".to_string(), layer_id));
            state.layer(layer_id).enabled = active;
            if active {
                state.current_layer = layer_id;
            }
        }

        // ── SetPartVisible(objId, index, partId, enabled, [layerId]) ────────
        // Granularité « part » non encore modélisée dans MenuObjectState : on rabat sur la
        // visibilité de l'objet (approximation documentée, cf. DESIGN.md §13).
        CMD_SET_PART_VISIBLE => {
            let obj_id = lua_to_u32(args.first());
            let enabled = lua_to_bool(args.get(3), true);
            let layer = target_layer(state, args, 4);
            state
                .known_cmd_log
                .push(("SetPartVisible".to_string(), layer));
            // Symétrique : la commande MONTRE autant qu'elle cache. Le handler (0x140CEA100) lit
            // ses quatre arguments puis réduit le dernier à un booléen (`setne`) qu'il applique —
            // il n'y a pas de chemin qui ignorerait `true`. N'agir que sur `false`, comme avant,
            // rendait un objet définitivement invisible dès qu'un script l'avait masqué une fois,
            // même quand un appel ultérieur le réaffiche. C'est la commande la plus fréquente de
            // l'éditeur d'avatar : 4 409 appels sur 5 425, soit 81 % du trafic reconnu.
            let index = lua_to_i32(args.get(1));
            let o = state.layer(layer).obj(obj_id);
            o.visible = enabled;
            o.visible_par_index.insert(index, enabled);
        }

        // KizunaSetPartVisible (0x140CCC930) : (objId, partId, visible).
        // Le handler natif résout l'objet puis la part par hash et marque la part dirty. Le
        // modèle conserve cette mutation séparément de la visibilité d'une instance d'objet.
        CMD_KIZUNA_SET_PART_VISIBLE => {
            if args.len() < 3 {
                return 0.0;
            }
            let obj_id = lua_to_u32(args.first());
            let part_id = lua_to_u32(args.get(1));
            let visible = lua_to_bool(args.get(2), true);
            let layer = state.current_layer;
            state
                .known_cmd_log
                .push(("KizunaSetPartVisible".to_string(), layer));
            state
                .layer(layer)
                .obj(obj_id)
                .part_visible
                .insert(part_id, visible);
            return 1.0;
        }

        // KizunaSetPartColorRGBA (0x140CDF9F0) : (objId, partIndex, partId, r, g, b, a).
        // Le septième argument est optionnel dans le natif (alpha = 0 si absent), mais les
        // appels Kizuna réels fournissent bien les quatre canaux. On garde les floats, sans
        // quantification prématurée, afin que le rendu puisse appliquer le même vecteur.
        CMD_KIZUNA_SET_PART_COLOR => {
            if args.len() < 6 {
                return 0.0;
            }
            let obj_id = lua_to_u32(args.first());
            let part_id = lua_to_u32(args.get(2));
            let rgba = [
                lua_to_f32(args.get(3)),
                lua_to_f32(args.get(4)),
                lua_to_f32(args.get(5)),
                lua_to_f32(args.get(6)),
            ];
            let layer = state.current_layer;
            state
                .known_cmd_log
                .push(("KizunaSetPartColorRGBA".to_string(), layer));
            state
                .layer(layer)
                .obj(obj_id)
                .part_color_rgba
                .insert(part_id, rgba);
            return 1.0;
        }

        // ── SetIconSprite(objId, h1, h2, h3, n4, [idx], [enable], [layerId]) ──
        // Handler 0x140CE74D0 : résout l'objet (arg0) puis 0x14053EB00 écrit des données
        // graphiques clé-hash. Layout CONFIRMÉ ; on retient `h1` (arg1) comme hash de sprite
        // primaire (INFÉRÉ). Commande la plus fréquente (icônes d'onglets du main_menu).
        CMD_SET_ICON_SPRITE => {
            // (objId, texturePathCrc, textureNameCrc, frameHash, 0, [layer]) — cf. Lua décompilé.
            let obj_id = lua_to_u32(args.first());
            let h1 = lua_to_u32_or_none(args.get(1)); // chemin g4tx
            let h2 = lua_to_u32_or_none(args.get(2)); // nom de région/texture
            let layer = target_layer(state, args, 7);
            state
                .known_cmd_log
                .push(("SetIconSprite".to_string(), layer));
            let o = state.layer(layer).obj(obj_id);
            o.sprite_texture_hash = h1;
            o.sprite_region_hash = h2;
        }

        // ── SetNodeSprite(objId, index, spriteHash) ─────────────────────────
        // Handler 0x140CDEEC0 -> 0x14101A280 (dword clé-hash sur sous-nœud). Champ sprite INFÉRÉ.
        CMD_SET_NODE_SPRITE => {
            let obj_id = lua_to_u32(args.first());
            let sprite = lua_to_u32_or_none(args.get(2));
            let layer = state.current_layer;
            state
                .known_cmd_log
                .push(("SetNodeSprite".to_string(), layer));
            state.layer(layer).obj(obj_id).sprite_texture_hash = sprite;
        }

        // ── SetNodeSpriteEnabled(objId, index, spriteHash, enabled) ─────────
        // Handler 0x140CDE7C0 -> 0x14101A280 (hash) + 0x14101A5A0 (flag). Sprite + visibilité INFÉRÉS.
        CMD_SET_NODE_SPRITE_EN => {
            let obj_id = lua_to_u32(args.first());
            let sprite = lua_to_u32_or_none(args.get(2));
            let enabled = lua_to_bool(args.get(3), true);
            let layer = state.current_layer;
            state
                .known_cmd_log
                .push(("SetNodeSpriteEnabled".to_string(), layer));
            let obj = state.layer(layer).obj(obj_id);
            obj.sprite_texture_hash = sprite;
            obj.visible = enabled;
        }

        // ── SetPartEnabled(objId, index, partHash, enabled, [layerId]) ──────
        // Handler 0x140CC77C0 : écrit BYTE [part+0x90] sur la part repérée par `partHash`.
        // Granularité part non modélisée : le fallback objet reste symétrique.
        CMD_SET_PART_ENABLED => {
            let obj_id = lua_to_u32(args.first());
            let enabled = lua_to_bool(args.get(3), true);
            let layer = target_layer(state, args, 4);
            state
                .known_cmd_log
                .push(("SetPartEnabled".to_string(), layer));
            let obj = state.layer(layer).obj(obj_id);
            obj.visible = enabled;
        }

        // ── SetAllPartsEnabled(objId, index, enabled) ───────────────────────
        // Handler 0x140CDE670 : propage le flag à TOUS les enfants. Fallback symétrique.
        CMD_SET_ALL_PARTS_ENABLED => {
            let obj_id = lua_to_u32(args.first());
            let enabled = lua_to_bool(args.get(2), true);
            let layer = state.current_layer;
            state
                .known_cmd_log
                .push(("SetAllPartsEnabled".to_string(), layer));
            let obj = state.layer(layer).obj(obj_id);
            obj.visible = enabled;
        }

        // ── SetChildVisible(objId, childHash, visible, [index]) ─────────────
        // Handler 0x140CE7CB0 -> 0x140540E90 (chemin de visibilité). Fallback symétrique.
        CMD_SET_CHILD_VISIBLE => {
            let obj_id = lua_to_u32(args.first());
            let visible = lua_to_bool(args.get(2), true);
            let layer = state.current_layer;
            state
                .known_cmd_log
                .push(("SetChildVisible".to_string(), layer));
            let obj = state.layer(layer).obj(obj_id);
            obj.visible = visible;
        }

        // ── SetObjectActive(objId, active, [layerId]) ───────────────────────
        // Handler 0x140CF3940 -> 0x14051A6B0 (flag d'octet). Champ `active` INFÉRÉ.
        CMD_SET_OBJECT_ACTIVE_S => {
            let obj_id = lua_to_u32(args.first());
            let active = lua_to_bool(args.get(1), true);
            let layer = target_layer(state, args, 2);
            state
                .known_cmd_log
                .push(("SetObjectActive".to_string(), layer));
            state.layer(layer).obj(obj_id).active = active;
        }

        // ── SetItemCount(objId, count, [a], [b]) ────────────────────────────
        // Handler 0x140CE6D50 -> 0x14053E550 : écrit DWORD [obj+0x148] = count. Champ numérique CONFIRMÉ.
        CMD_SET_ITEM_COUNT => {
            let obj_id = lua_to_u32(args.first());
            let count = lua_to_i32(args.get(1));
            let layer = state.current_layer;
            state
                .known_cmd_log
                .push(("SetItemCount".to_string(), layer));
            state.layer(layer).obj(obj_id).number = Some(count);
        }

        // ── SetNodeParam / ObjectAction : objId reversé, mutation interne non modélisée ──
        // Handlers 0x140CDF220 (BYTE [node+0xB1]) et 0x140CF48C0 (appel virtuel). Le layout
        // (objId=arg0) est CONFIRMÉ mais le champ ciblé n'est pas représenté dans MenuObjectState :
        // on référence l'objet (le runtime l'a bien piloté) sans inventer de champ.
        CMD_NODE_PARAM | CMD_OBJECT_ACTION => {
            let obj_id = lua_to_u32(args.first());
            let layer = if cmd_id == CMD_OBJECT_ACTION {
                target_layer(state, args, 2)
            } else {
                state.current_layer
            };
            if let Some(name) = command_name(cmd_id) {
                state.known_cmd_log.push((name.to_string(), layer));
            }
            let _ = state.layer(layer).obj(obj_id);
        }

        // ── SetObjectValue(objId, valueHash, flag) ──────────────────────────
        // 0x140CE6580 : FindObjectInLayer puis DWORD [obj+0x140]=valueHash. On retient valueHash
        // dans `value` (champ générique) ; le flag [obj+0x172] n'est pas modélisé.
        CMD_SET_OBJECT_VALUE => {
            let obj_id = lua_to_u32(args.first());
            let value = lua_to_i32(args.get(1));
            let layer = state.current_layer;
            state
                .known_cmd_log
                .push(("SetObjectValue".to_string(), layer));
            state.layer(layer).obj(obj_id).value = Some(value);
        }

        // ── SetSubObjectEnabled(objId, index, enabled) ──────────────────────
        // 0x140CCD490 : setter bool sur le sous-objet [obj+0x10]. Granularité non modélisée :
        // fallback objet symétrique.
        CMD_SET_SUBOBJECT_ENABLED => {
            let obj_id = lua_to_u32(args.first());
            let enabled = lua_to_bool(args.get(2), true);
            let layer = state.current_layer;
            state
                .known_cmd_log
                .push(("SetSubObjectEnabled".to_string(), layer));
            state.layer(layer).obj(obj_id).visible = enabled;
        }

        // ── SetSubNodeEnabled(objId, subId, enabled, [v]) ───────────────────
        // 0x140CE7A50 -> 0x140540FC0(obj, subId, enabled). Fallback objet symétrique.
        CMD_SET_SUBNODE_ENABLED => {
            let obj_id = lua_to_u32(args.first());
            let enabled = lua_to_bool(args.get(2), true);
            let layer = state.current_layer;
            state
                .known_cmd_log
                .push(("SetSubNodeEnabled".to_string(), layer));
            state.layer(layer).obj(obj_id).visible = enabled;
        }

        // ── SetNodeIndex(objId, index, value, [layerId]) ────────────────────
        // 0x140CD0470 : layerId optionnel en arg[3] (FindLayerById) sinon layer courant. Le champ
        // d'index/valeur ciblé n'est pas représenté -> on enregistre l'objet (réellement piloté).
        CMD_SET_NODE_INDEX => {
            let obj_id = lua_to_u32(args.first());
            let layer = target_layer(state, args, 3);
            state
                .known_cmd_log
                .push(("SetNodeIndex".to_string(), layer));
            let _ = state.layer(layer).obj(obj_id);
        }

        // ── SetSelectedIndex(objId, index, [bool], [bool]) -> bool (0x6A06BC75) ──
        // handler 0x140CE6B20 reversé : écrit l'index SÉLECTIONNÉ (curseur) `[obj+0x154]` de la liste.
        // Renvoie al=1. Présent title02 + shop.
        CMD_SET_SELECTED_INDEX => {
            let obj_id = lua_to_u32(args.first());
            let index = lua_to_i32(args.get(1));
            let layer = state.current_layer;
            state
                .known_cmd_log
                .push(("SetSelectedIndex".to_string(), layer));
            state.layer(layer).obj(obj_id).selected_index = Some(index);
            return 1.0;
        }

        // ── SetItemFlagA/B(objId, itemIndex, bool) -> bool (0x838B3427 / 0x32F65AA1) ──
        // Flags par-item (handlers 0x140CC69F0/0x140CC7670 reversés, renvoient al=1 si ≥3 args). Les 2
        // commandes dominantes du shop. Champ par-item inféré → on enregistre le sous-item à `itemIndex`.
        CMD_SET_ITEM_FLAG_A | CMD_SET_ITEM_FLAG_B => {
            if args.len() >= 3 {
                let obj_id = lua_to_u32(args.first());
                let index = lua_to_i32(args.get(1));
                let layer = state.current_layer;
                if let Some(name) = command_name(cmd_id) {
                    state.known_cmd_log.push((name.to_string(), layer));
                }
                let _ = state.layer(layer).obj(obj_id).sub_item(index);
                return 1.0;
            }
            return 0.0;
        }

        // ── ObjectActionById(objId, [index], [layerId]) ─────────────────────
        // 0x140CF4400 -> 0x14051E970 (même action virtuelle que ObjectAction). layerId optionnel
        // en arg[2]. Action non modélisée -> on enregistre l'objet.
        CMD_OBJECT_ACTION_BY_ID => {
            let obj_id = lua_to_u32(args.first());
            let layer = target_layer(state, args, 2);
            state
                .known_cmd_log
                .push(("ObjectActionById".to_string(), layer));
            let _ = state.layer(layer).obj(obj_id);
        }

        // ── Setters de sous-nœud / part (objId = arg0, layer courant) ───────
        // SetNodeValue, SetNodeParamBlock, SetPartParamI/F (param int/float par part),
        // SetObjectFlag : le champ ciblé (sous-nœud, part, ou flag interne) n'est pas représenté
        // dans MenuObjectState -> on enregistre l'objet (le runtime l'a réellement piloté), ce
        // qui le fait joindre au layout.
        CMD_SET_NODE_VALUE
        | CMD_SET_NODE_PARAM_BLOCK
        | CMD_SET_PART_PARAM_I
        | CMD_SET_PART_PARAM_F
        | CMD_SET_ELEMENT_COLOR
        | CMD_SET_OBJECT_FLAG => {
            let obj_id = lua_to_u32(args.first());
            let layer = state.current_layer;
            if let Some(name) = command_name(cmd_id) {
                state.known_cmd_log.push((name.to_string(), layer));
            }
            let _ = state.layer(layer).obj(obj_id);
        }

        // ── SetItemParam(objId, itemIndex, key, value, [en]) ────────────────
        // 0x140C96E80 : FindObjectInLayer(obj, itemIndex) puis pose key->value PAR ITEM. Layout
        // vérifié sur `title_menu_2::SwapTitleBannerCapture` : (objId, itemIndex, key, value).
        // Enregistre le paramètre keyé dans le sous-item correspondant (modèle de liste).
        CMD_SET_ITEM_PARAM => {
            let obj_id = lua_to_u32(args.first());
            let item_index = lua_to_i32(args.get(1));
            let key = lua_to_u32(args.get(2));
            let value = lua_to_i32(args.get(3));
            let layer = state.current_layer;
            state
                .known_cmd_log
                .push(("SetItemParam".to_string(), layer));
            state
                .layer(layer)
                .obj(obj_id)
                .sub_item(item_index)
                .params
                .insert(key, value);
        }

        // ── SetListItemValues / …Multi(layerId, objId, <table>…) ────────────
        // 0x140CB0240 / 0x140CB0460 : arg0 = layerId (FindLayerById), arg1 = objId, puis les
        // tables de valeurs PAR ITEM. `…Values` passe 1 table (1 colonne), `…Multi` N tables
        // PARALLÈLES (N colonnes). Item i = [table0[i], table1[i], …] (vérifié sur
        // soccer_top_menu_inc / main_menu_inc : `funcLuaMenuCommand(…Multi, layer, obj, t4, t5,
        // t6, t7, t8)` avec t4..t8 = tableaux parallèles indexés par item). Peuple `sub_items`.
        CMD_SET_LIST_ITEM_VALUES | CMD_SET_LIST_ITEM_VALUES_MULTI => {
            let layer = lua_to_u32(args.first());
            let obj_id = lua_to_u32(args.get(1));
            if let Some(name) = command_name(cmd_id) {
                state.known_cmd_log.push((name.to_string(), layer));
            }
            // Colonnes = toutes les tables après (layerId, objId). Nb d'items = colonne la + longue.
            let columns: Vec<Vec<i32>> = args
                .get(2..)
                .unwrap_or(&[])
                .iter()
                .filter(|v| matches!(v, Value::Table(_)))
                .map(|v| lua_table_to_i32_vec(Some(v)))
                .collect();
            let n_items = columns.iter().map(Vec::len).max().unwrap_or(0);
            let obj = state.layer(layer).obj(obj_id);
            for i in 0..n_items {
                let vals: Vec<i32> = columns
                    .iter()
                    .map(|c| c.get(i).copied().unwrap_or(0))
                    .collect();
                obj.sub_item(i as i32).values = vals;
            }
        }

        // ── GetObjectAttr(objId, [attr]) -> int : lit la donnée de scène ─────
        // Utilisé par `GetItemButtonNum` (script) pour connaître le nombre d'item-buttons d'un
        // layer-list. La valeur vient de `state.object_attr` (renseigné par l'appelant depuis
        // les slots `AttachLocator` des objbin de l'écran) ; 0 par défaut (layer non-liste).
        CMD_GET_OBJECT_ATTR => {
            let obj_id = lua_to_u32(args.first());
            state
                .known_cmd_log
                .push(("GetObjectAttr".to_string(), state.current_layer));
            return f64::from(state.object_attr.get(&obj_id).copied().unwrap_or(0));
        }

        // ── GetObjectActive(objId, [index]) -> bool ─────────────────────────
        // Le getter lit le même octet d'état que SetObjectActive (handler
        // 0x140CF3940 -> 0x14051A6B0). L'index optionnel désigne une instance,
        // mais le handler de setter actuellement exposé par le corpus ne porte
        // pas de tableau par instance : on renvoie donc l'état de l'objet, qui
        // est le défaut moteur pour toutes les instances non distinguées.
        CMD_GET_OBJECT_ACTIVE => {
            let obj_id = lua_to_u32(args.first());
            let layer = state.current_layer;
            state
                .known_cmd_log
                .push(("GetObjectActive".to_string(), layer));
            return if state
                .layers
                .get(&layer)
                .and_then(|l| l.objects.get(&obj_id))
                .is_some_and(|o| o.active)
            {
                1.0
            } else {
                0.0
            };
        }

        // ── Autres getters : renvoient un défaut sûr (état moteur non simulé) ─
        // Renvoyer le défaut maintient le flot de contrôle du script sans crash. Affiner
        // (vraies valeurs depuis les données de jeu) est un travail ultérieur.
        CMD_GET_NODE_FLOAT
        | CMD_GET_SPRITE_CELL_INDEX
        | CMD_GET_NODE_INDEX_BY_HASH
        | CMD_GET_GLOBAL_STATE_A
        | CMD_GET_GLOBAL_STATE_B => {
            if let Some(name) = command_name(cmd_id) {
                state
                    .known_cmd_log
                    .push((name.to_string(), state.current_layer));
            }
            return 0.0;
        }

        // ── Famille « apply → return true » (no-arg) : handlers 0x140CBF150 / 0x140CEEE80 /
        // 0x140C96A50 REVERSÉS (désassemblage nie.exe). Chacun lit/query un état moteur, l'applique,
        // et renvoie **AL=1** (inconditionnel pour les 2 premiers ; cas par défaut flag==0 pour le 3ᵉ
        // = l'état frais de niers). niers ne réplique pas la mutation moteur, mais le RETOUR correct
        // est `1` (le défaut getter `0` serait FAUX si le script teste le retour). Cf. consts.
        // `CMD_SET_GLOBAL_FLAG_TRUE` (0x74578BF4, 1 arg) partage la même sémantique de RETOUR :
        // pose un flag moteur global (hors layout) et renvoie 1. Cf. son const pour le désassemblage.
        CMD_APPLY_GLOBAL_CONFIG_TRUE
        | CMD_APPLY_QUERY_TRUE
        | CMD_APPLY_DEFAULT_TRUE
        | CMD_SET_GLOBAL_FLAG_TRUE => {
            if let Some(name) = command_name(cmd_id) {
                state
                    .known_cmd_log
                    .push((name.to_string(), state.current_layer));
            }
            return 1.0;
        }

        // Batch « apply état moteur → return 1 » (12 handlers reversés via la table de dispatch ;
        // ≤2 ret + `mov al,1` dominant vérifié sur chacun, cf. REVERSED_RETURN1). Même sémantique
        // de retour que la famille ci-dessus.
        c if REVERSED_RETURN1.contains(&c) => {
            if let Some(name) = command_name(cmd_id) {
                state
                    .known_cmd_log
                    .push((name.to_string(), state.current_layer));
            }
            return 1.0;
        }

        // Setters à garde d'arité reversés sur le binaire COURANT (triage iced-x86 + bornes .pdata) :
        // ret unique, aucune déf `al=0`, déf finale `al=1` → renvoient 1. Champ moteur non modélisé.
        // Cf. ARG_GUARDED_RETURN1.
        c if ARG_GUARDED_RETURN1.contains(&c) => {
            if let Some(name) = command_name(cmd_id) {
                state
                    .known_cmd_log
                    .push((name.to_string(), state.current_layer));
            }
            return 1.0;
        }

        // Appels UI Kizuna à garde d'arité : les handlers 0x140CB28E0, 0x140CB3570 et
        // 0x140CB35C0 poussent une réussite après leur mutation dans le manager natif. On
        // conserve leurs arguments numériques par partie sans inventer leur structure native.
        CMD_KIZUNA_APPLY_PART_FLAGS | CMD_KIZUNA_SET_PART_PARAM | CMD_KIZUNA_SET_PART_TEXTURE => {
            if let Some(name) = command_name(cmd_id) {
                state
                    .known_cmd_log
                    .push((name.to_string(), state.current_layer));
            }
            if args.len() >= 3 {
                let obj_id = lua_to_u32(args.first());
                let part_id = lua_to_u32(args.get(2));
                let raw_args = args
                    .iter()
                    .skip(1)
                    .map(|value| lua_to_u32(Some(value)))
                    .collect::<Vec<_>>();
                let object = state.layer(state.current_layer).obj(obj_id);
                match cmd_id {
                    CMD_KIZUNA_APPLY_PART_FLAGS => {
                        object.part_flag_args.insert(part_id, raw_args);
                    }
                    CMD_KIZUNA_SET_PART_PARAM => {
                        object.part_param_args.insert(part_id, raw_args);
                    }
                    CMD_KIZUNA_SET_PART_TEXTURE => {
                        object.part_texture_args.insert(part_id, raw_args);
                    }
                    _ => unreachable!("commande Kizuna déjà filtrée"),
                }
            }
            return 1.0;
        }

        // ── RegisterItemListCount (0x16C1C4C0) : handler 0x140CD8E30 reversé ──
        // Enregistre `object_attr[objId] = count` (arg3) dans le manager d'items que GetObjectAttr
        // relit → GetItemButtonNum renvoie le count fourni par le SCRIPT. Renvoie 1 (al=1) si ≥4 args.
        CMD_REGISTER_ITEM_LIST_COUNT => {
            if args.len() >= 4 {
                let obj_id = lua_to_u32(args.first());
                let count = lua_to_i32(args.get(3));
                state.set_object_attr(obj_id, count);
                state
                    .known_cmd_log
                    .push(("RegisterItemListCount".to_string(), state.current_layer));
                return 1.0;
            }
            return 0.0;
        }

        // ── Commande non reversée : journal pour découverte ────────────────
        _ => {
            state
                .unknown_cmd_log
                .push((cmd_id, lua_to_u32(args.first()), args_repr(args)));
        }
    }
    0.0
}

// ---------------------------------------------------------------------------
// run_menu
// ---------------------------------------------------------------------------

/// Charge et exécute un script de menu `.lua.bin`, puis appelle `OnOpenLayer`
/// si le script l'a défini — c'est la convention moteur qui déclenche la
/// construction du menu.
///
/// Pré-condition : les globals hôtes doivent déjà être installés sur `lua`
/// (via [`install_menu_host`] et [`crate::install_include`]).
///
/// # Arguments
/// - `lua`          — VM Lua instrumentée.
/// - `script_bytes` — bytecode `.lua.bin` du script.
/// - `name`         — nom lisible (pour les messages d'erreur Lua).
/// - `layer_id`     — identifiant du layer à ouvrir (passé à `OnOpenLayer`).
///
/// # Retour
/// `Ok(true)` si `OnOpenLayer` a été trouvé et appelé ;
/// `Ok(false)` si le script ne définit pas `OnOpenLayer`.
///
/// # Errors
/// [`LuaError`] si le bytecode est invalide ou si la VM remonte une erreur.
pub fn run_menu(
    lua: &Lua,
    script_bytes: &[u8],
    name: &str,
    layer_id: u32,
) -> Result<bool, LuaError> {
    let func = crate::load_bytecode(lua, script_bytes, name)?;
    func.call::<()>(())?;

    // Convention moteur : le script définit OnOpenLayer(layerId) et/ou
    // OnSetupLayer(layerId). On appelle d'abord OnSetupLayer si présent, puis
    // OnOpenLayer (iecode LuaRuntime.cs).
    let setup: mlua::Value = lua.globals().get("OnSetupLayer")?;
    if let mlua::Value::Function(f) = setup {
        // tolérance : une erreur ici ne doit pas masquer OnOpenLayer
        let _ = f.call::<mlua::MultiValue>(layer_id as f64);
    }

    let on_open: mlua::Value = lua.globals().get("OnOpenLayer")?;
    if let mlua::Value::Function(f) = on_open {
        f.call::<()>(layer_id as f64)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

// ---------------------------------------------------------------------------
// enumerate_header_tabs — onglets virtuels d'en-tête (main_menu)
// ---------------------------------------------------------------------------

/// Un onglet d'en-tête de menu, énuméré depuis la VRAIE logique du script.
///
/// Les onglets du main menu sont des **sous-items virtuels** : ils ne sont PAS des objets de
/// l'objbin, mais sont décrits par les fonctions de script `GetSortOfTabs` (ordre + types),
/// `GetMenuObjectNameFromTabType` (hash de nom d'objet par type) et `GetTabTextIdCRC` (hash
/// d'id de texte du libellé). Cf. `main_menu_1.02.92.00` (`SetupHeaderTab`/`SetupHeaderTabIcon`).
#[derive(Debug, Clone)]
pub struct HeaderTab {
    /// Index visuel 0-based dans la barre d'onglets (ordre de `GetSortOfTabs`).
    pub index: usize,
    /// Type d'onglet (`10, 20, 30, 70, 40, 80, 60, 50, 90` en mode normal).
    pub tab_type: i64,
    /// Hash CRC32 du nom d'objet de l'onglet (`GetMenuObjectNameFromTabType(type)`).
    pub obj_hash: u32,
    /// Hash CRC32 de l'id de texte du libellé (`GetTabTextIdCRC(type)`), `0` si absent.
    pub text_id: u32,
}

/// Énumère les onglets d'en-tête d'un menu en appelant ses VRAIES fonctions de script déjà
/// définies par l'exécution top-level (`GetSortOfTabs` / `GetMenuObjectNameFromTabType` /
/// `GetTabTextIdCRC`). Retourne un vec vide si le script ne définit pas ces fonctions (= ce
/// n'est pas un menu à onglets), de sorte que l'appel est sûr pour n'importe quel écran.
///
/// ## Mode main-menu NORMAL forcé
///
/// `GetSortOfTabs` renvoie la table d'onglets *chronicle* (5) ou *normale* (9) selon
/// `MAIN_MENU.IsChronicleModeMainMenu()`. Or le stub `funcLuaCommand` renvoie `0.0`, **truthy**
/// en Lua, ce qui rend `IsChronicleModeMainMenu()` vrai à tort. Le main menu standard (cf.
/// capture de référence `start.png`) n'est PAS en mode chronicle ; on force donc ce prédicat à
/// `false` avant l'appel pour obtenir les **9 vrais onglets** (corrige l'artefact du stub, ne
/// fabrique rien : la liste et les hashes viennent de la logique réelle du script).
///
/// Pré-condition : le top-level du script (et ses `INCLUDE`) doivent avoir été exécutés (via
/// [`drive_menu`] ou [`run_menu`]) afin que ces globals existent.
#[must_use]
pub fn enumerate_header_tabs(lua: &Lua) -> Vec<HeaderTab> {
    // Force le mode normal (non-chronicle) si MAIN_MENU.IsChronicleModeMainMenu existe.
    let _ = lua
        .load(
            "if MAIN_MENU and MAIN_MENU.IsChronicleModeMainMenu then \
             MAIN_MENU.IsChronicleModeMainMenu = function() return false end end",
        )
        .exec();

    let g = lua.globals();
    let Ok(Value::Function(sort_fn)) = g.get::<Value>("GetSortOfTabs") else {
        return Vec::new();
    };
    let Ok(Value::Function(name_fn)) = g.get::<Value>("GetMenuObjectNameFromTabType") else {
        return Vec::new();
    };
    let text_fn = match g.get::<Value>("GetTabTextIdCRC") {
        Ok(Value::Function(f)) => Some(f),
        _ => None,
    };
    let Ok(sort) = sort_fn.call::<Table>(()) else {
        return Vec::new();
    };

    let mut tabs = Vec::new();
    for i in 1..=sort.raw_len() {
        let Ok(tab_type) = sort.raw_get::<i64>(i as i64) else {
            continue;
        };
        // GetMenuObjectNameFromTabType renvoie 0 pour un type sans objet (ex. 70/80 désactivés).
        let obj_hash = name_fn.call::<i64>(tab_type as f64).unwrap_or(0) as u32;
        if obj_hash == 0 {
            continue;
        }
        let text_id = text_fn
            .as_ref()
            .and_then(|f| f.call::<i64>(tab_type as f64).ok())
            .unwrap_or(0) as u32;
        tabs.push(HeaderTab {
            index: i - 1,
            tab_type,
            obj_hash,
            text_id,
        });
    }
    tabs
}

// ---------------------------------------------------------------------------
// drive_menu — boucle de construction du moteur (manager nie.exe 0x14109D190)
// ---------------------------------------------------------------------------

/// Rapport d'exécution du driver de menu (diagnostic honnête de ce qui a tourné).
#[derive(Debug, Clone, Default)]
pub struct DriveReport {
    /// L'exécution top-level du script (qui définit les callbacks) a réussi.
    pub top_level_ok: bool,
    /// Erreur top-level éventuelle (1ʳᵉ ligne), si `top_level_ok == false`.
    pub top_level_err: Option<String>,
    /// `OnInit()` : `None` = absent ; `Some(true)` = appelé OK ; `Some(false)` = erreur Lua.
    pub on_init: Option<bool>,
    /// `OnOpenLayer(layerId)` a été appelé sans erreur sur ≥1 layerId candidat.
    pub on_open: bool,
    /// Callbacks de cycle de vie présents (fonctions globales définies par le script).
    pub callbacks: Vec<String>,
    /// Erreurs de callbacks capturées pendant le pilotage (nom + contexte), sans interrompre
    /// les autres layers : elles expliquent précisément un état runtime partiel.
    pub callback_errors: Vec<String>,
    /// Globals moteur non fournis, touchés pendant les callbacks du menu.
    pub missing_host_calls: Vec<String>,
    /// Chemins imbriqués moteur touchés pendant les callbacks (`LISTVIEW.Set...`, etc.).
    pub missing_host_paths: Vec<String>,
}

/// Pilote un script de menu selon la séquence **reversée** du manager `nie.exe` (`0x14109D190`,
/// cf. DESIGN.md §13) : exécution top-level, pose du global `__menuObjPtr`, `OnInit()` sans
/// argument, puis pour chaque `layerId` candidat `OnSetupLayer`/`OnOpenLayer`/`OnEnter` **par
/// index d'item**, enfin `Step()` (une frame). Chaque callback est tolérant aux erreurs (pcall
/// implicite) — l'état partiel résultant est accumulé dans le [`MenuState`] retourné par
/// [`install_menu_host`].
///
/// ## Boucle par item (layers-list)
///
/// Le moteur appelle `OnOpenLayer(layerId, itemIndex)` une fois **par item-button** d'un
/// layer-list (cf. `SetupItemButton` du title menu). Le nombre d'items vient de la scène
/// (`GetObjectAttr`) ; on le passe ici via `item_counts` (clé = hash de layer, valeur = compte).
/// Pour un layer absent de `item_counts`, un seul passage `itemIndex = 0` est émis.
///
/// Pré-condition : les globals hôtes doivent déjà être installés sur `lua` (via
/// [`install_menu_host`] et [`crate::install_include`]) et, pour que `GetObjectAttr` réponde,
/// `MenuState::object_attr` doit être renseigné (typiquement avec les mêmes valeurs que
/// `item_counts`) AVANT l'appel.
///
/// # Arguments
/// - `lua`          — VM Lua instrumentée.
/// - `script_bytes` — bytecode `.lua.bin` du script.
/// - `name`         — nom lisible (messages d'erreur Lua).
/// - `layer_ids`    — identifiants de layer candidats (`OnSetupLayer`/`OnOpenLayer`/`OnEnter`).
/// - `item_counts`  — nombre d'items à piloter par layer-list (clé = hash de layer).
///
/// # Errors
/// [`LuaError`] uniquement si le bytecode est invalide (signature/format) — les erreurs
/// d'exécution des callbacks sont capturées dans le [`DriveReport`], pas propagées.
pub fn drive_menu_for_frames(
    lua: &Lua,
    script_bytes: &[u8],
    name: &str,
    layer_ids: &[u32],
    item_counts: &BTreeMap<u32, i32>,
    frames: u32,
) -> Result<DriveReport, LuaError> {
    let func = crate::load_bytecode(lua, script_bytes, name)?;

    let mut report = DriveReport::default();

    // Exécution top-level : définit les callbacks (OnInit, OnSetupLayer, …). Tolérante :
    // un script peut échouer en route mais avoir déjà défini ses callbacks (cf. main_menu
    // qui indexe un global de scène absent dans OnInit, pas au top-level).
    match func.call::<()>(()) {
        Ok(()) => report.top_level_ok = true,
        Err(e) => {
            report.top_level_err = Some(e.to_string().lines().next().unwrap_or("").to_string())
        }
    }

    let g = lua.globals();

    // `__menuObjPtr` : pointeur (numérique) vers l'objet menu C++ que le manager pose avant
    // d'appeler OnInit ; les scripts le lisent. Valeur non nulle suffit pour le flot de contrôle.
    g.set("__menuObjPtr", 1.0_f64)?;

    // OnInit() — SANS argument (vérité terrain : la séquence du manager).
    if let Ok(Value::Function(f)) = g.raw_get::<Value>("OnInit") {
        match f.call::<MultiValue>(()) {
            Ok(_) => report.on_init = Some(true),
            Err(e) => {
                report.on_init = Some(false);
                report.callback_errors.push(format!("OnInit: {e}"));
            }
        }
    }

    // Par layerId candidat et par index d'item : OnSetupLayer crée les objets (ré-entre dans
    // funcLuaMenuCommand), puis OnOpenLayer/OnEnter (ouverture). Le moteur passe (layerId,
    // itemIndex) ; pour un layer-list on itère 0..count, sinon un seul passage (index 0).
    // Tolérant : on collecte l'état partiel.
    for &lid in layer_ids {
        let count = item_counts.get(&lid).copied().unwrap_or(0).max(1);
        for idx in 0..count {
            for cb in ["OnSetupLayer", "OnOpenLayer", "OnEnter"] {
                if let Ok(Value::Function(f)) = g.raw_get::<Value>(cb) {
                    let result = f.call::<MultiValue>((lid as f64, f64::from(idx)));
                    let ok = result.is_ok();
                    if let Err(e) = result {
                        report
                            .callback_errors
                            .push(format!("{cb}(0x{lid:08X}, {idx}): {e}"));
                    }
                    if cb == "OnOpenLayer" && ok {
                        report.on_open = true;
                    }
                }
            }
        }
    }

    // Step() — avance la même VM sur plusieurs frames : les scripts du jeu utilisent des
    // coroutines et conservent leur état entre deux callbacks. Zéro signifie réellement zéro
    // frame ; `drive_menu()` demande explicitement une frame pour conserver son contrat court.
    let pre_step = g.raw_get::<Value>("PreStep").ok();
    let step = g.raw_get::<Value>("Step").ok();
    let post_step = g.raw_get::<Value>("PostStep").ok();
    for _ in 0..frames {
        if let Some(Value::Function(f)) = &pre_step
            && let Err(e) = f.call::<MultiValue>(())
        {
            report.callback_errors.push(format!("PreStep: {e}"));
        }
        if let Some(Value::Function(f)) = &step
            && let Err(e) = f.call::<MultiValue>(())
        {
            report.callback_errors.push(format!("Step: {e}"));
        }
        if let Some(Value::Function(f)) = &post_step
            && let Err(e) = f.call::<MultiValue>(())
        {
            report.callback_errors.push(format!("PostStep: {e}"));
        }
    }

    // Inventaire des callbacks de cycle de vie présents (diagnostic).
    for cb in [
        "OnInit",
        "OnSetupLayer",
        "OnOpenLayer",
        "OnEnter",
        "OnCloseLayer",
        "PreStep",
        "Step",
        "PostStep",
        "OnBack",
        "OnDecideFocus",
        "OnFunction",
    ] {
        if matches!(g.raw_get::<Value>(cb), Ok(Value::Function(_))) {
            report.callbacks.push(cb.to_string());
        }
    }

    // Les stubs installés par `install_menu_host` conservent la surface réellement demandée
    // pendant les callbacks. La lire ici, avant de rendre la VM au caller, évite de confondre
    // un top-level chargé avec un menu effectivement pilotable.
    report.missing_host_calls = collect_missing_paths(&g, "_HOST_MISSING");
    report.missing_host_paths = collect_missing_paths(&g, "_HOST_MISSING_PATHS");

    Ok(report)
}

fn collect_missing_paths(globals: &mlua::Table, table_name: &str) -> Vec<String> {
    let Ok(table) = globals.get::<mlua::Table>(table_name) else {
        return Vec::new();
    };
    let mut paths: Vec<String> = table
        .pairs::<String, Value>()
        .filter_map(Result::ok)
        .map(|(path, _)| path)
        .collect();
    paths.sort_unstable();
    paths.dedup();
    paths
}

/// Compatibilité historique : pilote exactement une frame, comme l’ancien `drive_menu`.
pub fn drive_menu(
    lua: &Lua,
    script_bytes: &[u8],
    name: &str,
    layer_ids: &[u32],
    item_counts: &BTreeMap<u32, i32>,
) -> Result<DriveReport, LuaError> {
    drive_menu_for_frames(lua, script_bytes, name, layer_ids, item_counts, 1)
}

#[cfg(test)]
mod dispatch_tests {
    use super::*;
    use crate::new_vm;
    use mlua::Function;

    fn host() -> (mlua::Lua, std::rc::Rc<std::cell::RefCell<MenuState>>) {
        let lua = new_vm();
        let state = install_menu_host(&lua).unwrap();
        (lua, state)
    }
    fn menu_cmd(lua: &mlua::Lua) -> Function {
        lua.globals().get("funcLuaMenuCommand").unwrap()
    }

    #[test]
    fn crc32_global_matches_level5_hashes() {
        let (lua, _) = host();
        let crc32: Function = lua.globals().get("CRC32").unwrap();
        let value: u32 = crc32.call("general_win").unwrap();
        assert_eq!(value, 0x1174_73AB);
    }

    #[test]
    fn command_names_cover_reversed_set() {
        assert_eq!(command_name(0x2A64_B198), Some("SetObjectVisible"));
        assert_eq!(command_name(0x5CE7_F1AE), Some("SetLayerActive"));
        assert_eq!(command_name(0xB641_D667), Some("GetObjectActive"));
        assert_eq!(command_name(0x65E8_25B1), Some("ApplyGlobalConfig(=>true)"));
        assert_eq!(command_name(0xC313_5B00), Some("ApplyQuery(=>true)"));
        assert_eq!(command_name(0xB9FF_F3C9), Some("ApplyDefault(=>true)"));
        assert_eq!(command_name(0x7457_8BF4), Some("SetGlobalFlag(=>1)"));
        assert_eq!(command_name(0xAA99_33B4), Some("ApplyReturn1(=>1)"));
        assert_eq!(command_name(0xDEAD_BEEF), None);
    }

    /// `ApplyGlobalConfig` (cmdId `0x65E825B1`) — handler `0x140CBF150` REVERSÉ (désassemblage
    /// nie.exe) : lit un octet de config global, l'applique, et renvoie **AL=1 inconditionnellement**.
    /// Appelé sans argument dans `OnInit` de main_menu. Le retour DOIT être `1.0` (le défaut getter
    /// `0.0` serait faux). C'est le 13ᵉ cmdId porté, le 1ᵉʳ reversé from-scratch ce cycle.
    #[test]
    fn apply_global_config_returns_true_per_reversed_handler() {
        let (lua, _state) = host();
        // Les 3 commandes « apply → true » reversées renvoient 1 (pas le défaut getter 0).
        for cid in [
            CMD_APPLY_GLOBAL_CONFIG_TRUE,
            CMD_APPLY_QUERY_TRUE,
            CMD_APPLY_DEFAULT_TRUE,
        ] {
            let ret: f64 = menu_cmd(&lua).call::<f64>((f64::from(cid),)).unwrap();
            assert_eq!(ret, 1.0, "cmdId 0x{cid:08X} : handler reversé renvoie AL=1");
        }
        // `SetGlobalFlag` (0x74578BF4, 1 arg) : handler 0x140CEECE0 reversé via la table de dispatch
        // → pose un flag moteur global et renvoie AL=1 (avec son arg bool). Appelé avec le bool.
        let ret: f64 = menu_cmd(&lua)
            .call::<f64>((f64::from(CMD_SET_GLOBAL_FLAG_TRUE), false))
            .unwrap();
        assert_eq!(
            ret, 1.0,
            "SetGlobalFlag(bool) : handler reversé renvoie AL=1"
        );
    }

    /// Batch « apply état moteur → return 1 » (12 cmdId reversés via la table de dispatch +
    /// désassemblage : `≤2 ret`, `mov al,1` dominant vérifié sur chacun). Chacun doit renvoyer 1.0
    /// (appelé avec un objId factice) et être nommé. Ancré sur le binaire, pas deviné.
    #[test]
    fn reversed_return1_batch_returns_one() {
        let (lua, _state) = host();
        for &cid in REVERSED_RETURN1 {
            assert_eq!(
                command_name(cid),
                Some("ApplyReturn1(=>1)"),
                "0x{cid:08X} nommé"
            );
            let ret: f64 = menu_cmd(&lua).call::<f64>((f64::from(cid), 1.0)).unwrap();
            assert_eq!(ret, 1.0, "cmdId 0x{cid:08X} : handler reversé renvoie AL=1");
        }
        assert_eq!(REVERSED_RETURN1.len(), 18);
    }

    /// Setters à garde d'arité reversés sur le binaire **COURANT** via le triage déterministe
    /// (`scripts/triage_funclua_handlers.py` : iced-x86 + bornes `.pdata` à chunks chaînés). Classe
    /// `RETURN_1_SAFE` = ret unique, aucune déf `al=0` dans le corps, déf finale `al=1` ⇒ renvoient
    /// 1.0 inconditionnellement. Chacun doit être nommé et renvoyer 1.0. Ancré sur le binaire.
    #[test]
    fn arg_guarded_return1_batch_returns_one() {
        let (lua, _state) = host();
        for &cid in ARG_GUARDED_RETURN1 {
            assert_eq!(
                command_name(cid),
                Some("ArgGuardedReturn1(=>1)"),
                "0x{cid:08X} nommé"
            );
            let ret: f64 = menu_cmd(&lua).call::<f64>((f64::from(cid), 1.0)).unwrap();
            assert_eq!(ret, 1.0, "cmdId 0x{cid:08X} : handler renvoie AL=1");
        }
        // 209 + les 6 du 7ᵉ lot (éditeur d'avatar). Le compte est verrouillé exprès : un ajout
        // doit être un geste délibéré, justifié au-dessus de l'entrée, jamais un effet de bord.
        assert_eq!(ARG_GUARDED_RETURN1.len(), 215);
    }

    /// `RegisterItemListCount` (cmdId `0x16C1C4C0`) — handler `0x140CD8E30` REVERSÉ : enregistre
    /// `object_attr[objId] = count` (arg3) dans le manager d'items que `GetObjectAttr` relit. Débloque
    /// `GetItemButtonNum`. Args réels confirmés `(2250456639, hash, 0, 8)` ⇒ `object_attr[…]=8`.
    #[test]
    fn register_item_list_count_sets_object_attr_from_script() {
        let (lua, state) = host();
        let f = menu_cmd(&lua);
        // RegisterItemListCount(objId=2250456639, hash, 0, count=8) -> 1
        let ret: f64 = f
            .call::<f64>((
                f64::from(CMD_REGISTER_ITEM_LIST_COUNT),
                2_250_456_639_f64,
                282_284_405_f64,
                0_f64,
                8_f64,
            ))
            .unwrap();
        assert_eq!(ret, 1.0, "handler renvoie al=1 (>=4 args)");
        // GetObjectAttr(objId) relit le count fourni par le script (mécanisme GetItemButtonNum).
        assert_eq!(state.borrow().object_attr.get(&2_250_456_639), Some(&8));
        // < 4 args -> al=0 (pas d'enregistrement).
        let ret0: f64 = f
            .call::<f64>((f64::from(CMD_REGISTER_ITEM_LIST_COUNT), 1_f64))
            .unwrap();
        assert_eq!(ret0, 0.0, "handler renvoie al=0 (<4 args)");
    }

    /// `SetSelectedIndex` (cmdId `0x6A06BC75`) — handler `0x140CE6B20` REVERSÉ : écrit l'index
    /// sélectionné/curseur `[obj+0x154]` d'une liste, renvoie al=1. Args réels title02 `(objId, 0, true)`.
    #[test]
    fn set_selected_index_sets_cursor_and_returns_true() {
        let (lua, state) = host();
        // établir le layer courant via SetLayerActive, puis SetSelectedIndex(objId, 3).
        menu_cmd(&lua)
            .call::<f64>((f64::from(CMD_SET_LAYER_ACTIVE), f64::from(0x77_u32), true))
            .unwrap();
        let ret: f64 = menu_cmd(&lua)
            .call::<f64>((
                f64::from(CMD_SET_SELECTED_INDEX),
                f64::from(0xABCD_u32),
                3_f64,
                true,
            ))
            .unwrap();
        assert_eq!(ret, 1.0, "handler renvoie al=1");
        assert_eq!(
            state.borrow().layers[&0x77].objects[&0xABCD].selected_index,
            Some(3),
            "index sélectionné stocké sur l'objet"
        );
    }

    /// SetLayerActive(layerId, active) : layout vérifié sur l'appel réel de title_menu_2
    /// (`0x5CE7F1AE(layer=0xE6EC6AA3, rest=[false])`). Met enabled ET le layer courant.
    #[test]
    fn set_layer_active_sets_current_and_enabled() {
        let (lua, state) = host();
        menu_cmd(&lua)
            .call::<f64>((f64::from(CMD_SET_LAYER_ACTIVE), f64::from(0x1234_u32), true))
            .unwrap();
        let st = state.borrow();
        assert_eq!(st.current_layer, 0x1234, "layer courant mis à jour");
        assert!(st.layers.get(&0x1234).unwrap().enabled);
    }

    /// SetObjectVisible(objId, index, visible) — PAS de layerId universel en position 1 :
    /// l'objet va dans le layer courant ; `visible` est en 3ᵉ position (après `index`).
    #[test]
    fn set_object_visible_uses_current_layer_and_index_offset() {
        let (lua, state) = host();
        let f = menu_cmd(&lua);
        // établir le layer courant
        f.call::<f64>((f64::from(CMD_SET_LAYER_ACTIVE), f64::from(0xAAAA_u32), true))
            .unwrap();
        // SetObjectVisible(objId=0xBEEF, index=0, visible=false)
        f.call::<f64>((
            f64::from(CMD_SET_OBJECT_VISIBLE),
            f64::from(0xBEEF_u32),
            0.0_f64,
            false,
        ))
        .unwrap();
        let st = state.borrow();
        let obj = st
            .layers
            .get(&0xAAAA)
            .unwrap()
            .objects
            .get(&0xBEEF)
            .unwrap();
        assert!(!obj.visible, "visible lu en position 3 (après index)");
    }

    /// SetSprite(objId, index, cellId, frame, color) — cellId/frame/color après l'index.
    #[test]
    fn set_sprite_layout() {
        let (lua, state) = host();
        menu_cmd(&lua)
            .call::<f64>((
                f64::from(CMD_SET_SPRITE),
                f64::from(0x10_u32), // objId
                1.0_f64,             // index
                f64::from(0x55_u32), // cellId
                3.0_f64,             // frame
                f64::from(0x77_u32), // color
            ))
            .unwrap();
        let st = state.borrow();
        let obj = st.layers.get(&0).unwrap().objects.get(&0x10).unwrap();
        assert_eq!(obj.sprite_texture_hash, Some(0x55));
        assert_eq!(obj.frame, Some(3));
        assert_eq!(obj.color_hash, Some(0x77));
    }

    /// `GetObjectAttr(objId)` renvoie le compte d'items renseigné dans `object_attr`
    /// (donnée de scène) ; 0 pour un layer non renseigné. C'est ce que `GetItemButtonNum`
    /// du title menu interroge pour connaître le nombre d'item-buttons d'un layer-list.
    #[test]
    fn get_object_attr_returns_scene_count() {
        let (lua, state) = host();
        // Renseigne la donnée de scène : layer 2250456639 a 8 items, 3873872512 en a 3.
        {
            let mut st = state.borrow_mut();
            st.set_object_attr(2_250_456_639, 8);
            st.set_object_attr(3_873_872_512, 3);
        }
        let f = menu_cmd(&lua);
        let main = f
            .call::<f64>((f64::from(CMD_GET_OBJECT_ATTR), 2_250_456_639.0_f64))
            .unwrap();
        let sub = f
            .call::<f64>((f64::from(CMD_GET_OBJECT_ATTR), 3_873_872_512.0_f64))
            .unwrap();
        let other = f
            .call::<f64>((f64::from(CMD_GET_OBJECT_ATTR), 1234.0_f64))
            .unwrap();
        assert_eq!(main, 8.0, "compte du layer-list principal");
        assert_eq!(sub, 3.0, "compte du layer-list secondaire");
        assert_eq!(other, 0.0, "layer non renseigné -> 0");
        assert!(
            state
                .borrow()
                .known_cmd_log
                .iter()
                .any(|(n, _)| n == "GetObjectAttr")
        );
    }

    /// `command_name` couvre les cmdId résiduels nouvellement reversés.
    #[test]
    fn command_names_cover_residual_set() {
        assert_eq!(command_name(0x214D_A123), Some("SetIconSprite"));
        assert_eq!(command_name(0xCAE6_622C), Some("SetPartEnabled"));
        assert_eq!(command_name(0xC1DE_BA99), Some("SetItemCount"));
        assert_eq!(command_name(0xD72B_5ED5), Some("SetNodeParam"));
        assert_eq!(command_name(0x2581_DC5C), Some("ObjectAction"));
    }

    /// SetIconSprite(objId, h1, …) — registre l'objet dans le layer courant et retient h1 comme
    /// hash de sprite. Layout vérifié sur l'appel réel `0x214DA123(objId, h1, h2, h3, 0, 0)`.
    #[test]
    fn set_icon_sprite_registers_object_and_sprite() {
        let (lua, state) = host();
        let f = menu_cmd(&lua);
        f.call::<f64>((f64::from(CMD_SET_LAYER_ACTIVE), f64::from(0xAAAA_u32), true))
            .unwrap();
        f.call::<f64>((
            f64::from(CMD_SET_ICON_SPRITE),
            f64::from(0x76A9_E67E_u32), // objId (arg0)
            f64::from(0xF53A_1234_u32), // h1 (arg1)
            f64::from(0x3D14_AAAA_u32), // h2
            f64::from(0x38E2_BBBB_u32), // h3
            0.0_f64,                    // n4
            0.0_f64,                    // index (arg5)
        ))
        .unwrap();
        let st = state.borrow();
        let obj = st
            .layers
            .get(&0xAAAA)
            .unwrap()
            .objects
            .get(&0x76A9_E67E)
            .unwrap();
        assert_eq!(
            obj.sprite_texture_hash,
            Some(0xF53A_1234),
            "h1 retenu comme sprite"
        );
        assert!(st.unknown_cmd_log.is_empty(), "cmd reversé ≠ inconnu");
    }

    /// SetItemCount(objId, count) — écrit `number` (DWORD [obj+0x148] du moteur). Layout vérifié
    /// sur l'appel réel `0xC1DEBA99(objId, 8)`.
    #[test]
    fn set_item_count_sets_number() {
        let (lua, state) = host();
        menu_cmd(&lua)
            .call::<f64>((
                f64::from(CMD_SET_ITEM_COUNT),
                f64::from(0x862A_u32),
                8.0_f64,
            ))
            .unwrap();
        let st = state.borrow();
        assert_eq!(
            st.layers
                .get(&0)
                .unwrap()
                .objects
                .get(&0x862A)
                .unwrap()
                .number,
            Some(8)
        );
    }

    /// SetPartEnabled(objId, index, partHash, enabled) — le fallback de granularité
    /// part suit les deux transitions `true` et `false`.
    #[test]
    fn set_part_enabled_hides_when_disabled() {
        let (lua, state) = host();
        let f = menu_cmd(&lua);
        // enabled=true : objet enregistré, reste visible
        f.call::<f64>((
            f64::from(CMD_SET_PART_ENABLED),
            f64::from(0x111_u32),
            0.0_f64,
            f64::from(0x3014_u32),
            true,
        ))
        .unwrap();
        // enabled=false : masque
        f.call::<f64>((
            f64::from(CMD_SET_PART_ENABLED),
            f64::from(0x222_u32),
            0.0_f64,
            f64::from(0x3014_u32),
            false,
        ))
        .unwrap();
        // Le même objet doit pouvoir être réaffiché après un masquage.
        f.call::<f64>((
            f64::from(CMD_SET_PART_ENABLED),
            f64::from(0x222_u32),
            0.0_f64,
            f64::from(0x3014_u32),
            true,
        ))
        .unwrap();
        let st = state.borrow();
        let objs = &st.layers.get(&0).unwrap().objects;
        assert!(objs.get(&0x111).unwrap().visible, "part activée -> visible");
        assert!(
            objs.get(&0x222).unwrap().visible,
            "part réactivée -> visible"
        );
    }

    #[test]
    fn kizuna_part_commands_conservent_visibilite_et_rgba_flottant() {
        let (lua, state) = host();
        let f = menu_cmd(&lua);
        assert_eq!(
            f.call::<f64>((
                f64::from(CMD_KIZUNA_SET_PART_VISIBLE),
                f64::from(0x111_u32),
                f64::from(0x222_u32),
                false,
            ))
            .unwrap(),
            1.0
        );
        assert_eq!(
            f.call::<f64>((
                f64::from(CMD_KIZUNA_SET_PART_COLOR),
                f64::from(0x111_u32),
                3.0_f64,
                f64::from(0x222_u32),
                0.1_f64,
                0.2_f64,
                0.3_f64,
                0.4_f64,
            ))
            .unwrap(),
            1.0
        );
        {
            let object = &state.borrow().layers[&0].objects[&0x111];
            assert_eq!(object.part_visible.get(&0x222), Some(&false));
            assert_eq!(object.part_color_rgba[&0x222], [0.1, 0.2, 0.3, 0.4]);
        }
        for command in [
            CMD_KIZUNA_APPLY_PART_FLAGS,
            CMD_KIZUNA_SET_PART_PARAM,
            CMD_KIZUNA_SET_PART_TEXTURE,
        ] {
            assert_eq!(
                f.call::<f64>((
                    f64::from(command),
                    f64::from(0x111_u32),
                    3.0_f64,
                    f64::from(0x222_u32),
                    0xAABB_CCDD_u64 as f64,
                ))
                .unwrap(),
                1.0
            );
            let object = &state.borrow().layers[&0].objects[&0x111];
            let args = match command {
                CMD_KIZUNA_APPLY_PART_FLAGS => &object.part_flag_args,
                CMD_KIZUNA_SET_PART_PARAM => &object.part_param_args,
                CMD_KIZUNA_SET_PART_TEXTURE => &object.part_texture_args,
                _ => unreachable!(),
            };
            assert_eq!(args.get(&0x222), Some(&vec![3, 0x222, 0xAABB_CCDD]));
        }
        assert!(state.borrow().unknown_cmd_log.is_empty());
    }

    /// SetObjectActive(objId, active) — écrit le champ `active`. Layout vérifié sur
    /// `0xD1B51DF0(objId, false)`.
    #[test]
    fn set_object_active_toggles_active() {
        let (lua, state) = host();
        menu_cmd(&lua)
            .call::<f64>((
                f64::from(CMD_SET_OBJECT_ACTIVE_S),
                f64::from(0x862A_u32),
                false,
            ))
            .unwrap();
        let st = state.borrow();
        assert!(
            !st.layers
                .get(&0)
                .unwrap()
                .objects
                .get(&0x862A)
                .unwrap()
                .active
        );
    }

    /// SetNodeParam / ObjectAction : référencent l'objet (le runtime l'a piloté) sans inventer de
    /// champ, et sont journalisés comme connus (plus dans `unknown_cmd_log`).
    #[test]
    fn node_param_and_action_register_object_only() {
        let (lua, state) = host();
        let f = menu_cmd(&lua);
        f.call::<f64>((
            f64::from(CMD_NODE_PARAM),
            f64::from(0x304_u32),
            0.0_f64,
            0.0_f64,
            0.0_f64,
        ))
        .unwrap();
        f.call::<f64>((f64::from(CMD_OBJECT_ACTION), f64::from(0x1793_u32)))
            .unwrap();
        let st = state.borrow();
        assert!(st.layers.get(&0).unwrap().objects.contains_key(&0x304));
        assert!(st.layers.get(&0).unwrap().objects.contains_key(&0x1793));
        assert!(st.unknown_cmd_log.is_empty(), "cmd reversés ≠ inconnus");
        assert!(st.known_cmd_log.iter().any(|(n, _)| n == "SetNodeParam"));
        assert!(st.known_cmd_log.iter().any(|(n, _)| n == "ObjectAction"));
    }

    /// `command_name` couvre la vague 3 de cmdId résiduels (title + mainmenu).
    #[test]
    fn command_names_cover_wave3_set() {
        // title
        assert_eq!(command_name(0x988B_5B82), Some("SetObjectValue"));
        assert_eq!(command_name(0x513C_6C70), Some("SetItemParam"));
        assert_eq!(command_name(0xFC56_9E77), Some("SetSubObjectEnabled"));
        assert_eq!(command_name(0x9CAB_2E41), Some("SetNodeIndex"));
        assert_eq!(command_name(0x4BE9_C865), Some("ObjectActionById"));
        // mainmenu
        assert_eq!(command_name(0x8B1D_38C4), Some("SetNodeValue"));
        assert_eq!(command_name(0xBE2A_7145), Some("SetNodeParamBlock"));
        assert_eq!(command_name(0x2044_7515), Some("SetPartParamI"));
        assert_eq!(command_name(0x5F21_01DB), Some("SetPartParamF"));
        assert_eq!(command_name(0x80AB_69F3), Some("SetSubNodeEnabled"));
        assert_eq!(command_name(0x816C_D673), Some("SetObjectFlag"));
        assert_eq!(command_name(0x1AF6_1E89), Some("SetListItemValues"));
        assert_eq!(command_name(0x83B4_F0AC), Some("SetListItemValuesMulti"));
        assert_eq!(command_name(0x06B1_9AFF), Some("GetNodeIndexByHash"));
    }

    /// SetObjectValue(objId, valueHash, flag) — enregistre l'objet dans le layer courant et écrit
    /// `value`. Layout vérifié sur l'appel réel `0x988B5B82(0x5AFCAA78, 0xCB147A28, false)`.
    #[test]
    fn set_object_value_registers_and_sets_value() {
        let (lua, state) = host();
        let f = menu_cmd(&lua);
        f.call::<f64>((f64::from(CMD_SET_LAYER_ACTIVE), f64::from(0xAAAA_u32), true))
            .unwrap();
        f.call::<f64>((
            f64::from(CMD_SET_OBJECT_VALUE),
            f64::from(0x5AFC_AA78_u32),
            f64::from(0xCB14_7A28_u32),
            false,
        ))
        .unwrap();
        let st = state.borrow();
        let obj = st
            .layers
            .get(&0xAAAA)
            .unwrap()
            .objects
            .get(&0x5AFC_AA78)
            .unwrap();
        assert_eq!(
            obj.value,
            Some(0xCB14_7A28_u32 as i32),
            "valueHash retenu dans `value`"
        );
        assert!(st.unknown_cmd_log.is_empty(), "cmd reversé ≠ inconnu");
    }

    /// SetSubObjectEnabled(objId, index, enabled) — le fallback de granularité
    /// suit les deux transitions `true` et `false`.
    #[test]
    fn set_subobject_enabled_hides_when_disabled() {
        let (lua, state) = host();
        let f = menu_cmd(&lua);
        f.call::<f64>((
            f64::from(CMD_SET_SUBOBJECT_ENABLED),
            f64::from(0x111_u32),
            0.0_f64,
            true,
        ))
        .unwrap();
        f.call::<f64>((
            f64::from(CMD_SET_SUBOBJECT_ENABLED),
            f64::from(0xC88D_6DB6_u32),
            0.0_f64,
            false,
        ))
        .unwrap();
        f.call::<f64>((
            f64::from(CMD_SET_SUBOBJECT_ENABLED),
            f64::from(0xC88D_6DB6_u32),
            0.0_f64,
            true,
        ))
        .unwrap();
        let st = state.borrow();
        let objs = &st.layers.get(&0).unwrap().objects;
        assert!(objs.get(&0x111).unwrap().visible, "activé -> visible");
        assert!(
            objs.get(&0xC88D_6DB6).unwrap().visible,
            "réactivé -> visible"
        );
    }

    /// SetListItemValues(layerId, objId, <table>) — arg0 = layerId (PAS objId) ; l'objet-liste est
    /// enregistré dans le layer résolu ET ses sous-items peuplés (1 colonne). Layout vérifié sur
    /// l'appel réel `0x1AF61E89(0x9DB608F1, 0x43DCD9A7, <table>)` (main_menu_inc/soccer_top_menu_inc).
    #[test]
    fn set_list_item_values_populates_sub_items_single_column() {
        let (lua, state) = host();
        let tbl = lua.create_table().unwrap();
        // {1981080283, 3421673595} : valeurs par item (item 0, item 1).
        tbl.set(1, 1_981_080_283.0_f64).unwrap();
        tbl.set(2, 3_421_673_595.0_f64).unwrap();
        menu_cmd(&lua)
            .call::<f64>((
                f64::from(CMD_SET_LIST_ITEM_VALUES),
                f64::from(0x9DB6_08F1_u32), // layerId (arg0)
                f64::from(0x43DC_D9A7_u32), // objId (arg1)
                tbl,
            ))
            .unwrap();
        let st = state.borrow();
        let layer = st.layers.get(&0x9DB6_08F1).expect("layer = arg0");
        let obj = layer
            .objects
            .get(&0x43DC_D9A7)
            .expect("objId = arg1 enregistré dans layer arg0");
        assert_eq!(obj.sub_items.len(), 2, "2 items enregistrés");
        assert_eq!(
            obj.sub_items.get(&0).unwrap().values,
            vec![1_981_080_283_u32 as i32]
        );
        assert_eq!(
            obj.sub_items.get(&1).unwrap().values,
            vec![3_421_673_595_u32 as i32]
        );
        assert!(st.unknown_cmd_log.is_empty(), "cmd reversé ≠ inconnu");
        assert!(
            st.known_cmd_log
                .iter()
                .any(|(n, _)| n == "SetListItemValues")
        );
    }

    /// SetListItemValuesMulti(layerId, objId, t0, t1, t2) — N tables PARALLÈLES : item i =
    /// [t0[i], t1[i], t2[i]]. Layout vérifié sur soccer_top_menu_inc
    /// `funcLuaMenuCommand(0x83B4F0AC, layer, obj, L4, L5, L6, L7, L8)` (colonnes par item).
    #[test]
    fn set_list_item_values_multi_populates_parallel_columns() {
        let (lua, state) = host();
        let mk = |a: &[i64]| {
            let t = lua.create_table().unwrap();
            for (i, v) in a.iter().enumerate() {
                t.set(i as i64 + 1, *v as f64).unwrap();
            }
            t
        };
        // 2 items : item0 = [1847312674, 2750714971, 0], item1 = [1723685574, 3794706590, 18].
        let t0 = mk(&[1_847_312_674, 1_723_685_574]);
        let t1 = mk(&[2_750_714_971, 3_794_706_590]);
        let t2 = mk(&[0, 18]);
        menu_cmd(&lua)
            .call::<f64>((
                f64::from(CMD_SET_LIST_ITEM_VALUES_MULTI),
                f64::from(0xBBBB_u32),      // layerId
                f64::from(0x4666_8627_u32), // objId
                t0,
                t1,
                t2,
            ))
            .unwrap();
        let st = state.borrow();
        let obj = st
            .layers
            .get(&0xBBBB)
            .unwrap()
            .objects
            .get(&0x4666_8627)
            .unwrap();
        assert_eq!(obj.sub_items.len(), 2, "2 items");
        assert_eq!(
            obj.sub_items.get(&0).unwrap().values,
            vec![1_847_312_674_u32 as i32, 2_750_714_971_u32 as i32, 0],
            "item 0 = colonne par colonne"
        );
        assert_eq!(
            obj.sub_items.get(&1).unwrap().values,
            vec![1_723_685_574, 3_794_706_590_u32 as i32, 18],
            "item 1 = colonne par colonne"
        );
        assert!(
            st.known_cmd_log
                .iter()
                .any(|(n, _)| n == "SetListItemValuesMulti")
        );
    }

    /// SetItemParam(objId, itemIndex, key, value) — paramètre keyé enregistré dans le sous-item
    /// `itemIndex`. Layout vérifié sur `title_menu_2::SwapTitleBannerCapture`
    /// (`0x513C6C70(objId, itemIndex, key, value)`).
    #[test]
    fn set_item_param_records_keyed_sub_item() {
        let (lua, state) = host();
        let f = menu_cmd(&lua);
        f.call::<f64>((f64::from(CMD_SET_LAYER_ACTIVE), f64::from(0xCCCC_u32), true))
            .unwrap();
        // item 2, key 0x1234 -> value 7 ; item 2, key 0x5678 -> value 9.
        f.call::<f64>((
            f64::from(CMD_SET_ITEM_PARAM),
            f64::from(0x77_u32),
            2.0,
            f64::from(0x1234_u32),
            7.0,
        ))
        .unwrap();
        f.call::<f64>((
            f64::from(CMD_SET_ITEM_PARAM),
            f64::from(0x77_u32),
            2.0,
            f64::from(0x5678_u32),
            9.0,
        ))
        .unwrap();
        let st = state.borrow();
        let obj = st.layers.get(&0xCCCC).unwrap().objects.get(&0x77).unwrap();
        let item = obj.sub_items.get(&2).expect("item index 2");
        assert_eq!(item.params.get(&0x1234), Some(&7));
        assert_eq!(item.params.get(&0x5678), Some(&9));
        assert!(st.unknown_cmd_log.is_empty(), "cmd reversé ≠ inconnu");
        assert!(st.known_cmd_log.iter().any(|(n, _)| n == "SetItemParam"));
    }

    /// Les setters de sous-nœud/part de mainmenu enregistrent l'objet (objId = arg0) dans le layer
    /// courant sans inventer de champ, et sortent du journal des inconnus. Vérifié sur les appels
    /// réels `0x8B1D38C4(0x87DC9C59, …)`, `0x20447515(0xD5A761D9, …)`, `0x5F2101DB(0xD5A761D9, …)`.
    #[test]
    fn mainmenu_part_setters_register_object() {
        let (lua, state) = host();
        let f = menu_cmd(&lua);
        f.call::<f64>((f64::from(CMD_SET_LAYER_ACTIVE), f64::from(0xBBBB_u32), true))
            .unwrap();
        f.call::<f64>((
            f64::from(CMD_SET_NODE_VALUE),
            f64::from(0x87DC_9C59_u32),
            0.0,
            1.0,
            2.0,
        ))
        .unwrap();
        f.call::<f64>((
            f64::from(CMD_SET_PART_PARAM_I),
            f64::from(0xD5A7_61D9_u32),
            1.0,
            0.0,
            0.0,
        ))
        .unwrap();
        f.call::<f64>((
            f64::from(CMD_SET_PART_PARAM_F),
            f64::from(0xD5A7_61D9_u32),
            1.0,
            0.0,
            0.24_f64,
        ))
        .unwrap();
        // SetElementColor (0x2FC47DA5) — setter de couleur RGBA reversé via la table de dispatch
        // (0x140CC33F0) : sous-élément + canaux non modélisés -> l'objet est enregistré.
        f.call::<f64>((
            f64::from(CMD_SET_ELEMENT_COLOR),
            f64::from(0xC0C0_C0C0_u32),
            0.0,
            1.0_f64,
            0.0,
            0.627_f64,
            0.941_f64,
        ))
        .unwrap();
        let st = state.borrow();
        let objs = &st.layers.get(&0xBBBB).unwrap().objects;
        assert!(
            objs.contains_key(&0x87DC_9C59),
            "SetNodeValue enregistre l'objet"
        );
        assert!(
            objs.contains_key(&0xD5A7_61D9),
            "SetPartParamI/F enregistrent l'objet"
        );
        assert!(
            objs.contains_key(&0xC0C0_C0C0),
            "SetElementColor enregistre l'objet"
        );
        assert_eq!(command_name(CMD_SET_ELEMENT_COLOR), Some("SetElementColor"));
        assert!(st.unknown_cmd_log.is_empty(), "cmds reversés ≠ inconnus");
    }

    /// GetNodeIndexByHash(objId, index, hash) — getter reversé : renvoie 0 (lookup moteur non
    /// simulé) sans crash et journalisé comme connu.
    #[test]
    fn get_node_index_by_hash_returns_default() {
        let (lua, state) = host();
        let r = menu_cmd(&lua)
            .call::<f64>((
                f64::from(CMD_GET_NODE_INDEX_BY_HASH),
                f64::from(0xD5A7_61D9_u32),
                0.0_f64,
                1.0_f64,
            ))
            .unwrap();
        assert_eq!(r, 0.0);
        assert!(
            state
                .borrow()
                .known_cmd_log
                .iter()
                .any(|(n, _)| n == "GetNodeIndexByHash")
        );
        assert!(
            state.borrow().unknown_cmd_log.is_empty(),
            "getter reversé ≠ inconnu"
        );
    }

    /// `enumerate_header_tabs` appelle les VRAIES fonctions du script (GetSortOfTabs /
    /// GetMenuObjectNameFromTabType / GetTabTextIdCRC) et force le mode NORMAL (corrige le stub
    /// truthy-0 qui rendrait IsChronicleModeMainMenu vrai). On reproduit ici la structure exacte
    /// du `main_menu_1.02.92.00` (table normale 9 onglets {10,20,30,70,40,80,60,50,90}).
    #[test]
    fn enumerate_header_tabs_returns_normal_nine_tabs() {
        let lua = new_vm();
        let _ = install_menu_host(&lua).unwrap();
        lua.load(
            r#"
            MAIN_MENU = {}
            -- 0.0 (stub) est truthy en Lua -> sans le forçage, mode chronicle (5 onglets).
            function MAIN_MENU.IsChronicleModeMainMenu() return funcLuaCommand(1) end
            local NORMAL = {10,20,30,70,40,80,60,50,90}
            local CHRON  = {10,20,60,50,90}
            function GetSortOfTabs()
              if MAIN_MENU.IsChronicleModeMainMenu() then return CHRON end
              return NORMAL
            end
            function GetMenuObjectNameFromTabType(t)
              local m = {[10]=1730641026,[20]=3040181864,[30]=3607292371,[40]=283631169,
                         [50]=3165722229,[60]=948899592,[70]=1598948986,[80]=37450337,[90]=613075359}
              return m[t] or 0
            end
            function GetTabTextIdCRC(t)
              local m = {[10]=2074787642,[20]=1059912976,[30]=3955157337,[40]=2378443629,
                         [50]=1689681834,[60]=682656930,[70]=3095252521,[80]=1629403576,[90]=3754313428}
              return m[t] or 0
            end
        "#,
        )
        .exec()
        .unwrap();

        let tabs = enumerate_header_tabs(&lua);
        assert_eq!(
            tabs.len(),
            9,
            "mode normal forcé -> 9 onglets (pas les 5 chronicle)"
        );
        // Ordre = GetSortOfTabs.
        let types: Vec<i64> = tabs.iter().map(|t| t.tab_type).collect();
        assert_eq!(types, vec![10, 20, 30, 70, 40, 80, 60, 50, 90]);
        // Hashes d'objet/texte conformes au décompilé.
        assert_eq!(tabs[0].obj_hash, 1_730_641_026);
        assert_eq!(tabs[0].text_id, 2_074_787_642);
        assert_eq!(tabs[0].index, 0);
        assert_eq!(tabs[8].obj_hash, 613_075_359); // type 90
    }

    /// `enumerate_header_tabs` renvoie un vec vide pour un écran SANS onglets (fonctions absentes),
    /// rendant l'appel sûr pour n'importe quel menu (title, option, …).
    #[test]
    fn enumerate_header_tabs_empty_without_tab_functions() {
        let lua = new_vm();
        let _ = install_menu_host(&lua).unwrap();
        assert!(enumerate_header_tabs(&lua).is_empty());
    }

    // ── Identifiants de scénario relevés dans les scripts réels ───────────────
    //
    // Ces valeurs venaient des tests C# (`LuaRuntimeTests.cs`, `GameLuaHostTests.cs`), qui ne
    // s'exécutaient que sur une machine portant `re/lua/raw` + `unluac.jar` +
    // `re/menu/hash-dictionary.json` — trois chemins absents du dépôt. Elles sont reportées ici
    // pour survivre au retrait de `csharp/`.
    //
    // Deux natures **à ne pas confondre** :
    //  - un layerId qui EST le CRC32 de son nom de layer (vérifiable sans aucun dictionnaire) ;
    //  - une constante simplement OBSERVÉE dans le décompilé, dont le nom reste inconnu.

    /// `general_win` — layerId de `qrcode_menu.lua.bin`.
    const LAYER_GENERAL_WIN: u32 = 292_844_459;
    /// layerId de `savedata_management_menu_save_and_upload.lua.bin`, passé à `OnChangeLayerGroup`.
    const LAYER_SAVEDATA_GROUPE: u32 = 1_654_568_798;
    /// layerId passé à `OnSetupLayer` de `battle_menu_multi.lua.bin`. **Observé**, pas un hash de
    /// nom : `CRC32("battle_menu_multi") == 0xFEB5F0B8`, ce qui ne correspond pas.
    const LAYER_BATTLE_MENU_MULTI: u32 = 2_492_438_505;
    /// layerId passé à `OnOpenLayer` de `savedata_management_menu_save_and_upload.lua.bin`.
    /// **Observé**, pas un hash de nom.
    const LAYER_SAVEDATA_OUVERTURE: u32 = 536_044_352;

    /// Le layerId d'un layer nommé est le hash Level-5 de son nom — reproductible **sans** le
    /// `hash-dictionary.json` dont dépendaient les tests C#.
    #[test]
    fn les_layer_ids_nommes_sont_le_crc32_de_leur_nom() {
        assert_eq!(
            nie_formats::cfgbin::crc32(b"general_win"),
            LAYER_GENERAL_WIN,
            "general_win = 0x117473AB"
        );
        assert_eq!(nie_formats::cfgbin::crc32(b"general_win"), 0x1174_73AB);
        assert_eq!(
            nie_formats::cfgbin::crc32(b"savedata_management_menu_save_and_upload"),
            LAYER_SAVEDATA_GROUPE,
        );
    }

    /// Les deux autres layerIds sont des constantes observées : l'affirmer explicitement évite
    /// qu'on les « explique » un jour par un hash de nom qui ne colle pas.
    #[test]
    fn les_layer_ids_observes_ne_sont_pas_des_hash_de_nom() {
        assert_ne!(
            nie_formats::cfgbin::crc32(b"battle_menu_multi"),
            LAYER_BATTLE_MENU_MULTI,
            "CRC32(\"battle_menu_multi\") vaut 0xFEB5F0B8 : le layerId observé a une autre origine"
        );
        assert_eq!(LAYER_BATTLE_MENU_MULTI, 0x948F_97E9);
        assert_eq!(LAYER_SAVEDATA_OUVERTURE, 0x1FF3_6340);
    }

    /// Les deux cmdId émis par `savedata_management_menu_save_and_upload` : l'un est reversé et
    /// nommé, l'autre pas encore.
    #[test]
    fn les_cmd_ids_du_scenario_savedata() {
        // 711242136 = 0x2A64B198 — le seul des deux qui soit reversé.
        assert_eq!(CMD_SET_OBJECT_VISIBLE, 711_242_136);
        assert_eq!(command_name(711_242_136), Some("SetObjectVisible"));
        // 532421851 = 0x1FBC1CDB — émis par OnChangeLayerGroup, handler NON reversé à ce jour.
        // Le jour où il le sera, ce test doit être inversé : c'est la trace du travail restant.
        assert_eq!(0x1FBC_1CDB_u32, 532_421_851);
        assert_eq!(
            command_name(532_421_851),
            None,
            "0x1FBC1CDB non reversé : inverser cette assertion une fois le handler identifié"
        );
        // `SetSprite`, cité par les mêmes tests C#, est lui reversé.
        assert_eq!(CMD_SET_SPRITE, 3_781_155_141);
        assert_eq!(command_name(0xE15F_D945), Some("SetSprite"));
    }

    /// `GetObjectActive` lit l'état posé par `SetObjectActive` au lieu de renvoyer
    /// systématiquement le défaut `0.0`.
    #[test]
    fn get_object_active_reflects_setter() {
        let (lua, state) = host();
        let f = menu_cmd(&lua);
        let obj = f64::from(0x1_u32);
        assert_eq!(
            f.call::<f64>((f64::from(CMD_GET_OBJECT_ACTIVE), obj))
                .unwrap(),
            0.0
        );
        f.call::<f64>((f64::from(CMD_SET_OBJECT_ACTIVE_S), obj, false))
            .unwrap();
        assert_eq!(
            f.call::<f64>((f64::from(CMD_GET_OBJECT_ACTIVE), obj))
                .unwrap(),
            0.0
        );
        f.call::<f64>((f64::from(CMD_SET_OBJECT_ACTIVE_S), obj, true))
            .unwrap();
        assert_eq!(
            f.call::<f64>((f64::from(CMD_GET_OBJECT_ACTIVE), obj))
                .unwrap(),
            1.0
        );
        assert!(
            state
                .borrow()
                .known_cmd_log
                .iter()
                .any(|(n, _)| n == "GetObjectActive")
        );
        assert!(
            state.borrow().unknown_cmd_log.is_empty(),
            "getter reversé ≠ inconnu"
        );
    }

    #[test]
    fn general_get_text_reads_runtime_table() {
        let (lua, state) = host();
        state
            .borrow_mut()
            .set_text(0x1234_5678, "Jouer".to_string());
        let get_text: Function = lua.globals().get("funcLuaCommand").unwrap();
        let value: String = get_text
            .call((f64::from(CMD_GENERAL_GET_TEXT), f64::from(0x1234_5678_u32)))
            .unwrap();
        assert_eq!(value, "Jouer");
        let current_value: String = get_text
            .call((
                f64::from(CMD_GENERAL_GET_TEXT_CURRENT),
                f64::from(0x1234_5678_u32),
            ))
            .unwrap();
        assert_eq!(current_value, "Jouer");
        let missing: String = get_text
            .call((f64::from(CMD_GENERAL_GET_TEXT), f64::from(0xDEAD_BEEFu32)))
            .unwrap();
        assert!(missing.is_empty());
    }

    #[test]
    fn general_condition_and_list_queries_use_injected_runtime_state() {
        let (lua, state) = host();
        state.borrow_mut().set_condition(0xCAFE, true);
        state.borrow_mut().set_list_count(0xBEEF, 7, true);
        state.borrow_mut().set_resource_available(0xFACE, true);
        let command: Function = lua.globals().get("funcLuaCommand").unwrap();

        let active: bool = command
            .call((
                f64::from(CMD_GENERAL_IS_CONDITION_ACTIVE),
                f64::from(0xCAFE_u32),
            ))
            .unwrap();
        assert!(active);

        let (count, resolved): (i64, bool) = command
            .call((f64::from(CMD_GENERAL_GET_LIST_COUNT), f64::from(0xBEEF_u32)))
            .unwrap();
        assert_eq!((count, resolved), (7, true));

        let available: bool = command
            .call((
                f64::from(CMD_GENERAL_RESOURCE_AVAILABLE),
                f64::from(0xFACE_u32),
            ))
            .unwrap();
        assert!(available);
        let absent: bool = command
            .call((
                f64::from(CMD_GENERAL_RESOURCE_AVAILABLE),
                f64::from(0xDEAD_u32),
            ))
            .unwrap();
        assert!(!absent);

        let apply_effect: f64 = command
            .call((f64::from(CMD_GENERAL_APPLY_UI_EFFECT), 0x1234_u32))
            .unwrap();
        assert_eq!(apply_effect, 1.0);
        let empty_effect: f64 = command
            .call(f64::from(CMD_GENERAL_APPLY_UI_EFFECT))
            .unwrap();
        assert_eq!(empty_effect, 0.0);
        let apply_state: f64 = command
            .call((f64::from(CMD_GENERAL_APPLY_STATE), 0xCAFE_u32))
            .unwrap();
        assert_eq!(apply_state, 1.0);

        let set_global_int: f64 = command
            .call((f64::from(CMD_GENERAL_SET_GLOBAL_INT), 7_i32))
            .unwrap();
        assert_eq!(set_global_int, 1.0);
        assert_eq!(state.borrow().engine_int_2728, Some(7));
        let empty_global_int: f64 = command.call(f64::from(CMD_GENERAL_SET_GLOBAL_INT)).unwrap();
        assert_eq!(empty_global_int, 0.0);

        let menu_command: Function = lua.globals().get("funcLuaMenuCommand").unwrap();
        let kizuna_reset: f64 = menu_command.call(f64::from(CMD_KIZUNA_TOWN_RESET)).unwrap();
        assert_eq!(kizuna_reset, 1.0);

        let unknown: f64 = command.call((1.0_f64,)).unwrap();
        assert_eq!(unknown, 0.0);
        assert!(
            state
                .borrow()
                .unknown_general_cmd_log
                .iter()
                .any(|(id, _, _)| *id == 1)
        );
        assert!(state.borrow().unknown_cmd_log.is_empty());
    }

    #[test]
    fn general_kizuna_handlers_reproduisent_leurs_retours_re() {
        let (lua, state) = host();
        state.borrow_mut().set_condition(0x0BAD, true);
        let command: Function = lua.globals().get("funcLuaCommand").unwrap();

        let a: bool = command
            .call((f64::from(CMD_GENERAL_QUERY_BOOL_A), f64::from(0x0BAD_u32)))
            .unwrap();
        assert!(a);
        let b: bool = command
            .call((
                f64::from(CMD_GENERAL_QUERY_BOOL_B),
                f64::from(0x0BAD_u32),
                0_i32,
                0_i32,
            ))
            .unwrap();
        assert!(b);
        let (ok, value): (bool, i64) = command
            .call((f64::from(CMD_GENERAL_QUERY_STATE), 1_i32, 0_i32, 13_i32))
            .unwrap();
        assert!(ok);
        assert_eq!(value, 0);
        let applied: bool = command
            .call(f64::from(CMD_GENERAL_APPLY_CURRENT_STATE))
            .unwrap();
        assert!(applied);
        assert_eq!(state.borrow().unknown_general_cmd_log.len(), 0);
    }

    #[test]
    fn drive_menu_for_frames_keeps_the_same_lua_vm() {
        let lua = new_vm();
        let bytes = lua
            .load("frames = 0; function Step() frames = frames + 1 end")
            .into_function()
            .unwrap()
            .dump(false);
        let item_counts = BTreeMap::new();
        let report = drive_menu_for_frames(&lua, &bytes, "frames", &[], &item_counts, 3).unwrap();
        assert!(report.top_level_ok);
        assert_eq!(lua.globals().get::<i64>("frames").unwrap(), 3);
    }

    #[test]
    fn drive_menu_for_frames_zero_navance_pas_le_cycle() {
        let lua = new_vm();
        let bytes = lua
            .load("frames = 0; function Step() frames = frames + 1 end")
            .into_function()
            .unwrap()
            .dump(false);
        drive_menu_for_frames(&lua, &bytes, "zero-frames", &[], &BTreeMap::new(), 0).unwrap();
        assert_eq!(lua.globals().get::<i64>("frames").unwrap(), 0);
    }

    #[test]
    fn drive_menu_for_frames_orders_pre_step_step_post_step() {
        let lua = new_vm();
        let bytes = lua
            .load(
                r#"trace = ""
                   function PreStep() trace = trace .. "P" end
                   function Step() trace = trace .. "S" end
                   function PostStep() trace = trace .. "T" end"#,
            )
            .into_function()
            .unwrap()
            .dump(false);
        let report =
            drive_menu_for_frames(&lua, &bytes, "step-order", &[], &BTreeMap::new(), 2).unwrap();
        assert!(report.callbacks.contains(&"PreStep".to_string()));
        assert!(report.callbacks.contains(&"PostStep".to_string()));
        assert_eq!(lua.globals().get::<String>("trace").unwrap(), "PSTPST");
    }

    #[test]
    fn drive_menu_for_frames_expose_les_erreurs_de_callbacks() {
        let lua = new_vm();
        let bytes = lua
            .load(
                r#"function OnInit() error("init cassée") end
                   function Step() error("step cassée") end"#,
            )
            .into_function()
            .unwrap()
            .dump(false);
        let report =
            drive_menu_for_frames(&lua, &bytes, "callback-errors", &[], &BTreeMap::new(), 1)
                .unwrap();
        assert_eq!(report.on_init, Some(false));
        assert!(report.callback_errors.iter().any(|e| e.contains("OnInit")));
        assert!(report.callback_errors.iter().any(|e| e.contains("Step")));
    }

    #[test]
    fn drive_menu_for_frames_trace_les_methodes_moteur_non_portees() {
        let lua = new_vm();
        let _state = install_menu_host(&lua).unwrap();
        let bytes = lua
            .load(
                r#"function OnInit()
                         return GENERAL_WINDOW.Open(7)
                   end"#,
            )
            .into_function()
            .unwrap()
            .dump(false);
        let report =
            drive_menu_for_frames(&lua, &bytes, "host-paths", &[], &BTreeMap::new(), 0).unwrap();
        assert!(
            !report
                .missing_host_calls
                .contains(&"GENERAL_WINDOW".to_string())
        );
        assert!(
            report
                .missing_host_paths
                .contains(&"GENERAL_WINDOW.Open".to_string())
        );
        assert!(
            report
                .missing_host_paths
                .contains(&"GENERAL_WINDOW.Open()".to_string())
        );
    }
}
