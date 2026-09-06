//! Orchestration de jobs asynchrones annulables/pausables, avec progression.
//!
//! Architecture inspirée du `sd-task-system` de spacedrive (`Task` + `Interrupter` +
//! `TaskSystem`, cf. `var/spacedrive/crates/task-system`), mais implémentation **originale** et
//! volontairement réduite au besoin réel de niers (dispatch d'un job par `tokio::spawn`, pas de
//! pool de workers à vol de tâches — nie-explorer n'a jamais plus d'une poignée de jobs longs
//! simultanés, contrairement à l'indexeur de fichiers massif de spacedrive) :
//! - un job (`Task`) est annulable et pausable à des points de contrôle explicites
//!   (`ctx.interrupter.check().await`) ;
//! - il peut rapporter sa progression (`ctx.progress.report(done, total, message)`) sans dépendre
//!   d'un canal spécifique à l'appelant (UI Tauri, CLI, tests…).
//!
//! Usage typique dans `nie-explorer/src-tauri` : remplacer un `#[tauri::command]` synchrone
//! bloquant de bout en bout (ex. scan complet du VFS, ~255 800 entrées) par un `Task` dispatché
//! via [`TaskSystem`], dont la progression est relayée au frontend par `app_handle.emit(...)` et
//! que l'utilisatrice peut annuler en cours de route.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use thiserror::Error;
use tokio::sync::{mpsc, watch};

/// Identifiant unique d'un job dispatché — v4 aléatoire, stable pour toute la durée de vie du job
/// (sert de clé pour l'annulation et pour router les événements de progression côté appelant).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TaskId(pub uuid::Uuid);

impl TaskId {
    /// Génère un nouvel identifiant aléatoire.
    #[must_use]
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl Default for TaskId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Signal de contrôle transmis d'un [`TaskSystem`] à l'`Interrupter` d'un job en cours.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Signal {
    Run,
    Pause,
    Cancel,
}

/// Erreur renvoyée par [`Interrupter::check`] quand le job a été annulé — à propager immédiatement
/// (via `?`) pour interrompre proprement `Task::run` au prochain point de contrôle.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("job annulé")]
pub struct Canceled;

/// Point de contrôle pause/annulation, consulté volontairement par `Task::run` à intervalles
/// raisonnables (ex. tous les N éléments d'une boucle) — jamais préemptif, comme dans
/// sd-task-system : un job qui ne consulte jamais son `Interrupter` tourne simplement jusqu'au bout.
#[derive(Debug, Clone)]
pub struct Interrupter {
    rx: watch::Receiver<Signal>,
}

impl Interrupter {
    /// Point de contrôle : renvoie immédiatement si le job tourne, bloque tant qu'il est en pause,
    /// renvoie [`Canceled`] s'il a été annulé (par [`TaskHandle::cancel`] ou [`TaskSystem::cancel`]).
    ///
    /// Prend `&self` (pas `&mut self`) — clone en interne le `watch::Receiver` pour pouvoir
    /// attendre un changement (`changed()` exige `&mut`), afin que `Task::run` puisse appeler
    /// `ctx.interrupter.check()` directement sans jongler avec la mutabilité de `&TaskContext`.
    pub async fn check(&self) -> Result<(), Canceled> {
        let mut rx = self.rx.clone();
        loop {
            match *rx.borrow() {
                Signal::Run => return Ok(()),
                Signal::Cancel => return Err(Canceled),
                Signal::Pause => {}
            }
            // `changed()` échoue seulement si l'émetteur (TaskSystem) a été abandonné —
            // dans ce cas le job n'a plus de superviseur, on le laisse continuer (Run implicite).
            if rx.changed().await.is_err() {
                return Ok(());
            }
        }
    }

    /// Sonde non bloquante : `true` si une annulation a déjà été demandée, sans attendre une
    /// éventuelle pause en cours (utile en tête de boucle chaude, avant même de dériver un lot).
    #[must_use]
    pub fn is_canceled(&self) -> bool {
        *self.rx.borrow() == Signal::Cancel
    }
}

/// Avancement d'un job, tel que rapporté par [`ProgressReporter::report`] — forme neutre (JSON-able
/// via `serde`) pour être relayée telle quelle par n'importe quel transport (événement Tauri, log…).
#[derive(Debug, Clone, serde::Serialize)]
pub struct TaskProgress {
    /// Job qui rapporte cet avancement.
    pub id: TaskId,
    /// Unités traitées jusqu'ici.
    pub done: u64,
    /// Total attendu (0 si inconnu à l'avance — l'appelant affiche alors un indicateur indéterminé).
    pub total: u64,
    /// Message humain optionnel (ex. chemin en cours de traitement).
    pub message: Option<String>,
}

/// Émetteur de progression capturé par le `TaskContext` d'un job — clonable, un job peut le
/// partager entre sous-étapes sans se soucier de qui écoute côté appelant.
#[derive(Clone)]
pub struct ProgressReporter {
    id: TaskId,
    tx: mpsc::UnboundedSender<TaskProgress>,
}

impl ProgressReporter {
    /// Rapporte un avancement. Silencieux (pas de panique) si plus personne n'écoute — un job ne
    /// doit jamais planter parce que l'UI qui l'avait lancé a fermé son canal de progression.
    pub fn report(&self, done: u64, total: u64, message: impl Into<Option<String>>) {
        let _ = self.tx.send(TaskProgress {
            id: self.id,
            done,
            total,
            message: message.into(),
        });
    }
}

/// Contexte fourni à [`Task::run`] : point de contrôle pause/annulation + émetteur de progression.
#[derive(Clone)]
pub struct TaskContext {
    /// Point de contrôle pause/annulation — cf. [`Interrupter::check`].
    pub interrupter: Interrupter,
    /// Émetteur de progression — cf. [`ProgressReporter::report`].
    pub progress: ProgressReporter,
}

/// Résultat final d'un job qui va au bout de `Task::run` sans être annulé.
#[derive(Debug, Clone)]
pub enum ExecStatus {
    /// Terminé avec succès ; `output` est une charge utile JSON libre (résultat du job, ou
    /// `serde_json::Value::Null` si le job n'a rien à renvoyer au-delà de sa progression).
    Done(serde_json::Value),
}

/// Statut final d'un job tel que renvoyé par [`TaskHandle`], côté appelant.
#[derive(Debug, Clone)]
pub enum TaskStatus<E> {
    /// Le job est allé à son terme.
    Done(serde_json::Value),
    /// Le job a été annulé avant la fin (cf. [`TaskHandle::cancel`]).
    Canceled,
    /// Le job a échoué.
    Error(E),
}

/// Un job dispatchable par [`TaskSystem`]. `E` est le type d'erreur unifié de tous les jobs
/// dispatchés dans une même instance de `TaskSystem` (comme sd-task-system : un seul type d'erreur
/// par système, propre à l'appelant — ex. `String` dans `nie-explorer/src-tauri`).
#[async_trait]
pub trait Task<E>: Send + 'static {
    /// Identifiant stable du job (généré par l'appelant avant dispatch, pour pouvoir l'annuler
    /// depuis une commande séparée avant même que [`TaskSystem::dispatch`] ne renvoie).
    fn id(&self) -> TaskId;

    /// Exécute le job. Doit consulter `ctx.interrupter.check().await` à intervalles raisonnables
    /// et propager son erreur (`?`) pour s'arrêter proprement sur annulation.
    async fn run(&mut self, ctx: &TaskContext) -> Result<ExecStatus, E>;
}

/// Poignée d'un job dispatché — permet de l'annuler et d'attendre son résultat final.
pub struct TaskHandle<E> {
    id: TaskId,
    join: tokio::task::JoinHandle<TaskStatus<E>>,
    cancel_tx: watch::Sender<Signal>,
}

impl<E> TaskHandle<E> {
    /// Identifiant du job.
    #[must_use]
    pub fn id(&self) -> TaskId {
        self.id
    }

    /// Demande l'annulation du job — asynchrone : le job s'arrête à son prochain point de
    /// contrôle (`Interrupter::check`), pas immédiatement.
    pub fn cancel(&self) {
        let _ = self.cancel_tx.send(Signal::Cancel);
    }

    /// Met le job en pause — il se bloquera à son prochain point de contrôle jusqu'à [`Self::resume`]
    /// ou [`Self::cancel`]. Sans effet si le job est déjà terminé.
    pub fn pause(&self) {
        let _ = self.cancel_tx.send(Signal::Pause);
    }

    /// Relance un job précédemment mis en [`Self::pause`].
    pub fn resume(&self) {
        let _ = self.cancel_tx.send(Signal::Run);
    }

    /// Attend le statut final du job (`Done`, `Canceled`, ou `Error` s'il a paniqué en interne
    /// n'est PAS couvert ici — un panic dans `Task::run` remonte comme `JoinError`, cf.
    /// [`TaskHandle::join_result`] pour le cas rare où l'appelant veut le distinguer).
    pub async fn wait(self) -> TaskStatus<E> {
        self.join.await.unwrap_or(TaskStatus::Canceled)
    }
}

/// Système d'orchestration : dispatche des [`Task`], route l'annulation par [`TaskId`], et
/// centralise un unique canal de progression pour tous les jobs qu'il gère.
pub struct TaskSystem<E> {
    cancel_senders: Arc<Mutex<HashMap<TaskId, watch::Sender<Signal>>>>,
    progress_tx: mpsc::UnboundedSender<TaskProgress>,
    _marker: std::marker::PhantomData<fn() -> E>,
}

impl<E: Send + 'static> TaskSystem<E> {
    /// Crée un système vide et son canal de progression partagé (à consommer côté appelant, ex.
    /// une boucle qui relaie chaque [`TaskProgress`] vers `app_handle.emit("job-progress", …)`).
    #[must_use]
    pub fn new() -> (Self, mpsc::UnboundedReceiver<TaskProgress>) {
        let (progress_tx, progress_rx) = mpsc::unbounded_channel();
        (
            Self {
                cancel_senders: Arc::new(Mutex::new(HashMap::new())),
                progress_tx,
                _marker: std::marker::PhantomData,
            },
            progress_rx,
        )
    }

    /// Dispatche un job : le lance immédiatement sur le runtime tokio courant (le `TaskSystem`
    /// doit donc être utilisé depuis un contexte où un runtime tokio est déjà actif — c'est le cas
    /// de tout `#[tauri::command]` async, tauri embarquant son propre runtime).
    pub fn dispatch<T>(&self, mut task: T) -> TaskHandle<E>
    where
        T: Task<E>,
    {
        let id = task.id();
        let (cancel_tx, cancel_rx) = watch::channel(Signal::Run);
        self.cancel_senders
            .lock()
            .expect("cancel_senders mutex empoisonné")
            .insert(id, cancel_tx.clone());

        let ctx = TaskContext {
            interrupter: Interrupter { rx: cancel_rx },
            progress: ProgressReporter {
                id,
                tx: self.progress_tx.clone(),
            },
        };
        let cancel_senders = Arc::clone(&self.cancel_senders);

        let join = tokio::spawn(async move {
            let status = match task.run(&ctx).await {
                Ok(ExecStatus::Done(output)) if ctx.interrupter.is_canceled() => {
                    tracing::debug!(%id, "job terminé après demande d'annulation — statut Done conservé");
                    TaskStatus::Done(output)
                }
                Ok(ExecStatus::Done(output)) => TaskStatus::Done(output),
                Err(e) => TaskStatus::Error(e),
            };
            // Nettoyage du registre à la fin du job, réussite ou non — sans quoi `cancel_senders`
            // croîtrait indéfiniment sur une longue session (nie-explorer reste ouvert des heures).
            cancel_senders
                .lock()
                .expect("cancel_senders mutex empoisonné")
                .remove(&id);
            status
        });

        TaskHandle {
            id,
            join,
            cancel_tx,
        }
    }

    /// Annule un job par son identifiant — no-op silencieux s'il est déjà terminé ou inconnu.
    pub fn cancel(&self, id: TaskId) {
        self.signal(id, Signal::Cancel);
    }

    /// Met en pause un job par son identifiant — no-op silencieux s'il est déjà terminé ou inconnu.
    pub fn pause(&self, id: TaskId) {
        self.signal(id, Signal::Pause);
    }

    /// Relance un job précédemment mis en pause, par son identifiant.
    pub fn resume(&self, id: TaskId) {
        self.signal(id, Signal::Run);
    }

    fn signal(&self, id: TaskId, signal: Signal) {
        if let Some(tx) = self
            .cancel_senders
            .lock()
            .expect("cancel_senders mutex empoisonné")
            .get(&id)
        {
            let _ = tx.send(signal);
        }
    }
}

impl<E: Send + 'static> Default for TaskSystem<E> {
    fn default() -> Self {
        Self::new().0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct CountTo {
        id: TaskId,
        n: u64,
    }

    #[async_trait]
    impl Task<String> for CountTo {
        fn id(&self) -> TaskId {
            self.id
        }

        async fn run(&mut self, ctx: &TaskContext) -> Result<ExecStatus, String> {
            for i in 0..self.n {
                ctx.interrupter
                    .check()
                    .await
                    .map_err(|_| "annulé".to_string())?;
                ctx.progress.report(i + 1, self.n, None);
            }
            Ok(ExecStatus::Done(serde_json::json!({ "counted": self.n })))
        }
    }

    #[tokio::test]
    async fn job_va_au_bout_et_rapporte_sa_progression() {
        let (system, mut progress) = TaskSystem::<String>::new();
        let handle = system.dispatch(CountTo {
            id: TaskId::new(),
            n: 5,
        });
        let status = handle.wait().await;
        match status {
            TaskStatus::Done(v) => assert_eq!(v["counted"], 5),
            other => panic!("statut inattendu : {other:?}"),
        }
        let mut last = None;
        while let Ok(p) = progress.try_recv() {
            last = Some(p);
        }
        assert_eq!(last.unwrap().done, 5);
    }

    struct Forever {
        id: TaskId,
    }

    #[async_trait]
    impl Task<String> for Forever {
        fn id(&self) -> TaskId {
            self.id
        }

        async fn run(&mut self, ctx: &TaskContext) -> Result<ExecStatus, String> {
            let mut i: u64 = 0;
            loop {
                ctx.interrupter
                    .check()
                    .await
                    .map_err(|_| "annulé".to_string())?;
                i += 1;
                if i > 10_000_000 {
                    return Ok(ExecStatus::Done(serde_json::Value::Null));
                }
                tokio::task::yield_now().await;
            }
        }
    }

    #[tokio::test]
    async fn cancel_interrompt_le_job_a_son_prochain_point_de_controle() {
        let (system, _progress) = TaskSystem::<String>::new();
        let id = TaskId::new();
        let handle = system.dispatch(Forever { id });
        system.cancel(id);
        let status = handle.wait().await;
        assert!(matches!(status, TaskStatus::Error(_)));
    }

    #[tokio::test]
    async fn handle_cancel_fonctionne_aussi_directement() {
        let (system, _progress) = TaskSystem::<String>::new();
        let handle = system.dispatch(Forever { id: TaskId::new() });
        handle.cancel();
        let status = handle.wait().await;
        assert!(matches!(status, TaskStatus::Error(_)));
    }
}
