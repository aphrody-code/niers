//! Session Lua **persistante** côté application — l'équivalent du `ScriptInterpreter` d'Overload,
//! qui vit tant que l'application vit.
//!
//! ## Pourquoi un thread dédié
//!
//! `mlua::Lua` n'est ni `Send` ni `Sync` (la VM Lua a un état global non protégé, et `nie-lua`
//! utilise `Rc` pour ses tampons partagés). Tauri, lui, exige `Send + Sync` de tout état géré. La
//! VM vit donc **sur son propre thread**, et les commandes lui parlent par messages : c'est la
//! seule façon correcte de garder une VM vivante dans une application multi-thread, et ça évite
//! d'imposer `Send` à toutes les closures d'hôte.
//!
//! ## Ce que ça change pour l'utilisatrice
//!
//! La console devient un vrai REPL. Avant, chaque expression évaluée recréait une VM et
//! réexécutait le script entier : `x = 1` puis `x` répondait `nil`, et évaluer coûtait le prix
//! d'une exécution complète. Ici, l'état survit, et le rechargement est explicite — le
//! `RefreshAll()` d'Overload.

use std::sync::Mutex;
use std::sync::mpsc::{Sender, channel};

use serde::Serialize;

/// Requête envoyée au thread de session.
enum Request {
    /// Exécute un chunk sans l'attacher.
    Exec { name: String, data: Vec<u8>, reply: Sender<Result<Vec<String>, String>> },
    /// Attache un comportement (le script doit renvoyer une table).
    Attach { name: String, data: Vec<u8>, reply: Sender<Result<Vec<String>, String>> },
    /// Diffuse un callback de cycle de vie.
    Broadcast { callback: String, reply: Sender<Result<u32, String>> },
    /// Évalue une expression dans l'état courant.
    Eval { expression: String, reply: Sender<Result<String, String>> },
    /// Pose une valeur globale.
    SetGlobal { name: String, expression: String, reply: Sender<Result<(), String>> },
    /// Liste les globals.
    Globals { include_stdlib: bool, reply: Sender<Vec<LuaSessionGlobalDto>> },
    /// Recrée la VM et ré-attache les comportements.
    Reload { reply: Sender<Result<(), String>> },
    /// Récupère sortie + journaux accumulés.
    Drain { reply: Sender<LuaDrainDto> },
    /// Rapport de couverture d'API.
    ApiReport { reply: Sender<LuaApiReportDto> },
}

/// Sortie accumulée depuis la dernière collecte.
#[derive(Serialize, specta::Type, Default)]
pub struct LuaDrainDto {
    /// Lignes de `print`.
    pub stdout: Vec<String>,
    /// Messages `Debug.*`, préfixés de leur niveau.
    pub logs: Vec<LuaLogDto>,
}

/// Une ligne de journal.
#[derive(Serialize, specta::Type)]
pub struct LuaLogDto {
    /// `info`, `warn` ou `error`.
    pub level: String,
    /// Texte du message.
    pub message: String,
}

/// Un global de la session.
#[derive(Serialize, specta::Type)]
pub struct LuaSessionGlobalDto {
    /// Nom.
    pub name: String,
    /// Type Lua.
    pub type_name: String,
    /// Rendu texte.
    pub value: String,
    /// Nombre d'entrées si table.
    pub len: Option<u32>,
}

/// Ce que les scripts réclament face à ce que l'hôte fournit.
#[derive(Serialize, specta::Type, Debug)]
pub struct LuaApiReportDto {
    /// Globals réclamés mais absents — la liste de travail du portage moteur.
    pub missing: Vec<String>,
    /// Globals fournis par les binders.
    pub provided: Vec<String>,
    /// Part couverte, en pourcentage.
    pub coverage_percent: u32,
}

/// Poignée vers la session : `Send + Sync`, donc gérable par Tauri.
pub struct LuaSessionHandle {
    tx: Mutex<Option<Sender<Request>>>,
}

impl LuaSessionHandle {
    /// Démarre le thread de session.
    #[must_use]
    pub fn start(with_menu_host: bool) -> Self {
        let (tx, rx) = channel::<Request>();

        std::thread::Builder::new()
            .name("nie-lua-session".to_string())
            // Pile large : les scripts de menu du jeu s'appellent en cascade via `INCLUDE`, et la
            // pile par défaut d'un thread Windows (1 Mio) est vite atteinte — même remède que le
            // thread d'export specta.
            .stack_size(16 * 1024 * 1024)
            .spawn(move || {
                let mut session = match build_session(with_menu_host) {
                    Ok(s) => s,
                    Err(e) => {
                        // Sans VM, le thread ne peut rien faire d'utile : on répond une erreur à
                        // chaque requête plutôt que de laisser l'appelant attendre indéfiniment.
                        for req in rx {
                            reply_dead(req, &format!("session Lua indisponible : {e}"));
                        }
                        return;
                    }
                };

                for req in rx {
                    handle(&mut session, req);
                }
            })
            .expect("démarrage du thread de session Lua");

        Self { tx: Mutex::new(Some(tx)) }
    }

}

/// Noms de process du jeu, dans l'ordre d'essai — binaire patché EAC (lancé directement) d'abord,
/// repli sur le nom d'origine (via `EACLauncher.exe`). Même liste que `re_trace`.
const GAME_PROCESS_NAMES: [&str; 2] = ["nie_eacpatched.exe", "nie.exe"];

/// Construit la session avec le registre standard (`Debug`/`Math`) **plus** le binder `Live`,
/// adossé à `nie-trace` en **lecture seule** : un script Lua peut lire la mémoire de `nie.exe` en
/// cours d'exécution (`Live.Read`/`ReadU32`/`ReadU64`/`FindProcess`), jamais y écrire.
///
/// Construit ICI (dans le thread de session), pas dans `nie-lua` : la couche moteur ne doit pas
/// dépendre de `nie-trace` (crate de RE, dossier `forge/`). Les fermetures capturent des `Rc`, ce
/// qui est correct puisqu'elles ne quittent jamais ce thread.
fn build_session(with_menu_host: bool) -> Result<nie_lua::session::LuaSession, nie_lua::LuaError> {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    use nie_lua::host::{HostRegistry, LiveBinder, LogSink};

    let logs: LogSink = Rc::new(RefCell::new(Vec::new()));

    // `FindProcess` : ré-énumère à chaque appel — le jeu peut démarrer après la session.
    let find_process = Rc::new(|| {
        for name in GAME_PROCESS_NAMES {
            if let Some(pid) = nie_trace::find_pid_by_name(name) {
                let base = nie_trace::find_module_base(pid, "nie");
                return Some((i64::from(pid), base));
            }
        }
        None
    });

    // `read` : mémorise le dernier pid trouvé pour ne pas ré-énumérer les process à CHAQUE lecture
    // (un suivi de pointeur en fait des dizaines). Sur échec, le cache est invalidé et on
    // ré-résout une fois — le jeu a pu être relancé avec un nouveau pid.
    let pid_cache: Rc<Cell<Option<i32>>> = Rc::new(Cell::new(None));
    let read = Rc::new(move |addr: u64, len: usize| {
        if let Some(pid) = pid_cache.get() {
            if let Ok(bytes) = nie_trace::read_exact(pid, addr, len) {
                return Some(bytes);
            }
            pid_cache.set(None); // pid périmé (process fermé/relancé)
        }
        for name in GAME_PROCESS_NAMES {
            if let Some(pid) = nie_trace::find_pid_by_name(name) {
                pid_cache.set(Some(pid));
                if let Ok(bytes) = nie_trace::read_exact(pid, addr, len) {
                    return Some(bytes);
                }
            }
        }
        None
    });

    let registry = HostRegistry::standard(Rc::clone(&logs))
        .with(Box::new(LiveBinder { find_process, read }));

    nie_lua::session::LuaSession::new(registry, logs, with_menu_host)
}

impl LuaSessionHandle {
    fn send<T>(&self, make: impl FnOnce(Sender<T>) -> Request) -> Result<T, String> {
        let (reply_tx, reply_rx) = channel::<T>();
        {
            let guard = self.tx.lock().map_err(|_| "session Lua empoisonnée".to_string())?;
            let tx = guard.as_ref().ok_or("session Lua arrêtée")?;
            tx.send(make(reply_tx)).map_err(|_| "session Lua arrêtée".to_string())?;
        }
        reply_rx.recv().map_err(|_| "session Lua sans réponse".to_string())
    }

    /// Exécute un chunk.
    ///
    /// # Errors
    /// Message lisible si la session est indisponible ou si le chunk échoue.
    pub fn exec(&self, name: String, data: Vec<u8>) -> Result<Vec<String>, String> {
        self.send(|reply| Request::Exec { name, data, reply })?
    }

    /// Attache un comportement et renvoie ses callbacks définis.
    ///
    /// # Errors
    /// Message lisible si le script ne renvoie pas de table ou échoue.
    pub fn attach(&self, name: String, data: Vec<u8>) -> Result<Vec<String>, String> {
        self.send(|reply| Request::Attach { name, data, reply })?
    }

    /// Diffuse un callback à tous les comportements ; renvoie combien l'ont réellement défini.
    ///
    /// # Errors
    /// Message lisible si un callback existant échoue.
    pub fn broadcast(&self, callback: String) -> Result<u32, String> {
        self.send(|reply| Request::Broadcast { callback, reply })?
    }

    /// Évalue une expression dans l'état courant.
    ///
    /// # Errors
    /// Message lisible si la session est indisponible.
    pub fn eval(&self, expression: String) -> Result<String, String> {
        self.send(|reply| Request::Eval { expression, reply })?
    }

    /// Pose une valeur globale.
    ///
    /// # Errors
    /// Message lisible si l'expression est invalide.
    pub fn set_global(&self, name: String, expression: String) -> Result<(), String> {
        self.send(|reply| Request::SetGlobal { name, expression, reply })?
    }

    /// Liste les globals.
    ///
    /// # Errors
    /// Message lisible si la session est indisponible.
    pub fn globals(&self, include_stdlib: bool) -> Result<Vec<LuaSessionGlobalDto>, String> {
        self.send(|reply| Request::Globals { include_stdlib, reply })
    }

    /// Recrée la VM et ré-attache les comportements.
    ///
    /// # Errors
    /// Message lisible si la reconstruction échoue.
    pub fn reload(&self) -> Result<(), String> {
        self.send(|reply| Request::Reload { reply })?
    }

    /// Récupère et vide la sortie accumulée.
    ///
    /// # Errors
    /// Message lisible si la session est indisponible.
    pub fn drain(&self) -> Result<LuaDrainDto, String> {
        self.send(|reply| Request::Drain { reply })
    }

    /// Rapport de couverture d'API.
    ///
    /// # Errors
    /// Message lisible si la session est indisponible.
    pub fn api_report(&self) -> Result<LuaApiReportDto, String> {
        self.send(|reply| Request::ApiReport { reply })
    }
}

/// Répond une erreur à une requête quand la session n'a pas pu démarrer.
fn reply_dead(req: Request, msg: &str) {
    match req {
        Request::Exec { reply, .. } | Request::Attach { reply, .. } => {
            let _ = reply.send(Err(msg.to_string()));
        }
        Request::Broadcast { reply, .. } => {
            let _ = reply.send(Err(msg.to_string()));
        }
        Request::Eval { reply, .. } => {
            let _ = reply.send(Err(msg.to_string()));
        }
        Request::SetGlobal { reply, .. } | Request::Reload { reply } => {
            let _ = reply.send(Err(msg.to_string()));
        }
        Request::Globals { reply, .. } => {
            let _ = reply.send(Vec::new());
        }
        Request::Drain { reply } => {
            let _ = reply.send(LuaDrainDto::default());
        }
        Request::ApiReport { reply } => {
            let _ = reply.send(LuaApiReportDto {
                missing: Vec::new(),
                provided: Vec::new(),
                coverage_percent: 0,
            });
        }
    }
}

/// Traite une requête sur le thread de la session.
fn handle(session: &mut nie_lua::session::LuaSession, req: Request) {
    match req {
        Request::Exec { name, data, reply } => {
            let r = session.exec(&name, &data).map_err(|e| e.to_string());
            let _ = reply.send(r);
        }
        Request::Attach { name, data, reply } => {
            let r = session
                .attach(&name, &data)
                .map(|b| b.defined_callbacks().iter().map(|s| (*s).to_string()).collect())
                .map_err(|e| e.to_string());
            let _ = reply.send(r);
        }
        Request::Broadcast { callback, reply } => {
            let r = session.broadcast(&callback).map(|n| n as u32).map_err(|e| e.to_string());
            let _ = reply.send(r);
        }
        Request::Eval { expression, reply } => {
            let r = session.eval(&expression).map_err(|e| e.to_string());
            let _ = reply.send(r);
        }
        Request::SetGlobal { name, expression, reply } => {
            let r = session.set_global(&name, &expression).map_err(|e| e.to_string());
            let _ = reply.send(r);
        }
        Request::Globals { include_stdlib, reply } => {
            let out = session
                .globals(include_stdlib)
                .into_iter()
                .map(|g| LuaSessionGlobalDto {
                    name: g.name,
                    type_name: g.type_name,
                    value: g.value,
                    len: g.len,
                })
                .collect();
            let _ = reply.send(out);
        }
        Request::Reload { reply } => {
            let r = session.reload().map_err(|e| e.to_string());
            let _ = reply.send(r);
        }
        Request::Drain { reply } => {
            let _ = reply.send(LuaDrainDto {
                stdout: session.take_output(),
                logs: session
                    .take_logs()
                    .into_iter()
                    .map(|l| LuaLogDto { level: l.level.label().to_string(), message: l.message })
                    .collect(),
            });
        }
        Request::ApiReport { reply } => {
            let report = session.api_report();
            let _ = reply.send(LuaApiReportDto {
                coverage_percent: report.coverage_percent(),
                missing: report.missing,
                provided: report.provided,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// La propriété centrale : l'état survit d'un appel à l'autre, à travers le canal.
    #[test]
    fn la_session_persiste_entre_deux_appels() {
        let h = LuaSessionHandle::start(false);
        h.eval("compteur = 10".to_string()).expect("eval");
        h.eval("compteur = compteur + 32".to_string()).expect("eval");
        assert_eq!(h.eval("compteur".to_string()).unwrap(), "42");
    }

    #[test]
    fn attache_diffuse_et_recharge() {
        let h = LuaSessionHandle::start(false);
        let callbacks = h
            .attach(
                "c".to_string(),
                b"local M = {} function M.OnStart() marqueur = 1 end return M".to_vec(),
            )
            .expect("attachement");
        assert!(callbacks.contains(&"OnStart".to_string()), "callbacks : {callbacks:?}");

        assert_eq!(h.broadcast("OnStart".to_string()).unwrap(), 1);
        assert_eq!(h.eval("marqueur".to_string()).unwrap(), "1");

        // Le rechargement repart d'une VM neuve : le global posé par le callback disparaît, mais
        // le comportement est ré-attaché (donc rediffusable).
        h.reload().expect("rechargement");
        assert_eq!(h.eval("type(rawget(_G, 'marqueur'))".to_string()).unwrap(), "nil");
        assert_eq!(h.broadcast("OnStart".to_string()).unwrap(), 1);
    }

    #[test]
    fn collecte_sortie_et_journaux() {
        let h = LuaSessionHandle::start(false);
        h.exec("t".to_string(), b"print('ligne') Debug.LogError('grave')".to_vec()).expect("exec");
        let drained = h.drain().expect("drain");
        assert_eq!(drained.stdout, vec!["ligne".to_string()]);
        assert_eq!(drained.logs.len(), 1);
        assert_eq!(drained.logs[0].level, "error");
        // Vidé : une seconde collecte ne rejoue pas les mêmes lignes.
        assert!(h.drain().expect("drain").stdout.is_empty());
    }

    #[test]
    fn rapport_dapi() {
        let h = LuaSessionHandle::start(false);
        h.eval("APPEL_MOTEUR_ABSENT()".to_string()).expect("eval");
        let report = h.api_report().expect("rapport");
        assert!(report.missing.contains(&"APPEL_MOTEUR_ABSENT".to_string()), "{report:?}");
        assert!(report.provided.contains(&"Debug".to_string()), "{report:?}");
    }

    #[test]
    fn edition_de_valeur_en_session_vivante() {
        let h = LuaSessionHandle::start(false);
        h.eval("pv = 100".to_string()).expect("eval");
        h.set_global("pv".to_string(), "777".to_string()).expect("set");
        assert_eq!(h.eval("pv".to_string()).unwrap(), "777");
    }
}
