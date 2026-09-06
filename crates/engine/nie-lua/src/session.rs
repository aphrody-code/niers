//! **Session Lua persistante** — VM qui vit entre les appels, comportements attachés, rechargement.
//!
//! ## Le problème que ça règle
//!
//! [`crate::runtime::execute`] crée une VM neuve à chaque appel. C'est la bonne propriété pour
//! *analyser* un script (deux analyses ne se contaminent pas), mais la mauvaise pour *travailler
//! avec* : une console où `x = 1` puis `x` répond `nil` n'est pas une console, et réexécuter tout
//! le script à chaque expression évaluée est aussi lent qu'incorrect.
//!
//! [`LuaSession`] garde la VM vivante : l'état survit d'une évaluation à l'autre, et le
//! rechargement est **explicite**.
//!
//! ## Ce qui vient d'Overload
//!
//! - **Rechargement par recréation du contexte.** `ScriptInterpreter::RefreshAll()` détruit puis
//!   recrée le `sol::state` entier, avec ce constat en commentaire : *« unconsidering a script is
//!   impossible with Lua, we have to reparse every behaviours »*. C'est exact — Lua n'a pas de
//!   « désenregistrer » : une fonction globale posée par un script reste après modification du
//!   fichier. [`LuaSession::reload`] fait donc la même chose : VM neuve, binders réinstallés,
//!   comportements ré-attachés.
//! - **Contrat d'attachement.** Chez Overload, un `Behaviour` charge `<nom>.lua`, **exige que le
//!   script retourne une table**, et y injecte `owner`. On reprend ce contrat ([`Behaviour`]).
//! - **Callback absent = ignoré silencieusement.** Overload appelle `OnStart`/`OnUpdate`/… et ne
//!   se plaint pas si la fonction n'existe pas. Indispensable : aucun script ne définit tous les
//!   points d'entrée.
//!
//! ## Ce qu'on ajoute
//!
//! Overload conçoit l'API que ses scripts consomment ; niers la retro-conçoit. La session tient
//! donc le compte de ce que les scripts **réclament sans l'obtenir** ([`LuaSession::api_report`]) :
//! c'est la liste de travail du portage moteur, produite par l'exécution elle-même.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::{Lua, MultiValue, Table, Value};

use crate::host::{HostRegistry, LogEntry, LogSink};
use crate::menu_host::{DriveReport, MenuState};
use crate::runtime::{
    GlobalEntry, RuntimeContext, install_host_stubs, install_print_capture, list_globals,
    value_to_string,
};
use crate::{
    ChunkMode, LuaError, index_script_paths, is_lua52_bytecode, resolve_script_path,
    validate_bytecode,
};

/// Points d'entrée standard d'un comportement, dans l'ordre du cycle de vie d'Overload.
///
/// Les noms sont ceux d'Overload (`OnAwake`, `OnStart`, …) : c'est une convention d'outillage pour
/// nos propres scripts, **pas** une prétention sur l'API de Level-5, dont les points d'entrée
/// réels sont ceux du reverse (cf. [`crate::menu_host`]).
pub const LIFECYCLE_CALLBACKS: [&str; 6] = [
    "OnAwake",
    "OnStart",
    "OnEnable",
    "OnUpdate",
    "OnDisable",
    "OnDestroy",
];

type IncludeResolver = Rc<dyn Fn(&str) -> Option<Vec<u8>>>;
type BuiltVm = (Lua, Option<Rc<RefCell<MenuState>>>);

/// Un script attaché, avec la table qu'il a renvoyée.
///
/// Contrat repris d'Overload : le chunk **doit renvoyer une table**, qui porte ses callbacks. Un
/// script qui ne renvoie rien n'est pas un comportement — c'est un script d'initialisation, et le
/// dire clairement évite de chercher pourquoi `OnUpdate` n'est jamais appelé.
pub struct Behaviour {
    /// Nom logique (chemin VFS ou étiquette d'éditeur).
    pub name: String,
    /// Table renvoyée par le script.
    table: Table,
}

impl Behaviour {
    /// Callbacks du cycle de vie effectivement définis par ce script.
    #[must_use]
    pub fn defined_callbacks(&self) -> Vec<&'static str> {
        LIFECYCLE_CALLBACKS
            .iter()
            .copied()
            .filter(|name| matches!(self.table.get::<Value>(*name), Ok(Value::Function(_))))
            .collect()
    }

    /// Appelle un callback s'il existe. **Absent = succès silencieux**, comme chez Overload.
    ///
    /// # Errors
    /// [`mlua::Error`] seulement si le callback existe ET échoue — une erreur réelle du script,
    /// qu'il ne faut surtout pas confondre avec « le script ne définit pas ce point d'entrée ».
    pub fn call(&self, callback: &str, args: MultiValue) -> mlua::Result<MultiValue> {
        match self.table.get::<Value>(callback) {
            Ok(Value::Function(f)) => f.call(args),
            _ => Ok(MultiValue::new()),
        }
    }
}

/// Ce qu'un script demande au moteur, et ce qu'il obtient.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApiReport {
    /// Globals réclamés mais non définis — la liste de travail du portage.
    pub missing: Vec<String>,
    /// Globals fournis par les binders installés.
    pub provided: Vec<String>,
}

impl ApiReport {
    /// Part de la surface réclamée qui est couverte, en pourcentage (100 si rien n'est réclamé).
    #[must_use]
    pub fn coverage_percent(&self) -> u32 {
        let total = self.missing.len() + self.provided.len();
        if total == 0 {
            return 100;
        }
        ((self.provided.len() * 100) / total) as u32
    }
}

/// Une VM Lua persistante, ses binders et ses comportements attachés.
pub struct LuaSession {
    lua: Lua,
    /// État du host de menu installé dans cette VM, quand `with_menu_host` est actif.
    ///
    /// Le conserver est essentiel pour une session live : les callbacks Lua mutent cet état
    /// entre deux appels, exactement comme le manager de menus natif conserve ses objets.
    menu_state: Option<Rc<RefCell<MenuState>>>,
    registry: HostRegistry,
    logs: LogSink,
    stdout: Rc<RefCell<Vec<String>>>,
    behaviours: Vec<Behaviour>,
    /// Sources attachées, conservées pour le rechargement — sans elles, `reload` ne pourrait pas
    /// ré-attacher ce qui était en place.
    attached_sources: Vec<(String, Vec<u8>)>,
    with_menu_host: bool,
    /// Résolveur persistant des `INCLUDE`, typiquement une fermeture adossée au VFS brut.
    include_resolver: Option<IncludeResolver>,
    /// Includes demandés mais absents depuis le dernier prélèvement.
    missing_includes: Rc<RefCell<Vec<String>>>,
    /// Includes effectivement résolus depuis le dernier prélèvement, dans l'ordre de chargement.
    loaded_includes: Rc<RefCell<Vec<String>>>,
    /// Contexte natif réappliqué après chaque reconstruction de VM.
    context: RuntimeContext,
}

impl LuaSession {
    /// Crée une session : VM neuve, binders installés, stubs de globals actifs.
    ///
    /// `logs` DOIT être le tampon confié aux binders de `registry` (typiquement à
    /// [`crate::host::DebugBinder`]) : sans ça, [`Self::take_logs`] lirait un tampon que personne
    /// n'alimente et la session paraîtrait muette. [`Self::standard`] évite ce piège en
    /// construisant les deux ensemble.
    ///
    /// # Errors
    /// [`LuaError`] si l'installation de l'hôte échoue.
    pub fn new(
        registry: HostRegistry,
        logs: LogSink,
        with_menu_host: bool,
    ) -> Result<Self, LuaError> {
        Self::new_with_resolver(registry, logs, with_menu_host, None)
    }

    /// Crée une session dont `INCLUDE(name)` lit les chunks depuis le résolveur fourni.
    ///
    /// Le résolveur est conservé par la session : `reload()` reconstruit la VM et réinstalle
    /// exactement le même accès au VFS, au lieu de retomber silencieusement sur une console sans
    /// modules. Les chunks inclus sont exécutés dans la VM de la session, comme dans `nie.exe`.
    ///
    /// # Errors
    /// [`LuaError`] si l'installation de l'hôte ou de `INCLUDE` échoue.
    pub fn with_include<F>(
        registry: HostRegistry,
        logs: LogSink,
        with_menu_host: bool,
        resolver: F,
    ) -> Result<Self, LuaError>
    where
        F: Fn(&str) -> Option<Vec<u8>> + 'static,
    {
        Self::new_with_resolver(registry, logs, with_menu_host, Some(Rc::new(resolver)))
    }

    /// Crée une session dont `INCLUDE` lit directement un index de chemins VFS.
    ///
    /// `paths` doit contenir les chemins physiques des chunks (`.lua.bin`) et `reader` les lit
    /// depuis le VFS brut. La résolution accepte les noms physiques, les basenames et les noms
    /// logiques `LUA_*`, en sélectionnant la version numérique la plus récente.
    ///
    /// # Errors
    /// [`LuaError`] si l'installation de l'hôte ou de `INCLUDE` échoue.
    pub fn with_script_paths<I, F>(
        registry: HostRegistry,
        logs: LogSink,
        with_menu_host: bool,
        paths: I,
        reader: F,
    ) -> Result<Self, LuaError>
    where
        I: IntoIterator<Item = String>,
        F: Fn(&str) -> Option<Vec<u8>> + 'static,
    {
        let paths = paths.into_iter().collect::<Vec<_>>();
        let (by_name, by_logical) = index_script_paths(paths.iter().map(String::as_str));
        let by_name = Rc::new(by_name);
        let by_logical = Rc::new(by_logical);
        let reader = Rc::new(reader);
        Self::with_include(registry, logs, with_menu_host, move |name| {
            let path = resolve_script_path(name, &by_name, &by_logical)?;
            reader(path)
        })
    }

    fn new_with_resolver(
        registry: HostRegistry,
        logs: LogSink,
        with_menu_host: bool,
        include_resolver: Option<IncludeResolver>,
    ) -> Result<Self, LuaError> {
        let stdout = Rc::new(RefCell::new(Vec::new()));
        let missing_includes = Rc::new(RefCell::new(Vec::new()));
        let loaded_includes = Rc::new(RefCell::new(Vec::new()));
        let context = RuntimeContext::default();
        let (lua, menu_state) = Self::build_vm(
            &registry,
            &stdout,
            with_menu_host,
            include_resolver.as_ref(),
            &missing_includes,
            &loaded_includes,
            &context,
        )?;
        Ok(Self {
            lua,
            menu_state,
            registry,
            logs,
            stdout,
            behaviours: Vec::new(),
            attached_sources: Vec::new(),
            with_menu_host,
            include_resolver,
            missing_includes,
            loaded_includes,
            context,
        })
    }

    /// Session prête à l'emploi : registre standard (`Debug` + `Math`) et tampon de journal
    /// correctement relié.
    ///
    /// # Errors
    /// [`LuaError`] si l'installation de l'hôte échoue.
    pub fn standard(with_menu_host: bool) -> Result<Self, LuaError> {
        let logs: LogSink = Rc::new(RefCell::new(Vec::new()));
        let registry = HostRegistry::standard(Rc::clone(&logs));
        Self::new(registry, logs, with_menu_host)
    }

    /// Reconstruit une VM complète. Le tampon de journal n'est pas repris ici : il est déjà
    /// capturé par les closures des binders de `registry`, qui survivent au rechargement.
    fn build_vm(
        registry: &HostRegistry,
        stdout: &Rc<RefCell<Vec<String>>>,
        with_menu_host: bool,
        include_resolver: Option<&IncludeResolver>,
        missing_includes: &Rc<RefCell<Vec<String>>>,
        loaded_includes: &Rc<RefCell<Vec<String>>>,
        context: &RuntimeContext,
    ) -> Result<BuiltVm, LuaError> {
        let lua = crate::new_vm();
        install_print_capture(&lua, Rc::clone(stdout))?;
        registry.bind_all(&lua)?;
        let menu_state = if with_menu_host {
            Some(crate::install_menu_host(&lua)?)
        } else {
            None
        };
        if let Some(resolver) = include_resolver {
            let resolver = Rc::clone(resolver);
            let missing = Rc::clone(missing_includes);
            let loaded = Rc::clone(loaded_includes);
            crate::install_include(&lua, move |name| match resolver(name) {
                Some(bytes) => {
                    loaded.borrow_mut().push(name.to_string());
                    Some(bytes)
                }
                None => {
                    missing.borrow_mut().push(name.to_string());
                    None
                }
            })?;
        }
        // Les stubs viennent EN DERNIER : la métatable de `_G` ne doit intercepter que ce qu'aucun
        // binder n'a fourni, sinon tout serait déclaré « manquant ».
        install_host_stubs(&lua)?;
        context.apply(&lua)?;
        Ok((lua, menu_state))
    }

    /// Accès à la VM, pour les usages avancés (ex. installer un binder supplémentaire).
    #[must_use]
    pub fn lua(&self) -> &Lua {
        &self.lua
    }

    /// État du menu partagé par la VM live, s’il a été demandé à la construction.
    #[must_use]
    pub fn menu_state(&self) -> Option<Rc<RefCell<MenuState>>> {
        self.menu_state.as_ref().map(Rc::clone)
    }

    /// Exécute un menu dans la VM persistante, avec ses includes VFS et son `MenuState`.
    ///
    /// Cette méthode conserve les globals, coroutines et mutations déjà produits par les
    /// évaluations précédentes. Elle est donc adaptée au pilotage « live » ; `reload()` reste
    /// l’opération explicite qui recrée le contexte.
    ///
    /// # Errors
    /// [`LuaError`] si le host menu n’a pas été activé ou si le bytecode est invalide.
    pub fn drive_menu_for_frames(
        &self,
        script_bytes: &[u8],
        name: &str,
        layer_ids: &[u32],
        item_counts: &std::collections::BTreeMap<u32, i32>,
        frames: u32,
    ) -> Result<DriveReport, LuaError> {
        if self.menu_state.is_none() {
            return Err(LuaError::Vm(mlua::Error::RuntimeError(
                "LuaSession: with_menu_host=false, impossible de piloter un menu".to_string(),
            )));
        }
        crate::menu_host::drive_menu_for_frames(
            &self.lua,
            script_bytes,
            name,
            layer_ids,
            item_counts,
            frames,
        )
    }

    /// Lignes de `print` accumulées depuis le dernier [`Self::take_output`].
    #[must_use]
    pub fn take_output(&self) -> Vec<String> {
        std::mem::take(&mut self.stdout.borrow_mut())
    }

    /// Includes VFS demandés mais absents depuis le dernier prélèvement, dédoublonnés et triés.
    #[must_use]
    pub fn take_missing_includes(&self) -> Vec<String> {
        let mut journal = self.missing_includes.borrow_mut();
        let mut missing = std::mem::take(&mut *journal);
        missing.sort_unstable();
        missing.dedup();
        missing
    }

    /// Includes VFS effectivement chargés depuis le dernier prélèvement, dans l'ordre réel.
    #[must_use]
    pub fn take_loaded_includes(&self) -> Vec<String> {
        std::mem::take(&mut self.loaded_includes.borrow_mut())
    }

    /// Messages `Debug.*` accumulés depuis le dernier appel.
    #[must_use]
    pub fn take_logs(&self) -> Vec<LogEntry> {
        std::mem::take(&mut self.logs.borrow_mut())
    }

    /// Exécute un chunk (source ou bytecode) dans la session, sans l'attacher.
    ///
    /// # Errors
    /// [`LuaError`] si le chunk échoue — ici l'erreur EST propagée : contrairement à une analyse,
    /// une exécution demandée explicitement doit dire qu'elle a raté.
    pub fn exec(&self, name: &str, data: &[u8]) -> Result<Vec<String>, LuaError> {
        validate_bytecode(data)?;
        let mode = if is_lua52_bytecode(data) {
            ChunkMode::Binary
        } else {
            ChunkMode::Text
        };
        let values: MultiValue = self
            .lua
            .load(data)
            .set_name(name.to_string())
            .set_mode(mode)
            .call(())?;
        Ok(values.iter().map(value_to_string).collect())
    }

    /// Attache un script comme comportement.
    ///
    /// Contrat d'Overload : le chunk doit **renvoyer une table**. Un chunk qui renvoie autre chose
    /// (ou rien) est refusé explicitement plutôt qu'attaché à vide — sinon ses callbacks ne
    /// seraient jamais appelés et rien ne dirait pourquoi.
    ///
    /// # Errors
    /// [`LuaError`] si le chunk échoue ou ne renvoie pas de table.
    pub fn attach(&mut self, name: &str, data: &[u8]) -> Result<&Behaviour, LuaError> {
        validate_bytecode(data)?;
        let mode = if is_lua52_bytecode(data) {
            ChunkMode::Binary
        } else {
            ChunkMode::Text
        };
        let value: Value = self
            .lua
            .load(data)
            .set_name(name.to_string())
            .set_mode(mode)
            .call(())?;

        let Value::Table(table) = value else {
            return Err(LuaError::Vm(mlua::Error::RuntimeError(format!(
                "« {name} » n'est pas un comportement : un script attaché doit renvoyer une table \
                 portant ses callbacks (OnStart, OnUpdate, …)"
            ))));
        };

        self.behaviours.push(Behaviour {
            name: name.to_string(),
            table,
        });
        self.attached_sources
            .push((name.to_string(), data.to_vec()));
        Ok(self.behaviours.last().expect("vient d'être poussé"))
    }

    /// Comportements attachés.
    #[must_use]
    pub fn behaviours(&self) -> &[Behaviour] {
        &self.behaviours
    }

    /// Diffuse un callback à tous les comportements attachés.
    ///
    /// Renvoie le nombre de comportements qui définissaient réellement ce callback — utile pour
    /// distinguer « diffusé à personne » de « diffusé et sans effet ».
    ///
    /// # Errors
    /// [`LuaError`] à la première erreur *réelle* d'un callback (un callback absent n'en est pas
    /// une).
    pub fn broadcast(&self, callback: &str) -> Result<usize, LuaError> {
        let mut called = 0;
        for behaviour in &self.behaviours {
            if behaviour.defined_callbacks().contains(&callback) {
                behaviour.call(callback, MultiValue::new())?;
                called += 1;
            }
        }
        Ok(called)
    }

    /// Recrée la VM et ré-attache les comportements — le `RefreshAll` d'Overload.
    ///
    /// Une VM neuve est la seule façon correcte de recharger : Lua ne sait pas retirer une
    /// définition. Recharger « par-dessus » laisserait les globals de l'ancienne version en place,
    /// et une fonction supprimée du script continuerait d'exister.
    ///
    /// # Errors
    /// [`LuaError`] si la reconstruction ou un ré-attachement échoue.
    pub fn reload(&mut self) -> Result<(), LuaError> {
        let (lua, menu_state) = Self::build_vm(
            &self.registry,
            &self.stdout,
            self.with_menu_host,
            self.include_resolver.as_ref(),
            &self.missing_includes,
            &self.loaded_includes,
            &self.context,
        )?;
        self.lua = lua;
        self.menu_state = menu_state;
        self.behaviours.clear();

        let sources = std::mem::take(&mut self.attached_sources);
        for (name, data) in sources {
            self.attach(&name, &data)?;
        }
        Ok(())
    }

    /// Évalue une expression dans l'état COURANT de la session — la console.
    ///
    /// # Errors
    /// Jamais : l'échec est rendu en texte, parce que voir le message est le résultat attendu.
    pub fn eval(&self, expression: &str) -> Result<String, LuaError> {
        crate::runtime::eval_expression(&self.lua, expression)
    }

    /// Globals de la session.
    #[must_use]
    pub fn globals(&self, include_stdlib: bool) -> Vec<GlobalEntry> {
        list_globals(&self.lua, include_stdlib)
    }

    /// Pose une valeur globale — l'éditeur de valeurs, appliqué à une session vivante.
    ///
    /// L'expression est évaluée par la VM : `999`, `'texte'` ou `{a=1}` sont tous acceptés, sans
    /// que l'appelant ait à typer quoi que ce soit.
    ///
    /// # Errors
    /// [`LuaError`] si l'expression est invalide.
    pub fn set_global(&self, name: &str, expression: &str) -> Result<(), LuaError> {
        self.lua
            .load(format!("{name} = {expression}"))
            .set_name("=set_global")
            .exec()?;
        Ok(())
    }

    /// Injecte un contexte natif typé dans la VM vivante et le conserve pour `reload()`.
    ///
    /// Les valeurs sont posées après les stubs : elles remplacent donc réellement les proxies
    /// d'accès manquant. Le contexte est remplacé en bloc pour qu'un ancien état de save/scene ne
    /// survive pas silencieusement à un changement d'écran.
    pub fn set_context(&mut self, context: RuntimeContext) -> Result<(), LuaError> {
        self.context.clear_replaced_by(&self.lua, &context)?;
        context.apply(&self.lua)?;
        self.context = context;
        Ok(())
    }

    /// Retourne une copie du contexte actuellement associé à la session.
    #[must_use]
    pub fn context(&self) -> RuntimeContext {
        self.context.clone()
    }

    /// Confronte ce que les scripts ont réclamé à ce que les binders fournissent.
    #[must_use]
    pub fn api_report(&self) -> ApiReport {
        let mut missing: Vec<String> = self
            .lua
            .globals()
            .get::<Table>("_HOST_MISSING")
            .map(|t| {
                t.pairs::<String, Value>()
                    .filter_map(Result::ok)
                    .map(|(k, _)| k)
                    .collect()
            })
            .unwrap_or_default();
        missing.sort_unstable();
        missing.dedup();

        ApiReport {
            missing,
            provided: self.registry.installed_names(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> LuaSession {
        LuaSession::standard(false).expect("session")
    }

    /// La propriété qui manquait : l'état survit d'une évaluation à l'autre.
    #[test]
    fn la_session_conserve_son_etat() {
        let s = session();
        s.eval("compteur = 1").expect("eval");
        s.eval("compteur = compteur + 41").expect("eval");
        assert_eq!(s.eval("compteur").unwrap(), "42");
    }

    #[test]
    fn le_contexte_natif_type_survit_au_reload_et_ne_devient_pas_un_stub() {
        let mut s = session();
        let mut context = RuntimeContext::default();
        context.set_number("pieceIdx", 3.0);
        context.set_boolean("isGrayout", true);
        context.set_string("MENU_LINIT_NONE", "native-sentinel");
        let mut initial = context.clone();
        initial.set_number("oldSceneSlot", 9.0);
        s.set_context(initial).expect("contexte initial");
        s.exec(
            "context",
            br#"assert(pieceIdx == 3); assert(isGrayout == true); assert(MENU_LINIT_NONE == "native-sentinel")"#,
        )
        .expect("globals de contexte");
        s.set_context(context.clone()).expect("remplacement contexte");
        s.exec(
            "context-replacement",
            br#"assert(rawget(_G, "oldSceneSlot") == nil)"#,
        )
        .expect("ancien slot supprimé");
        assert!(s.api_report().missing.is_empty());
        assert_eq!(s.context(), context);

        s.reload().expect("reload");
        s.exec(
            "context-after-reload",
            br#"assert(pieceIdx == 3); assert(isGrayout == true); assert(MENU_LINIT_NONE == "native-sentinel")"#,
        )
        .expect("contexte après reload");
    }

    #[test]
    fn la_session_persistante_resout_include_dans_la_meme_vm() {
        let logs: LogSink = Rc::new(RefCell::new(Vec::new()));
        let registry = HostRegistry::standard(Rc::clone(&logs));
        let module = br#"module_value = (rawget(_G, "module_value") or 0) + 1; return { value = module_value }"#
            .to_vec();
        let module_for_resolver = module.clone();
        let mut s = LuaSession::with_include(registry, logs, false, move |name| {
            (name == "LUA_TEST_MODULE").then(|| module_for_resolver.clone())
        })
        .expect("session avec include");

        s.exec(
            "main",
            br#"INCLUDE("LUA_TEST_MODULE"); main_value = module_value"#,
        )
        .expect("premier include");
        assert_eq!(s.eval("main_value").unwrap(), "1");
        assert_eq!(s.take_loaded_includes(), vec!["LUA_TEST_MODULE"]);
        s.reload().expect("rechargement");
        s.exec(
            "main",
            br#"INCLUDE("LUA_TEST_MODULE"); main_value = module_value"#,
        )
        .expect("include après rechargement");
        assert_eq!(s.eval("main_value").unwrap(), "1");
        assert_eq!(s.take_loaded_includes(), vec!["LUA_TEST_MODULE"]);
        s.exec("missing", br#"INCLUDE("LUA_MISSING"); missing_value = 1"#)
            .expect("include absent toléré");
        assert_eq!(s.take_missing_includes(), vec!["LUA_MISSING"]);
        assert!(s.take_missing_includes().is_empty());
    }

    #[test]
    fn la_session_indexe_un_include_vfs_versionne() {
        let logs: LogSink = Rc::new(RefCell::new(Vec::new()));
        let registry = HostRegistry::standard(Rc::clone(&logs));
        let paths = vec![
            "data/common/script/lua/menu/module_9.lua.bin".to_string(),
            "data/common/script/lua/menu/module_10.lua.bin".to_string(),
        ];
        let session = LuaSession::with_script_paths(registry, logs, false, paths, |path| {
            (path.ends_with("module_10.lua.bin")).then(|| b"vfs_value = 42".to_vec())
        })
        .expect("session VFS");
        session
            .exec("main", br#"INCLUDE("LUA_MODULE"); assert(vfs_value == 42)"#)
            .expect("include VFS versionné");
        assert_eq!(session.take_loaded_includes(), vec!["LUA_MODULE"]);
    }

    #[test]
    fn la_session_menu_conserve_le_menu_state_de_la_vm_live() {
        let mut s = LuaSession::standard(true).expect("session menu");
        let bytes = s
            .lua()
            .load(
                &br#"
                function OnInit()
                    funcLuaMenuCommand(0x2A64B198, 0x1234, 0, false)
                end
            "#[..],
            )
            .into_function()
            .expect("compile menu")
            .dump(false);

        let report = s
            .drive_menu_for_frames(
                &bytes,
                "live-menu",
                &[],
                &std::collections::BTreeMap::new(),
                0,
            )
            .expect("drive menu");
        assert_eq!(report.on_init, Some(true));
        let state = s.menu_state().expect("MenuState conservé");
        assert!(!state.borrow().layers[&0].objects[&0x1234].visible);

        s.reload().expect("reload");
        assert!(
            s.menu_state()
                .expect("MenuState après reload")
                .borrow()
                .layers
                .is_empty()
        );
    }

    #[test]
    fn attache_un_comportement_et_diffuse_les_callbacks() {
        let mut s = session();
        s.attach(
            "essai",
            br#"
            local M = { appels = 0 }
            function M.OnStart() M.appels = M.appels + 1 end
            function M.OnUpdate() M.appels = M.appels + 10 end
            comportement = M
            return M
            "#,
        )
        .expect("attachement");

        let b = &s.behaviours()[0];
        let defined = b.defined_callbacks();
        assert!(defined.contains(&"OnStart"), "callbacks : {defined:?}");
        assert!(defined.contains(&"OnUpdate"), "callbacks : {defined:?}");
        assert!(
            !defined.contains(&"OnDestroy"),
            "OnDestroy n'est pas défini : {defined:?}"
        );

        assert_eq!(s.broadcast("OnStart").unwrap(), 1);
        assert_eq!(s.broadcast("OnUpdate").unwrap(), 1);
        // Un callback qu'aucun script ne définit : diffusé à personne, sans erreur.
        assert_eq!(s.broadcast("OnDestroy").unwrap(), 0);

        assert_eq!(s.eval("comportement.appels").unwrap(), "11");
    }

    #[test]
    fn refuse_un_script_qui_ne_renvoie_pas_de_table() {
        let mut s = session();
        let err = match s.attach("pas_un_comportement", b"local x = 1") {
            Err(e) => e,
            Ok(_) => panic!("un script sans table ne doit pas être attaché"),
        };
        assert!(
            err.to_string().contains("doit renvoyer une table"),
            "message peu clair : {err}"
        );
    }

    /// Le rechargement doit VRAIMENT repartir de zéro : une définition retirée du script ne doit
    /// pas survivre. C'est tout l'argument d'Overload pour recréer le contexte.
    #[test]
    fn le_rechargement_efface_letat_precedent() {
        let mut s = session();
        s.eval("resteApres = 'oui'").expect("eval");
        assert_eq!(s.eval("resteApres").unwrap(), "oui");

        s.reload().expect("rechargement");
        // La VM est neuve : le global posé à la main a disparu. `nil` — et pas la valeur d'avant.
        assert_eq!(s.eval("type(rawget(_G, 'resteApres'))").unwrap(), "nil");
    }

    #[test]
    fn le_rechargement_reattache_les_comportements() {
        let mut s = session();
        s.attach("c", b"local M = {} function M.OnStart() end return M")
            .expect("attachement");
        assert_eq!(s.behaviours().len(), 1);

        s.reload().expect("rechargement");
        assert_eq!(
            s.behaviours().len(),
            1,
            "le comportement doit être ré-attaché"
        );
        assert_eq!(s.broadcast("OnStart").unwrap(), 1);
    }

    #[test]
    fn rapport_dapi_confronte_reclame_et_fourni() {
        let s = session();
        // `Debug` et `Math` sont fournis par les binders ; les deux autres non.
        s.eval("Debug.Log('ok') MOTEUR_INCONNU() AUTRE_APPEL()")
            .expect("eval");

        let report = s.api_report();
        assert!(report.provided.contains(&"Debug".to_string()));
        assert!(
            report.missing.contains(&"MOTEUR_INCONNU".to_string()),
            "{report:?}"
        );
        assert!(
            report.missing.contains(&"AUTRE_APPEL".to_string()),
            "{report:?}"
        );
        assert!(
            !report.missing.contains(&"Debug".to_string()),
            "un global fourni par un binder ne doit jamais être compté manquant : {report:?}"
        );
    }

    #[test]
    fn edite_une_valeur_dans_la_session_vivante() {
        let s = session();
        s.eval("pv = 100").expect("eval");
        s.set_global("pv", "250").expect("écriture");
        assert_eq!(s.eval("pv").unwrap(), "250");

        s.set_global("table_test", "{a = 1, b = 2}")
            .expect("écriture");
        assert_eq!(s.eval("table_test.b").unwrap(), "2");
    }

    #[test]
    fn capture_la_sortie_et_les_journaux() {
        let s = session();
        s.exec("t", b"print('sortie') Debug.LogWarning('attention')")
            .expect("exec");
        assert_eq!(s.take_output(), vec!["sortie".to_string()]);
        let logs = s.take_logs();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].message, "attention");
        // Le tampon est vidé par la prise : deux appels ne doivent pas rejouer les mêmes lignes.
        assert!(s.take_logs().is_empty());
    }
}
