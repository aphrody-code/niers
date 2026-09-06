//! **Binders d'API hôte composables** — surface moteur exposée aux scripts, par morceaux.
//!
//! ## Pourquoi ce module existe
//!
//! `install_menu_host` ([`crate::menu_host`], 1 900 lignes) fait tout d'un bloc : c'est l'hôte de
//! menu, complet et vérifié, mais indivisible. Or la surface d'API que réclament les ~1 100
//! scripts du jeu est bien plus large que les menus, et se découvre **par incréments** (cf.
//! [`crate::discover_host_calls`] : chaque script exécuté révèle les globals qu'il attend). Il faut
//! donc pouvoir ajouter un pan d'API sans toucher au reste, et surtout **savoir ce qui est
//! implémenté et ce qui ne l'est pas**.
//!
//! ## Architecture, reprise d'Overload
//!
//! Le moteur Overload (`OvCore/Scripting`) sépare son exposition Lua en *binders* indépendants —
//! `LuaGlobalsBinder`, `LuaMathsBinder`, `LuaActorBinder`, `LuaComponentBinder` — agrégés par un
//! unique `LuaBinder::CallBinders(state)`. Chaque binder pose une table globale cohérente
//! (`Debug`, `Math`, `Inputs`, `Resources`…) au lieu d'éparpiller des fonctions libres.
//!
//! On reprend la structure : un [`HostBinder`] par domaine, un [`HostRegistry`] qui les compose,
//! et une trace de ce que chacun a posé ([`HostRegistry::installed_names`]) — cette dernière n'est
//! pas dans Overload, mais elle est indispensable ici : notre travail consiste justement à
//! comparer *ce que le jeu demande* à *ce que nous fournissons*.
//!
//! ## Différence assumée avec Overload
//!
//! Overload **conçoit** l'API que ses scripts utiliseront ; niers la **retro-conçoit**. Les noms
//! posés ici (`Debug.Log`, `Math.Lerp`…) sont donc des services utilitaires pour nos propres
//! scripts d'outillage et de test, pas une prétention à reproduire l'API de Level-5 — celle-ci
//! vit dans [`crate::menu_host`], adossée au reverse de `nie.exe`.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::{Lua, Value, Variadic};

/// Niveau d'un message de journal, comme la table `Debug` d'Overload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    /// Message courant.
    Info,
    /// Anomalie non bloquante.
    Warning,
    /// Erreur.
    Error,
}

impl LogLevel {
    /// Étiquette courte, telle qu'affichée dans une console.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warn",
            Self::Error => "error",
        }
    }
}

/// Une ligne journalisée par un script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    /// Sévérité.
    pub level: LogLevel,
    /// Texte, arguments joints par une tabulation (comme `print`).
    pub message: String,
}

/// Tampon de journal partagé entre l'hôte et l'appelant.
pub type LogSink = Rc<RefCell<Vec<LogEntry>>>;

/// Un pan d'API exposé aux scripts.
///
/// Équivalent d'un `Lua*Binder` d'Overload : il pose une (ou plusieurs) table(s) globale(s) et
/// déclare ce qu'il installe, pour que l'outillage sache distinguer « implémenté » de « manquant ».
pub trait HostBinder {
    /// Nom du domaine (`"Debug"`, `"Math"`…) — sert aux diagnostics.
    fn name(&self) -> &'static str;

    /// Globals que ce binder installe. Déclaratif : c'est cette liste qui est confrontée aux
    /// appels manquants relevés à l'exécution.
    fn provides(&self) -> Vec<String>;

    /// Installe effectivement les fonctions dans la VM.
    ///
    /// # Errors
    /// [`mlua::Error`] si l'enregistrement échoue.
    fn bind(&self, lua: &Lua) -> mlua::Result<()>;
}

/// Table `Debug` — journalisation par niveau.
///
/// Overload expose `Debug.Log/LogInfo/LogWarning/LogError`. Ici les messages ne partent pas sur la
/// sortie standard mais dans un tampon : un script lancé depuis une interface graphique doit
/// pouvoir montrer ce qu'il a dit, ce qu'un `println!` ne permet pas.
pub struct DebugBinder {
    sink: LogSink,
}

impl DebugBinder {
    /// Crée le binder autour d'un tampon partagé.
    #[must_use]
    pub fn new(sink: LogSink) -> Self {
        Self { sink }
    }

    fn join(args: &Variadic<Value>) -> String {
        args.iter()
            .map(crate::runtime::value_to_string)
            .collect::<Vec<_>>()
            .join("\t")
    }
}

impl HostBinder for DebugBinder {
    fn name(&self) -> &'static str {
        "Debug"
    }

    fn provides(&self) -> Vec<String> {
        vec!["Debug".to_string()]
    }

    fn bind(&self, lua: &Lua) -> mlua::Result<()> {
        let table = lua.create_table()?;

        for (key, level) in [
            ("Log", LogLevel::Info),
            ("LogInfo", LogLevel::Info),
            ("LogWarning", LogLevel::Warning),
            ("LogError", LogLevel::Error),
        ] {
            let sink = Rc::clone(&self.sink);
            let f = lua.create_function(move |_, args: Variadic<Value>| {
                sink.borrow_mut().push(LogEntry {
                    level,
                    message: Self::join(&args),
                });
                Ok(())
            })?;
            table.set(key, f)?;
        }

        lua.globals().set("Debug", table)?;
        Ok(())
    }
}

/// Table `Math` — utilitaires numériques.
///
/// Reprend la table `Math` d'Overload (`RandomInt`, `RandomFloat`, `Lerp`, `CheckPercentage`).
/// Le générateur est **déterministe et local** : deux exécutions du même script donnent la même
/// suite. Un aléa dépendant de l'horloge rendrait impossible de comparer deux passages, alors que
/// comparer est tout l'intérêt d'un outil d'analyse.
pub struct MathBinder {
    /// Graine initiale du générateur.
    pub seed: u64,
}

impl Default for MathBinder {
    fn default() -> Self {
        Self {
            seed: 0x2545_F491_4F6C_DD1D,
        }
    }
}

impl HostBinder for MathBinder {
    fn name(&self) -> &'static str {
        "Math"
    }

    fn provides(&self) -> Vec<String> {
        vec!["Math".to_string()]
    }

    fn bind(&self, lua: &Lua) -> mlua::Result<()> {
        let table = lua.create_table()?;

        // xorshift64* : suffisant pour de l'outillage, tient en quelques lignes, et surtout
        // reproductible d'une machine à l'autre (contrairement à `rand` dont l'algorithme peut
        // changer entre versions).
        let state = Rc::new(RefCell::new(self.seed.max(1)));

        let next = {
            let state = Rc::clone(&state);
            move || {
                let mut s = state.borrow_mut();
                *s ^= *s >> 12;
                *s ^= *s << 25;
                *s ^= *s >> 27;
                s.wrapping_mul(0x2545_F491_4F6C_DD1D)
            }
        };

        let random_float = {
            let next = next.clone();
            lua.create_function(move |_, ()| {
                // 53 bits de mantisse : la conversion est exacte, pas d'artefact d'arrondi.
                Ok((next() >> 11) as f64 / (1u64 << 53) as f64)
            })?
        };

        let random_int = {
            let next = next.clone();
            lua.create_function(move |_, (min, max): (i64, i64)| {
                if max <= min {
                    return Ok(min);
                }
                let span = (max - min + 1) as u64;
                Ok(min + (next() % span) as i64)
            })?
        };

        let check_percentage = {
            let next = next.clone();
            lua.create_function(move |_, percent: f64| {
                let roll = (next() >> 11) as f64 / (1u64 << 53) as f64 * 100.0;
                Ok(roll < percent)
            })?
        };

        let lerp = lua.create_function(|_, (a, b, t): (f64, f64, f64)| Ok(a + (b - a) * t))?;

        table.set("RandomFloat", random_float)?;
        table.set("RandomInt", random_int)?;
        table.set("CheckPercentage", check_percentage)?;
        table.set("Lerp", lerp)?;

        lua.globals().set("Math", table)?;
        Ok(())
    }
}

/// Table `Vfs` — lecture des assets du jeu depuis un script.
///
/// Sans équivalent direct chez Overload (dont la table `Resources` charge des modèles/textures
/// déjà gérés par son gestionnaire d'assets). Ici, l'intérêt est différent : permettre à un script
/// d'outillage de parcourir les ~255 000 fichiers du jeu — écrire une passe d'analyse en Lua au
/// lieu d'un binaire Rust à recompiler.
pub struct VfsBinder<L, R>
where
    L: Fn(&str) -> Vec<String> + 'static,
    R: Fn(&str) -> Option<Vec<u8>> + 'static,
{
    /// Liste les chemins dont le préfixe correspond.
    pub list: Rc<L>,
    /// Lit un fichier par chemin.
    pub read: Rc<R>,
}

impl<L, R> HostBinder for VfsBinder<L, R>
where
    L: Fn(&str) -> Vec<String> + 'static,
    R: Fn(&str) -> Option<Vec<u8>> + 'static,
{
    fn name(&self) -> &'static str {
        "Vfs"
    }

    fn provides(&self) -> Vec<String> {
        vec!["Vfs".to_string()]
    }

    fn bind(&self, lua: &Lua) -> mlua::Result<()> {
        let table = lua.create_table()?;

        let list = Rc::clone(&self.list);
        table.set(
            "List",
            lua.create_function(move |lua, prefix: String| {
                let paths = list(&prefix);
                let out = lua.create_table()?;
                for (i, p) in paths.iter().enumerate() {
                    out.set(i + 1, p.as_str())?;
                }
                Ok(out)
            })?,
        )?;

        let read = Rc::clone(&self.read);
        table.set(
            "Read",
            lua.create_function(move |lua, path: String| match read(&path) {
                // `create_string` sur des octets bruts : les assets ne sont pas de l'UTF-8, et une
                // conversion lossy corromprait silencieusement ce que le script inspecte.
                Some(bytes) => Ok(Value::String(lua.create_string(&bytes)?)),
                None => Ok(Value::Nil),
            })?,
        )?;

        let read_size = Rc::clone(&self.read);
        table.set(
            "Size",
            lua.create_function(move |_, path: String| {
                Ok(read_size(&path).map_or(-1i64, |b| b.len() as i64))
            })?,
        )?;

        lua.globals().set("Vfs", table)?;
        Ok(())
    }
}

/// Plafond d'une lecture mémoire live, en octets — évite qu'un `Live.Read(addr, 1e9)` alloue un
/// tampon géant sur une faute de frappe. Même valeur que la borne côté `re_trace`.
const LIVE_READ_MAX: usize = 1024 * 1024;

/// Interprète une adresse passée depuis Lua : nombre entier, nombre flottant, ou chaîne (`"0x…"`
/// ou décimale). Les adresses utilisateur x64 (< 2⁴⁷) sont exactes en `f64`, donc un nombre Lua
/// (double) suffit à les représenter sans perte.
fn parse_live_addr(v: &Value) -> Option<u64> {
    match v {
        Value::Integer(i) => Some(*i as u64),
        Value::Number(n) => Some(*n as u64),
        Value::String(s) => {
            let t = s.to_str().ok()?;
            let t = t.trim();
            let (digits, radix) = t
                .strip_prefix("0x")
                .or_else(|| t.strip_prefix("0X"))
                .map_or((t, 10), |d| (d, 16));
            u64::from_str_radix(digits, radix).ok()
        }
        _ => None,
    }
}

/// Table `Live` — **lecture** de la mémoire du process du jeu en cours d'exécution.
///
/// Sans équivalent chez Overload (qui possède le process qu'il exécute). Ici, l'intérêt est
/// d'inspecter le jeu VIVANT depuis un script Lua : suivre un pointeur, lire une structure, voir
/// une valeur réelle à l'instant T — au lieu d'un dump figé ou d'un binaire Rust à recompiler pour
/// chaque expérience.
///
/// **Strictement en lecture.** Ce binder n'expose que `FindProcess`/`Read`/`ReadU32`/`ReadU64` ;
/// il n'écrit jamais dans le process. Les fermetures qui l'alimentent (`find_process`, `read`)
/// s'appuient sur `nie-trace`, dont la surface est elle-même lecture seule. Aucun octet n'est écrit
/// dans `nie.exe` par ce chemin.
pub struct LiveBinder<F, R>
where
    F: Fn() -> Option<(i64, Option<u64>)> + 'static,
    R: Fn(u64, usize) -> Option<Vec<u8>> + 'static,
{
    /// Renvoie `(pid, base_du_module)` si le jeu tourne, `None` sinon.
    pub find_process: Rc<F>,
    /// Lit `len` octets à `addr`. `None` si la lecture échoue (adresse non mappée, process absent,
    /// permission refusée — EAC actif, p. ex.).
    pub read: Rc<R>,
}

impl<F, R> HostBinder for LiveBinder<F, R>
where
    F: Fn() -> Option<(i64, Option<u64>)> + 'static,
    R: Fn(u64, usize) -> Option<Vec<u8>> + 'static,
{
    fn name(&self) -> &'static str {
        "Live"
    }

    fn provides(&self) -> Vec<String> {
        vec!["Live".to_string()]
    }

    fn bind(&self, lua: &Lua) -> mlua::Result<()> {
        let table = lua.create_table()?;

        let find = Rc::clone(&self.find_process);
        table.set(
            "FindProcess",
            lua.create_function(move |lua, ()| match find() {
                Some((pid, base)) => {
                    let t = lua.create_table()?;
                    t.set("pid", pid)?;
                    // Base en chaîne hexadécimale : cohérent avec la façon dont une adresse
                    // s'écrit, et ré-utilisable tel quel dans `Live.Read`.
                    if let Some(b) = base {
                        t.set("base", format!("0x{b:x}"))?;
                    }
                    Ok(Value::Table(t))
                }
                None => Ok(Value::Nil),
            })?,
        )?;

        let read = Rc::clone(&self.read);
        table.set(
            "Read",
            lua.create_function(move |lua, (addr, len): (Value, i64)| {
                let addr = parse_live_addr(&addr).ok_or_else(|| {
                    mlua::Error::RuntimeError("Live.Read : adresse invalide".into())
                })?;
                if len <= 0 || len as usize > LIVE_READ_MAX {
                    return Err(mlua::Error::RuntimeError(format!(
                        "Live.Read : longueur hors bornes (1..={LIVE_READ_MAX})"
                    )));
                }
                // Octets bruts en chaîne Lua : à décoder avec `string.byte`/`string.unpack`. Une
                // conversion texte corromprait des octets non-UTF-8, qui sont la règle en mémoire.
                match read(addr, len as usize) {
                    Some(bytes) => Ok(Value::String(lua.create_string(&bytes)?)),
                    None => Ok(Value::Nil),
                }
            })?,
        )?;

        // Deux raccourcis de lecture d'entier petit-boutiste — le geste de base du suivi de
        // pointeur, qu'il serait pénible de réécrire en `string.byte` à chaque fois.
        let read_u32 = Rc::clone(&self.read);
        table.set(
            "ReadU32",
            lua.create_function(move |_, addr: Value| {
                let addr = parse_live_addr(&addr).ok_or_else(|| {
                    mlua::Error::RuntimeError("Live.ReadU32 : adresse invalide".into())
                })?;
                Ok(read_u32(addr, 4)
                    .and_then(|b| b.try_into().ok())
                    .map(|a: [u8; 4]| u32::from_le_bytes(a) as i64))
            })?,
        )?;

        let read_u64 = Rc::clone(&self.read);
        table.set(
            "ReadU64",
            lua.create_function(move |_, addr: Value| {
                let addr = parse_live_addr(&addr).ok_or_else(|| {
                    mlua::Error::RuntimeError("Live.ReadU64 : adresse invalide".into())
                })?;
                // Renvoyé en nombre Lua (double) : une adresse (< 2⁴⁷) reste exacte ; une valeur
                // 64 bits pleine au-delà de 2⁵³ perdrait ses bits de poids faible — pour ces
                // cas-là, lire les 8 octets bruts avec `Live.Read`.
                Ok(read_u64(addr, 8)
                    .and_then(|b| b.try_into().ok())
                    .map(|a: [u8; 8]| u64::from_le_bytes(a) as f64))
            })?,
        )?;

        lua.globals().set("Live", table)?;
        Ok(())
    }
}

/// Compose plusieurs [`HostBinder`] et retient ce qui a été installé.
///
/// Équivalent du `LuaBinder::CallBinders` d'Overload, plus la traçabilité : [`Self::installed_names`]
/// donne la liste des globals fournis, ce qui permet à l'outillage de dire « ce script réclame 14
/// fonctions moteur, 3 sont implémentées » au lieu d'un simple échec.
#[derive(Default)]
pub struct HostRegistry {
    binders: Vec<Box<dyn HostBinder>>,
}

impl HostRegistry {
    /// Registre vide.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Ajoute un binder.
    #[must_use]
    pub fn with(mut self, binder: Box<dyn HostBinder>) -> Self {
        self.binders.push(binder);
        self
    }

    /// Registre par défaut : journalisation + maths, adossé au tampon donné.
    #[must_use]
    pub fn standard(sink: LogSink) -> Self {
        Self::new()
            .with(Box::new(DebugBinder::new(sink)))
            .with(Box::new(MathBinder::default()))
    }

    /// Globals installés par l'ensemble des binders, triés.
    #[must_use]
    pub fn installed_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.binders.iter().flat_map(|b| b.provides()).collect();
        names.sort_unstable();
        names.dedup();
        names
    }

    /// Noms des domaines enregistrés.
    #[must_use]
    pub fn binder_names(&self) -> Vec<&'static str> {
        self.binders.iter().map(|b| b.name()).collect()
    }

    /// Installe tous les binders dans `lua`.
    ///
    /// # Errors
    /// [`mlua::Error`] au premier binder qui échoue — un hôte à moitié installé est pire qu'un
    /// hôte absent : le script partirait en croyant l'API disponible.
    pub fn bind_all(&self, lua: &Lua) -> mlua::Result<()> {
        for binder in &self.binders {
            binder.bind(lua)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_journalise_par_niveau() {
        let lua = crate::new_vm();
        let sink: LogSink = Rc::new(RefCell::new(Vec::new()));
        DebugBinder::new(Rc::clone(&sink)).bind(&lua).expect("bind");

        lua.load("Debug.Log('a') Debug.LogWarning('b', 2) Debug.LogError('c')")
            .exec()
            .expect("exécution");

        let entries = sink.borrow();
        assert_eq!(entries.len(), 3);
        assert_eq!(
            entries[0],
            LogEntry {
                level: LogLevel::Info,
                message: "a".into()
            }
        );
        assert_eq!(
            entries[1],
            LogEntry {
                level: LogLevel::Warning,
                message: "b\t2".into()
            }
        );
        assert_eq!(entries[2].level, LogLevel::Error);
    }

    #[test]
    fn math_est_deterministe_et_borne() {
        let run = || {
            let lua = crate::new_vm();
            MathBinder::default().bind(&lua).expect("bind");
            lua.load("local t = {} for i = 1, 20 do t[i] = Math.RandomInt(1, 6) end return table.concat(t, ',')")
                .eval::<String>()
                .expect("exécution")
        };

        let first = run();
        assert_eq!(
            first,
            run(),
            "le générateur doit être reproductible d'une VM à l'autre"
        );
        for value in first.split(',') {
            let n: i64 = value.parse().expect("entier");
            assert!((1..=6).contains(&n), "valeur hors bornes : {n}");
        }
    }

    #[test]
    fn math_lerp() {
        let lua = crate::new_vm();
        MathBinder::default().bind(&lua).expect("bind");
        let v: f64 = lua
            .load("return Math.Lerp(0, 10, 0.25)")
            .eval()
            .expect("exécution");
        assert!((v - 2.5).abs() < 1e-9, "lerp = {v}");
    }

    #[test]
    fn vfs_expose_la_lecture_dassets() {
        let lua = crate::new_vm();
        let binder = VfsBinder {
            list: Rc::new(|prefix: &str| {
                ["data/a.bin", "data/b.bin", "autre/c.bin"]
                    .iter()
                    .filter(|p| p.starts_with(prefix))
                    .map(ToString::to_string)
                    .collect()
            }),
            read: Rc::new(|path: &str| (path == "data/a.bin").then(|| vec![1u8, 2, 3, 4])),
        };
        binder.bind(&lua).expect("bind");

        let count: i64 = lua.load("return #Vfs.List('data/')").eval().expect("liste");
        assert_eq!(count, 2, "le préfixe doit filtrer");

        let size: i64 = lua
            .load("return Vfs.Size('data/a.bin')")
            .eval()
            .expect("taille");
        assert_eq!(size, 4);

        let missing: i64 = lua
            .load("return Vfs.Size('data/inconnu')")
            .eval()
            .expect("taille");
        assert_eq!(
            missing, -1,
            "un fichier absent doit se distinguer d'un fichier vide"
        );

        // Les octets bruts doivent traverser sans conversion lossy.
        let byte: i64 = lua
            .load("return string.byte(Vfs.Read('data/a.bin'), 1)")
            .eval()
            .expect("lecture");
        assert_eq!(byte, 1);
    }

    #[test]
    fn live_lit_la_memoire_sans_jamais_ecrire() {
        // Faux process : base 0x140000000, et un u32 petit-boutiste 0x12345678 à 0x1000.
        let lua = crate::new_vm();
        let binder = LiveBinder {
            find_process: Rc::new(|| Some((4242, Some(0x1_4000_0000)))),
            read: Rc::new(|addr: u64, len: usize| {
                (addr == 0x1000 && len <= 4).then(|| vec![0x78, 0x56, 0x34, 0x12][..len].to_vec())
            }),
        };
        binder.bind(&lua).expect("bind");

        // FindProcess expose pid + base.
        let pid: i64 = lua
            .load("return Live.FindProcess().pid")
            .eval()
            .expect("pid");
        assert_eq!(pid, 4242);
        let base: String = lua
            .load("return Live.FindProcess().base")
            .eval()
            .expect("base");
        assert_eq!(base, "0x140000000");

        // Read renvoie les octets bruts, décodables via string.byte.
        let first: i64 = lua
            .load("return string.byte(Live.Read(0x1000, 4), 1)")
            .eval()
            .expect("read");
        assert_eq!(first, 0x78);

        // ReadU32 recompose l'entier petit-boutiste.
        let value: i64 = lua
            .load("return Live.ReadU32(0x1000)")
            .eval()
            .expect("readu32");
        assert_eq!(value, 0x1234_5678);

        // Une adresse non mappée donne nil, pas une erreur — un script peut sonder sans planter.
        assert_eq!(
            lua.load("return Live.ReadU32(0x9999)")
                .eval::<Value>()
                .unwrap(),
            Value::Nil
        );

        // `Live` n'expose AUCUNE écriture : le contrat lecture seule est vérifiable.
        let has_write: bool = lua
            .load("return Live.Write ~= nil or Live.Poke ~= nil or Live.Set ~= nil")
            .eval()
            .expect("inspection");
        assert!(!has_write, "Live ne doit exposer que de la lecture");
    }

    #[test]
    fn le_registre_compose_et_trace() {
        let sink: LogSink = Rc::new(RefCell::new(Vec::new()));
        let registry = HostRegistry::standard(Rc::clone(&sink));
        assert_eq!(registry.binder_names(), vec!["Debug", "Math"]);
        assert_eq!(
            registry.installed_names(),
            vec!["Debug".to_string(), "Math".to_string()]
        );

        let lua = crate::new_vm();
        registry.bind_all(&lua).expect("bind_all");
        lua.load("Debug.Log(Math.Lerp(0, 4, 0.5))")
            .exec()
            .expect("exécution");
        assert_eq!(sink.borrow()[0].message, "2");
    }
}
