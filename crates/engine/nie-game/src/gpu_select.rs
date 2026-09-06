//! Sélection du backend et de l'adaptateur GPU, par plateforme.
//!
//! niers tourne dans deux environnements aux contraintes opposées, et le même binaire doit
//! servir les deux :
//!
//! - **Windows, poste de travail** — l'objectif est le GPU discret. D3D12 est le backend natif
//!   de la plateforme et celui que le jeu original utilise (D3D11) ; il est prioritaire, et la
//!   préférence de puissance vise explicitement la carte NVIDIA/AMD dédiée. Sur une machine
//!   Optimus, une préférence neutre choisit l'iGPU Intel et divise les performances.
//! - **Serveur Linux sans GPU** — il n'y a ni D3D12 ni pilote matériel. Vulkan logiciel
//!   (lavapipe) rend le pipeline, et c'est aussi la référence de rendu déterministe : un
//!   rasteriseur logiciel produit les mêmes octets d'un vendeur à l'autre, ce qu'aucun GPU ne
//!   garantit.
//!
//! Rien n'est codé en dur pour un poste donné : la plateforme décide, et `NIE_WGPU_BACKEND`
//! (`dx12`, `vulkan`, `metal`, `gl`, `all`) ainsi que `NIE_WGPU_FORCE_FALLBACK=1` permettent de
//! surcharger, notamment pour reproduire un rendu de référence sur une machine à GPU.

use tracing::{debug, info, warn};

/// Backend imposé par `NIE_WGPU_BACKEND`, s'il l'est.
fn backend_impose() -> Option<wgpu::Backends> {
    let v = std::env::var("NIE_WGPU_BACKEND").ok()?;
    let choisi = match v.trim().to_ascii_lowercase().as_str() {
        "dx12" | "d3d12" | "directx" => Some(wgpu::Backends::DX12),
        "vulkan" | "vk" => Some(wgpu::Backends::VULKAN),
        "metal" => Some(wgpu::Backends::METAL),
        "gl" | "opengl" | "gles" => Some(wgpu::Backends::GL),
        "all" => Some(wgpu::Backends::all()),
        autre => {
            warn!("NIE_WGPU_BACKEND={autre} inconnu — sélection automatique");
            None
        }
    };
    if let Some(b) = choisi {
        info!("backend wgpu imposé par NIE_WGPU_BACKEND : {b:?}");
    }
    choisi
}

/// Backends à essayer, **dans l'ordre de préférence**.
///
/// L'ordre compte : passer plusieurs backends d'un coup à `Instance::new` laisse wgpu trancher,
/// et son choix n'est pas le nôtre. On essaie donc un backend à la fois et on garde le premier
/// qui expose un adaptateur.
#[must_use]
pub fn backends_preferes() -> Vec<wgpu::Backends> {
    if let Some(b) = backend_impose() {
        return vec![b];
    }

    // D3D12 d'abord sur Windows : backend natif de la plateforme, celui que le jeu original
    // utilise (D3D11) et que les pilotes NVIDIA/AMD servent en premier. Vulkan en repli pour les
    // machines dont le pilote D3D12 est absent ou trop ancien.
    #[cfg(target_os = "windows")]
    {
        vec![wgpu::Backends::DX12, wgpu::Backends::VULKAN]
    }
    // Vulkan seul sur Linux : GLES/Zink échoue à initialiser une surface Wayland sous WSLg
    // (DRI2/ZINK → SIGSEGV), et lavapipe couvre le cas sans GPU.
    #[cfg(target_os = "linux")]
    {
        vec![wgpu::Backends::VULKAN]
    }
    #[cfg(target_os = "macos")]
    {
        vec![wgpu::Backends::METAL]
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        vec![wgpu::Backends::all()]
    }
}

/// Préférence de puissance : le GPU **discret** quand la plateforme en expose un.
///
/// `HighPerformance` est ce qui fait choisir la NVIDIA plutôt que l'Intel intégrée sur un
/// portable à double GPU. Sur un serveur sans matériel, la préférence est sans effet — il n'y a
/// qu'un adaptateur, logiciel.
#[must_use]
pub fn preference_puissance() -> wgpu::PowerPreference {
    wgpu::PowerPreference::HighPerformance
}

/// `true` si l'appelant doit sauter l'essai matériel et aller droit au rendu logiciel.
///
/// Utile pour produire une référence de rendu reproductible sur une machine équipée : le
/// rasteriseur logiciel ne dépend pas du vendeur du pilote.
#[must_use]
pub fn fallback_impose() -> bool {
    matches!(
        std::env::var("NIE_WGPU_FORCE_FALLBACK").as_deref(),
        Ok("1" | "true")
    )
}

/// Instance wgpu sur un backend donné.
fn instance_sur(backends: wgpu::Backends) -> wgpu::Instance {
    wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    })
}

/// Instance wgpu sur le premier backend préféré qui expose un adaptateur.
///
/// L'énumération est le seul test fiable : une instance se crée sans erreur sur un backend dont
/// aucun pilote n'est installé, et l'échec ne se manifeste qu'à la demande d'adaptateur. On
/// interroge donc chaque backend, dans l'ordre, et on retient le premier qui répond.
#[must_use]
pub fn instance() -> wgpu::Instance {
    let candidats = backends_preferes();
    let dernier = candidats.len() - 1;
    for (i, backends) in candidats.into_iter().enumerate() {
        let instance = instance_sur(backends);
        // Le dernier candidat est rendu sans condition : mieux vaut laisser l'appelant échouer
        // sur une demande d'adaptateur, avec son message, qu'inventer un repli ici.
        if i == dernier {
            debug!("instance wgpu sur {backends:?} (dernier candidat)");
            return instance;
        }
        if pollster::block_on(instance.enumerate_adapters(backends)).is_empty() {
            debug!("aucun adaptateur sur {backends:?} — backend suivant");
            continue;
        }
        info!("backend wgpu retenu : {backends:?}");
        return instance;
    }
    unreachable!("backends_preferes() ne rend jamais une liste vide")
}

/// Décrit un adaptateur en une ligne lisible : nom, backend, type de matériel.
///
/// Vaut la peine d'être journalisé à chaque démarrage — c'est ce qui permet de distinguer
/// « le rendu est lent » de « le rendu tourne sur l'iGPU ou sur lavapipe ».
#[must_use]
pub fn decrire(adapter: &wgpu::Adapter) -> String {
    let info = adapter.get_info();
    let genre = match info.device_type {
        wgpu::DeviceType::DiscreteGpu => "GPU discret",
        wgpu::DeviceType::IntegratedGpu => "GPU intégré",
        wgpu::DeviceType::VirtualGpu => "GPU virtuel",
        wgpu::DeviceType::Cpu => "logiciel (CPU)",
        wgpu::DeviceType::Other => "type inconnu",
    };
    format!(
        "{} — {genre}, backend {:?}, pilote « {} »",
        info.name, info.backend, info.driver_info
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// La sélection par défaut n'est jamais vide : un `Backends` vide ferait échouer toute
    /// énumération d'adaptateur avec un message opaque.
    #[test]
    fn backends_par_defaut_non_vides() {
        // SAFETY-libre : on lit seulement, sans poser la variable.
        if std::env::var("NIE_WGPU_BACKEND").is_err() {
            assert!(!backends_preferes().is_empty());
        }
    }

    /// Sur Windows, D3D12 doit être proposé : c'est le backend natif, et celui que le pilote
    /// NVIDIA sert le mieux. Ce test verrouille l'intention contre une régression silencieuse.
    #[cfg(target_os = "windows")]
    #[test]
    fn windows_propose_d3d12() {
        if std::env::var("NIE_WGPU_BACKEND").is_err() {
            assert_eq!(backends_preferes().first(), Some(&wgpu::Backends::DX12));
        }
    }

    /// La préférence vise le matériel le plus puissant — sans quoi un portable Optimus rend sur
    /// son iGPU sans que rien ne le signale.
    #[test]
    fn preference_vise_le_gpu_discret() {
        assert_eq!(
            preference_puissance(),
            wgpu::PowerPreference::HighPerformance
        );
    }
}
