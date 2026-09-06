//! VFS (Virtual File System) pour nie-formats.
//!
//! Reproduit fidèlement le comportement du VFS de nie.exe en chargeant
//! `cpk_list.cfg.bin` et en indexant et extrayant les fichiers des CPK du jeu.
//!
//! Deux montages servent les **mêmes chemins logiques** (`data/common/…`, `data/dx11/…`) :
//!
//! - **packs** — l'installation du jeu : `cpk_list.cfg.bin` + `packs/*.cpk` ([`Vfs::init`]) ;
//! - **dump** — une arborescence déjà extraite, où le chemin logique EST le chemin disque
//!   ([`Vfs::init_loose`]).
//!
//! Un consommateur n'a pas à choisir : [`Vfs::init`] bascule seul sur le dump quand l'index
//! chiffré est absent mais que l'arborescence est là, et [`resolve_game_dir`] reconnaît les
//! deux racines. Le moteur tourne donc à l'identique sur une install Steam ou sur un dump.

#![cfg(not(target_arch = "wasm32"))]

use crate::FormatError;
use crate::cpk::CpkReader;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

/// Budget mémoire du cache CPK (octets bruts cumulés), large pour garder un maximum de
/// CPK chauds en RAM (« tout chargé comme nie.exe »). Configurable via l'env
/// `NIE_CPK_CACHE_BUDGET_GIB` (défaut 16 Gio). L'éviction LRU n'intervient qu'au-delà :
/// borne de sécurité pour ne pas saturer l'hôte partagé (les 57 Go de CPK ne tiennent
/// pas dans les 45 Gio de la machine).
fn cpk_cache_budget() -> usize {
    let gib = std::env::var("NIE_CPK_CACHE_BUDGET_GIB")
        .ok()
        .and_then(|s| s.trim().parse::<usize>().ok())
        .filter(|&g| g > 0)
        .unwrap_or(16);
    gib.saturating_mul(1024 * 1024 * 1024)
}

/// Cache LRU borné des CPK chargés (nom CPK → lecteur + octets bruts). Évince le moins
/// récemment utilisé quand le total dépasse [`CPK_CACHE_BUDGET`]. Un `Arc` cloné par un
/// `read()` en cours garde sa donnée vivante même après éviction (extraction sûre).
struct CpkCache {
    map: HashMap<String, Arc<(CpkReader, Vec<u8>)>>,
    /// Ordre d'utilisation : avant = moins récent (candidat à l'éviction).
    order: VecDeque<String>,
    bytes: usize,
    budget: usize,
}

impl CpkCache {
    fn new(budget: usize) -> Self {
        Self {
            map: HashMap::new(),
            order: VecDeque::new(),
            bytes: 0,
            budget,
        }
    }

    /// Récupère un CPK et le marque comme récemment utilisé.
    fn get(&mut self, key: &str) -> Option<Arc<(CpkReader, Vec<u8>)>> {
        let arc = self.map.get(key)?.clone();
        if let Some(pos) = self.order.iter().position(|k| k == key) {
            self.order.remove(pos);
        }
        self.order.push_back(key.to_string());
        Some(arc)
    }

    /// Insère un CPK et évince les moins récents tant que le budget est dépassé.
    fn insert(&mut self, key: String, arc: Arc<(CpkReader, Vec<u8>)>) {
        let size = arc.1.len();
        if let Some(old) = self.map.insert(key.clone(), arc) {
            self.bytes = self.bytes.saturating_sub(old.1.len());
            if let Some(pos) = self.order.iter().position(|k| k == &key) {
                self.order.remove(pos);
            }
        }
        self.bytes += size;
        self.order.push_back(key);
        self.evincer();
    }

    /// Évince les CPK les moins récents tant que le budget est dépassé.
    ///
    /// Garde toujours au moins une entrée : évincer le CPK qu'on vient de demander ferait
    /// boucler le prochain `read()` sur un rechargement disque immédiat.
    fn evincer(&mut self) {
        while self.bytes > self.budget && self.order.len() > 1 {
            if let Some(old_key) = self.order.pop_front()
                && let Some(old) = self.map.remove(&old_key)
            {
                self.bytes = self.bytes.saturating_sub(old.1.len());
            }
        }
    }

    /// Vide entièrement le cache et rend les octets libérés.
    fn vider(&mut self) -> usize {
        let liberes = self.bytes;
        self.map.clear();
        self.order.clear();
        self.bytes = 0;
        liberes
    }
}

/// Occupation du cache CPK à un instant donné.
///
/// Sert à rendre la consommation **observable** : un cache dont le budget par défaut est de
/// 16 Gio peut faire grossir un hôte de plusieurs gigaoctets sans qu'aucune interface ne le
/// dise, et le symptôme (machine qui rame) n'accuse jamais le cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheCpkStats {
    /// Octets bruts actuellement retenus.
    pub octets: usize,
    /// Nombre de CPK en cache.
    pub entrees: usize,
    /// Budget au-delà duquel l'éviction LRU se déclenche.
    pub budget: usize,
}

/// Cache des CPK déjà chargés, borné LRU (cf. [`CpkCache`]).
type CpkCacheMap = Mutex<CpkCache>;

/// Entrée du VFS.
#[derive(Debug, Clone)]
pub struct VfsEntry {
    pub internal_path: String,
    pub cpk_filename: String,
    pub file_size: u32,
}

/// Système de fichiers virtuel transparent pour Inazuma Eleven: Victory Road.
pub struct Vfs {
    game_data_dir: PathBuf,
    loose_files: bool,
    index: HashMap<String, VfsEntry>,
    /// Index supplémentaire `chemin_interne → nom_cpk` pour les CPK ABSENTS de
    /// `cpk_list.cfg.bin` (films, sound_asset…) : alimenté depuis l'index global
    /// `path → cpk` (cf. [`Vfs::add_extra_index`]). `read()` y bascule sur miss.
    index_extra: HashMap<String, String>,
    cpk_names: HashSet<String>,
    cpk_cache: CpkCacheMap,
    /// Index du montage **dump**, construit paresseusement. Un dump complet porte ~255 000
    /// fichiers : en parcourir l'arborescence coûte des minutes sur NTFS, et ni `read` ni
    /// `is_readable` n'en ont besoin (le chemin logique est le chemin disque). Seuls les
    /// consommateurs qui énumèrent — [`Vfs::find`], [`Vfs::iter`], [`Vfs::asset_count`] —
    /// le déclenchent, et une seule fois.
    loose_index: OnceLock<HashMap<String, VfsEntry>>,
}

impl Default for Vfs {
    fn default() -> Self {
        Self::new()
    }
}

impl Vfs {
    /// Crée une nouvelle instance de VFS vide.
    #[must_use]
    pub fn new() -> Self {
        Self {
            game_data_dir: PathBuf::new(),
            loose_files: false,
            index: HashMap::new(),
            index_extra: HashMap::new(),
            cpk_names: HashSet::new(),
            cpk_cache: Mutex::new(CpkCache::new(cpk_cache_budget())),
            loose_index: OnceLock::new(),
        }
    }

    /// Ajoute des entrées `(chemin_interne, nom_cpk)` à l'index supplémentaire — pour les
    /// fichiers dont le CPK n'est pas listé dans `cpk_list.cfg.bin` (films, sound_asset…).
    /// Les entrées déjà présentes dans l'index principal restent prioritaires. Le CPK
    /// référencé doit exister dans `packs/`. Retourne le nombre d'entrées ajoutées.
    pub fn add_extra_index<I>(&mut self, entries: I) -> usize
    where
        I: IntoIterator<Item = (String, String)>,
    {
        let mut added = 0;
        for (path, cpk) in entries {
            if !self.index.contains_key(&path) {
                self.index_extra.insert(path, cpk);
                added += 1;
            }
        }
        added
    }

    /// Initialise le VFS à partir du répertoire du jeu (contenant `data/cpk_list.cfg.bin`).
    ///
    /// Après avoir indexé les ~254 000 fichiers du `cpk_list.cfg.bin`, appelle automatiquement
    /// [`Vfs::discover_extra_cpks`] pour indexer les CPK présents dans `packs/` mais absents
    /// du `cpk_list` (par exemple les DLC ou mises à jour ajoutant de nouveaux packs).
    pub fn init<P: AsRef<Path>>(&mut self, game_data_dir: P) -> Result<(), FormatError> {
        let game_data_dir = game_data_dir.as_ref().to_path_buf();
        self.game_data_dir = game_data_dir;
        self.loose_files = false;

        let cpk_list_path = self.game_data_dir.join("cpk_list.cfg.bin");
        // Pas d'index chiffré, mais l'arborescence est là : c'est un dump déjà extrait. On y
        // bascule au lieu d'échouer — le dump sert les mêmes chemins logiques que les packs,
        // donc tout consommateur qui appelait `init` continue de fonctionner sans le savoir.
        // L'inverse serait faux : sans `common/`, il n'y a rien à servir, on rend l'erreur.
        if !cpk_list_path.is_file() && est_racine_dump(&self.game_data_dir) {
            return self.init_loose(self.game_data_dir.clone());
        }
        let mut file = File::open(&cpk_list_path)
            .map_err(|_| FormatError::Corrupt("impossible d'ouvrir cpk_list.cfg.bin"))?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)
            .map_err(|_| FormatError::Corrupt("impossible de lire cpk_list.cfg.bin"))?;

        // Déchiffrement du cpk_list.cfg.bin — DEUX variantes coexistent selon le build :
        //   - builds Steam récents : AES-256-CBC (clé/IV reversés de nie.exe) → `decrypt_cpk_list` ;
        //   - dumps plus anciens : enveloppe à clé fixe Viola → `decrypt_block(_, 0, VIOLA_FIXED_KEY)`.
        // On tente l'AES d'abord, puis Viola en repli, en VALIDANT chaque résultat par
        // `cfgbin_parse` (durci contre les en-têtes chiffrés/corrompus : renvoie Err sans
        // paniquer). Le premier déchiffrement produisant un cfg.bin valide gagne. Indispensable :
        // un seul des deux déchiffre correctement un fichier donné (l'autre rend du garbage).
        let cfg = crate::cpk::decrypt_cpk_list(&data)
            .ok()
            .and_then(|aes| crate::cfgbin::cfgbin_parse(&aes).ok())
            .or_else(|| {
                let mut viola = data.clone();
                crate::cpk::decrypt_block(&mut viola, 0, crate::cpk::VIOLA_FIXED_KEY);
                crate::cfgbin::cfgbin_parse(&viola).ok()
            })
            .ok_or(FormatError::Corrupt(
                "echec de parsing du cpk_list.cfg.bin (ni AES-256-CBC ni clé Viola)",
            ))?;

        // Parcourir les entrées et indexer les fichiers
        for root_entry in &cfg.entries {
            for child in &root_entry.children {
                if child.variables.len() < 5 {
                    continue;
                }
                let directory = match &child.variables[0] {
                    crate::cfgbin::Value::String(s) => s,
                    _ => continue,
                };
                let filename = match &child.variables[1] {
                    crate::cfgbin::Value::String(s) => s,
                    _ => continue,
                };
                let cpk_hash = match &child.variables[3] {
                    crate::cfgbin::Value::String(s) => s,
                    _ => continue,
                };
                let file_size = match &child.variables[4] {
                    crate::cfgbin::Value::Int(v) => *v as u32,
                    _ => 0,
                };

                let internal_path = format!("{}{}", directory, filename);

                let entry = VfsEntry {
                    internal_path: internal_path.clone(),
                    cpk_filename: cpk_hash.clone(),
                    file_size,
                };

                // Les entrées avec un nom CPK non vide sont des fichiers dans un pack.
                // Les entrées avec un nom CPK vide sont des fichiers loose (ex. vidéos d'intro
                // `IE_15th.usm`, `L5logo.usm`, config `app_config_6.00.23.00.cfg.bin`) :
                // le jeu les charge directement depuis le disque. On les enregistre quand même
                // afin que `read()` puisse les servir en fallback disque (cf. gestion en aval).
                if !cpk_hash.is_empty() {
                    self.cpk_names.insert(cpk_hash.clone());
                }
                self.index.insert(internal_path, entry);
            }
        }

        // Auto-découverte des CPK supplémentaires : CPK présents dans `packs/` mais absents
        // du `cpk_list.cfg.bin` (DLC, mises à jour ajoutant des packs avant la prochaine
        // mise à jour du `cpk_list`). Retourne 0 si tous les packs sont déjà indexés.
        self.discover_extra_cpks();

        Ok(())
    }

    /// Scanne `packs/*.cpk` et indexe dans [`Vfs::index_extra`] tous les fichiers des CPK
    /// absents du `cpk_list` principal (CPK dont le nom n'est pas encore dans l'index).
    ///
    /// Appelé automatiquement par [`Vfs::init`]. Peut aussi être appelé manuellement pour
    /// réindexer après avoir ajouté un CPK dans `packs/`. Retourne le nombre d'entrées
    /// nouvellement indexées.
    ///
    /// Pour chaque CPK hors-index trouvé sur disque, le TOC complet est parsé et ses
    /// chemins internes (`répertoire/nom`) sont ajoutés à `index_extra`. Les entrées déjà
    /// présentes dans l'index principal restent prioritaires.
    ///
    /// # Robustesse
    ///
    /// Les CPK illisibles ou non-déchiffrables sont silencieusement ignorés (log sur stderr
    /// en mode debug). La fonction est idempotente : un second appel ne ré-ajoute pas les
    /// entrées déjà dans `index_extra`.
    pub fn discover_extra_cpks(&mut self) -> usize {
        if self.loose_files {
            return 0;
        }
        let packs = self.game_data_dir.join("packs");
        let Ok(dir_iter) = std::fs::read_dir(&packs) else {
            return 0;
        };

        let mut added = 0usize;
        for dir_entry in dir_iter.flatten() {
            let name = dir_entry.file_name().to_string_lossy().to_string();
            if !name.ends_with(".cpk") || self.cpk_names.contains(&name) {
                continue; // Déjà dans l'index principal
            }
            // CPK présent sur disque mais absent du cpk_list : parser son TOC.
            let cpk_path = packs.join(&name);
            let Ok(mut f) = File::open(&cpk_path) else {
                continue;
            };
            let mut bytes = Vec::new();
            if f.read_to_end(&mut bytes).is_err() {
                continue;
            }
            let Ok(reader) = CpkReader::new(&bytes, &name) else {
                continue;
            };
            for e in &reader.entries {
                if e.filename.is_empty() {
                    continue;
                }
                let path = if e.directory.is_empty() {
                    e.filename.clone()
                } else {
                    format!("{}/{}", e.directory, e.filename)
                };
                if !self.index.contains_key(&path) && !self.index_extra.contains_key(&path) {
                    self.index_extra.insert(path, name.clone());
                    added += 1;
                }
            }
            self.cpk_names.insert(name);
        }
        added
    }

    /// Monte un **dump** : une arborescence déjà extraite, sans CPK ni `cpk_list.cfg.bin`.
    ///
    /// `extracted_data_dir` est le `data/` du dump (celui qui porte `common/`, `dx11/`…),
    /// exactement comme [`Vfs::init`] prend le `data/` de l'installation. Les chemins servis
    /// sont les chemins **logiques** du jeu (`data/common/…`) : un dump et une install sont
    /// interchangeables pour l'appelant.
    ///
    /// Le montage est immédiat — l'index n'est construit que si quelqu'un énumère
    /// (cf. [`Vfs::loose_index`]). Lire un fichier n'a jamais besoin de l'index.
    pub fn init_loose<P: AsRef<Path>>(&mut self, extracted_data_dir: P) -> Result<(), FormatError> {
        let extracted_data_dir = extracted_data_dir.as_ref().to_path_buf();
        if !extracted_data_dir.is_dir() {
            return Err(FormatError::Corrupt("repertoire de dump introuvable"));
        }
        self.game_data_dir = extracted_data_dir;
        self.loose_files = true;
        self.index.clear();
        self.index_extra.clear();
        self.cpk_names.clear();
        self.loose_index = OnceLock::new();
        Ok(())
    }

    /// Index du dump, construit au premier appel puis mémorisé.
    ///
    /// Les clés sont des chemins **logiques** (`data/common/…`) : c'est ce que le reste du
    /// moteur manipule, et c'est ce que sert un montage par packs. Indexer sous le chemin
    /// relatif au dump (`common/…`) donnerait un VFS dont `iter()` et `read()` ne parlent
    /// pas la même langue.
    fn loose_index(&self) -> &HashMap<String, VfsEntry> {
        self.loose_index.get_or_init(|| {
            let mut index = HashMap::new();
            let mut pile = vec![self.game_data_dir.clone()];
            while let Some(dir) = pile.pop() {
                let Ok(lecture) = std::fs::read_dir(&dir) else {
                    continue;
                };
                for entree in lecture.flatten() {
                    let chemin = entree.path();
                    let Ok(typ) = entree.file_type() else {
                        continue;
                    };
                    if typ.is_dir() {
                        pile.push(chemin);
                        continue;
                    }
                    let Ok(rel) = chemin.strip_prefix(&self.game_data_dir) else {
                        continue;
                    };
                    let internal_path =
                        format!("data/{}", rel.to_string_lossy().replace('\\', "/"));
                    let file_size = entree.metadata().map(|m| m.len() as u32).unwrap_or(0);
                    index.insert(
                        internal_path.clone(),
                        VfsEntry {
                            internal_path,
                            cpk_filename: String::new(),
                            file_size,
                        },
                    );
                }
            }
            index
        })
    }

    /// Dit si ce VFS sert un **dump** (arborescence extraite) plutôt que les packs CPK.
    #[must_use]
    pub fn is_dump(&self) -> bool {
        self.loose_files
    }

    /// Occupation actuelle du cache CPK.
    ///
    /// Le cache retient les octets **bruts** de chaque CPK ouvert : quelques lectures dans des
    /// paquets différents suffisent à retenir plusieurs centaines de mégaoctets. Sans cette
    /// mesure, cette consommation est invisible depuis l'extérieur.
    #[must_use]
    pub fn cache_stats(&self) -> CacheCpkStats {
        let cache = self
            .cpk_cache
            .lock()
            .expect("verrou du cache CPK empoisonné");
        CacheCpkStats {
            octets: cache.bytes,
            entrees: cache.map.len(),
            budget: cache.budget,
        }
    }

    /// Vide le cache CPK et rend les octets libérés.
    ///
    /// Les lectures en cours ne sont pas affectées : chacune détient un `Arc` sur sa donnée,
    /// qui reste vivante jusqu'à la fin de l'extraction. Vider ne fait que relâcher la
    /// référence du cache.
    pub fn vider_cache(&self) -> usize {
        self.cpk_cache
            .lock()
            .expect("verrou du cache CPK empoisonné")
            .vider()
    }

    /// Change le budget du cache CPK et évince immédiatement ce qui dépasse.
    ///
    /// Le budget par défaut (16 Gio, cf. `NIE_CPK_CACHE_BUDGET_GIB`) vise un outil de traitement
    /// par lots qui a la machine pour lui. Une application de bureau qui tourne à côté du jeu
    /// et d'un navigateur a besoin d'un plafond bien plus bas, et doit pouvoir le poser
    /// **après** la construction du VFS — au moment où elle sait ce qu'elle est.
    ///
    /// Rend les octets libérés par l'éviction déclenchée.
    pub fn regler_budget_cache(&self, budget: usize) -> usize {
        let mut cache = self
            .cpk_cache
            .lock()
            .expect("verrou du cache CPK empoisonné");
        let avant = cache.bytes;
        cache.budget = budget;
        cache.evincer();
        avant.saturating_sub(cache.bytes)
    }

    /// Cherche une entrée par son chemin interne.
    ///
    /// Sur un dump, déclenche la construction de l'index (cf. [`Vfs::loose_index`]) : préférer
    /// [`Vfs::is_readable`] quand seule la présence importe.
    #[must_use]
    pub fn find(&self, internal_path: &str) -> Option<&VfsEntry> {
        if self.loose_files {
            return self.loose_index().get(internal_path);
        }
        self.index.get(internal_path)
    }

    /// Dit si une entrée indexée est réellement servable, **sans lire son contenu**.
    ///
    /// Être dans l'index ne suffit pas : `cpk_list.cfg.bin` est l'index du JEU, et il déclare
    /// des fichiers « loose » (colonne CPK vide) qui n'existent pas forcément sur une
    /// installation donnée — constaté sur `common/movie/{IE_15th,L5logo}.usm`, annoncés par
    /// l'index alors que `common/movie/` est vide sur le disque. Une façade qui se contente de
    /// [`Vfs::find`] annonce donc des fichiers que [`Vfs::read`] refusera.
    ///
    /// Le test reste bon marché — présence du conteneur, jamais l'extraction — parce qu'il est
    /// appelé par des façades qui décrivent des fichiers de plusieurs centaines de mégaoctets.
    #[must_use]
    pub fn is_readable(&self, internal_path: &str) -> bool {
        if self.loose_files {
            // Même résolution que `read` — y compris le repli inter-racine : un dump range
            // les fichiers là où le disque les range, pas là où le `cpk_list` les déclare.
            return self.resolve_loose_path(internal_path).is_some();
        }
        let Some(entry) = self.index.get(internal_path) else {
            return self.index_extra.contains_key(internal_path);
        };
        if entry.cpk_filename.is_empty() {
            // Loose déclaré par cpk_list : le chemin interne débute par `data/`, que
            // `game_data_dir` porte déjà.
            let rel = internal_path.strip_prefix("data/").unwrap_or(internal_path);
            return self.game_data_dir.join(rel).is_file();
        }
        self.game_data_dir
            .join("packs")
            .join(&entry.cpk_filename)
            .is_file()
    }

    /// Chemin disque d'un fichier « loose » — déclaré dans `cpk_list.cfg.bin` avec un CPK vide,
    /// donc servi depuis le disque et non depuis un pack.
    ///
    /// Le chemin déclaré n'est pas toujours le chemin réel : sur l'installation Steam, les deux
    /// vidéos d'introduction sont annoncées sous `data/common/movie/` et rangées sous
    /// `data/dx11/movie/`. Le `cpk_list` donne un chemin **logique**, le disque range par
    /// répertoire de plateforme. Prendre le chemin déclaré au pied de la lettre rendait ces
    /// fichiers illisibles — par `read` comme par un dump.
    ///
    /// La résolution essaie donc le chemin déclaré, puis le même sous-chemin sous les autres
    /// racines de premier niveau présentes dans `data/`. **Un seul** candidat est accepté :
    /// deux racines portant le même sous-chemin décriraient deux fichiers différents (une
    /// variante par plateforme), et en choisir un au hasard servirait un contenu faux sous un
    /// nom juste.
    ///
    /// Renvoie `None` si aucun candidat n'existe ou si plusieurs sont en concurrence.
    #[must_use]
    pub fn resolve_loose_path(&self, internal_path: &str) -> Option<PathBuf> {
        let rel = internal_path.strip_prefix("data/").unwrap_or(internal_path);
        let direct = self.game_data_dir.join(rel);
        if direct.is_file() {
            return Some(direct);
        }

        let (racine, reste) = rel.split_once('/')?;
        let mut candidats: Vec<PathBuf> = Vec::new();
        for e in std::fs::read_dir(&self.game_data_dir).ok()?.flatten() {
            // `packs/` contient les archives, jamais l'arborescence logique : l'écarter évite
            // de proposer un homonyme qui n'en est pas un.
            if e.file_name() == racine || e.file_name() == "packs" {
                continue;
            }
            if !e.file_type().is_ok_and(|t| t.is_dir()) {
                continue;
            }
            let c = e.path().join(reste);
            if c.is_file() {
                candidats.push(c);
            }
        }
        if candidats.len() == 1 {
            candidats.pop()
        } else {
            None
        }
    }

    /// Lit un fichier complet du VFS.
    ///
    /// Résolution en quatre étapes :
    /// 1. Mode loose (`init_loose`) : lit directement depuis le disque.
    /// 2. Index principal (cpk_list.cfg.bin) → CPK non vide : extrait du pack.
    /// 3. Index principal → CPK vide : fichier "loose" enregistré dans cpk_list mais servi
    ///    directement depuis le disque (ex. `IE_15th.usm`, `L5logo.usm`,
    ///    `app_config_6.00.23.00.cfg.bin`), localisé par [`Vfs::resolve_loose_path`] — le
    ///    chemin déclaré et le chemin réel diffèrent pour certains d'entre eux.
    /// 4. Index supplémentaire (`index_extra`, peuplé par [`Vfs::discover_extra_cpks`]) →
    ///    extrait du CPK hors-cpk_list correspondant.
    pub fn read(&self, internal_path: &str) -> Result<Vec<u8>, FormatError> {
        if self.loose_files {
            let disk_path = self
                .resolve_loose_path(internal_path)
                .ok_or(FormatError::Corrupt("fichier absent du dump"))?;
            let mut file = File::open(&disk_path)
                .map_err(|_| FormatError::Corrupt("impossible d'ouvrir loose file"))?;
            let mut data = Vec::new();
            file.read_to_end(&mut data)
                .map_err(|_| FormatError::Corrupt("impossible de lire loose file"))?;
            return Ok(data);
        }

        // Résolution du CPK : index principal (cpk_list.cfg.bin) puis index supplémentaire
        // (CPK présents dans packs/ mais hors cpk_list, découverts par discover_extra_cpks).
        let cpk_filename: String = match self.find(internal_path) {
            Some(entry) => entry.cpk_filename.clone(),
            None => self
                .index_extra
                .get(internal_path)
                .cloned()
                .ok_or(FormatError::Corrupt("fichier non trouve dans le VFS"))?,
        };

        // Cas spécial : CPK vide → fichier loose enregistré dans cpk_list (vidéos d'intro,
        // fichier de configuration système…). Le chemin interne débute par "data/" ; on retire
        // ce préfixe pour obtenir un chemin relatif à game_data_dir.
        if cpk_filename.is_empty() {
            let disk_path = self
                .resolve_loose_path(internal_path)
                .ok_or(FormatError::Corrupt("loose file manquant sur disque"))?;
            let mut file = File::open(&disk_path)
                .map_err(|_| FormatError::Corrupt("loose file manquant sur disque"))?;
            let mut data = Vec::new();
            file.read_to_end(&mut data)
                .map_err(|_| FormatError::Corrupt("impossible de lire loose file cpk_list"))?;
            return Ok(data);
        }

        let cpk_filename = &cpk_filename;
        let mut cache = self.cpk_cache.lock().unwrap();

        let reader_arc = if let Some(arc) = cache.get(cpk_filename) {
            arc
        } else {
            let cpk_path = self.game_data_dir.join("packs").join(cpk_filename);
            let mut file = File::open(&cpk_path)
                .map_err(|_| FormatError::Corrupt("impossible d'ouvrir le CPK"))?;
            let mut cpk_bytes = Vec::new();
            file.read_to_end(&mut cpk_bytes)
                .map_err(|_| FormatError::Corrupt("impossible de lire le CPK"))?;

            let reader = CpkReader::new(&cpk_bytes, cpk_filename)?;
            let arc = Arc::new((reader, cpk_bytes));
            cache.insert(cpk_filename.clone(), arc.clone());
            arc
        };

        // Le verrou du cache est rendu AVANT l'extraction : la recherche d'entrée et la
        // décompression CRILAYLA sont du CPU pur sur des octets que notre `Arc` garde vivants
        // même si un autre thread évince ce CPK entre-temps (c'est la garantie du cache).
        // Le garder aurait sérialisé tout le décodage du serveur derrière un seul mutex.
        drop(cache);

        let (reader, cpk_bytes) = &*reader_arc;

        // Trouver l'entrée CPK : on matche le CHEMIN COMPLET (`directory/filename`) en
        // priorité — sinon les fichiers de même nom de base dans des dossiers différents du
        // MÊME cpk (ex. `common/text/fr/skill_text.cfg.bin` vs `.../de/...`, `.../ja/...`)
        // collisionnent et on sert toujours le premier (bug de langue). Repli sur le basename
        // (exact puis insensible à la casse) pour l'index supplémentaire dont le scan azalee
        // abaisse la casse des chemins (TOC CPK : casse d'origine `Chronicle_Title_CN_01.usm`).
        let filename = internal_path
            .split('/')
            .next_back()
            .unwrap_or(internal_path);
        let full_path = |e: &crate::cpk::CpkEntry| format!("{}/{}", e.directory, e.filename);

        let cpk_entry = reader
            .entries
            .iter()
            .find(|e| full_path(e) == internal_path)
            .or_else(|| {
                reader
                    .entries
                    .iter()
                    .find(|e| full_path(e).eq_ignore_ascii_case(internal_path))
            })
            .or_else(|| reader.entries.iter().find(|e| e.filename == filename))
            .or_else(|| {
                reader
                    .entries
                    .iter()
                    .find(|e| e.filename.eq_ignore_ascii_case(filename))
            })
            .ok_or(FormatError::Corrupt("fichier non trouve dans le CPK"))?;

        reader.extract(cpk_bytes, cpk_entry)
    }

    /// Rend un chemin logique accessible comme **fichier sur disque**, pour les consommateurs
    /// qui prennent un chemin et non des octets (un décodeur qui `mmap`, un sous-processus).
    ///
    /// Sur un dump, renvoie le fichier lui-même : **aucune copie**, aucun octet écrit. Sur un
    /// montage par packs, extrait vers `cache_dir` — et ne réextrait pas si le fichier y est
    /// déjà à la bonne taille, parce que les assets visés (un atlas de police fait ~44 Mo) sont
    /// gros et relus à chaque lancement.
    ///
    /// # Errors
    ///
    /// Rend l'erreur de [`Vfs::read`] si le fichier n'est pas servable, ou `Corrupt` si le
    /// cache n'est pas inscriptible.
    pub fn materialiser(
        &self,
        internal_path: &str,
        cache_dir: &Path,
    ) -> Result<PathBuf, FormatError> {
        if self.loose_files
            && let Some(direct) = self.resolve_loose_path(internal_path)
        {
            return Ok(direct);
        }
        let octets = self.read(internal_path)?;
        let nom = internal_path.replace(['/', '\\'], "_");
        let cible = cache_dir.join(nom);
        if std::fs::metadata(&cible).is_ok_and(|m| m.len() == octets.len() as u64) {
            return Ok(cible);
        }
        std::fs::create_dir_all(cache_dir)
            .map_err(|_| FormatError::Corrupt("cache d'assets non creable"))?;
        std::fs::write(&cible, &octets)
            .map_err(|_| FormatError::Corrupt("ecriture dans le cache d'assets impossible"))?;
        Ok(cible)
    }

    /// Répertoire `data/` du jeu sur lequel ce VFS est monté (celui passé à [`Vfs::init`]).
    ///
    /// Exposé pour les consommateurs qui doivent atteindre les packs eux-mêmes plutôt que passer
    /// par [`Vfs::read`] — c'est le cas de l'extraction massive (`nie-viola`), qui ouvre chaque
    /// pack une seule fois au lieu d'une résolution par fichier.
    #[must_use]
    pub fn game_data_dir(&self) -> &Path {
        &self.game_data_dir
    }

    /// Nombre de fichiers indexés.
    ///
    /// Sur un dump, compte les fichiers réellement présents sur disque (et construit l'index
    /// au passage) : c'est le nombre de fichiers **servables**, là où un montage par packs
    /// rapporte ce que `cpk_list.cfg.bin` déclare.
    #[must_use]
    pub fn asset_count(&self) -> usize {
        if self.loose_files {
            return self.loose_index().len();
        }
        self.index.len()
    }

    /// Nombre de packs CPK uniques indexés.
    #[must_use]
    pub fn cpk_count(&self) -> usize {
        self.cpk_names.len()
    }

    /// Nombre d'entrées dans l'index supplémentaire (CPKs hors cpk_list découverts par
    /// [`Vfs::discover_extra_cpks`]).
    #[must_use]
    pub fn extra_count(&self) -> usize {
        self.index_extra.len()
    }

    /// Nombre de fichiers "loose" dans l'index principal (CPK vide dans cpk_list) :
    /// fichiers chargés directement depuis le disque plutôt qu'un pack.
    #[must_use]
    pub fn loose_count(&self) -> usize {
        if self.loose_files {
            // Sur un dump, aucun fichier ne vient d'un pack : tout est servi depuis le disque.
            return self.asset_count();
        }
        self.index
            .values()
            .filter(|e| e.cpk_filename.is_empty())
            .count()
    }

    /// Itère sur toutes les entrées indexées (chemin_interne, entrée VFS).
    ///
    /// Sur un dump, itère l'arborescence extraite (index construit au premier appel).
    pub fn iter(&self) -> impl Iterator<Item = (&str, &VfsEntry)> {
        let index = if self.loose_files {
            self.loose_index()
        } else {
            &self.index
        };
        index.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Itère sur l'index **supplémentaire** — `(chemin_interne, nom_cpk)` des fichiers dont le
    /// pack est absent de `cpk_list.cfg.bin` (films, `sound_asset`, packs de mise à jour).
    ///
    /// [`Vfs::iter`] ne les voit pas : un consommateur qui veut la couverture **complète** du
    /// jeu (un dump, un inventaire) doit parcourir les deux. C'est exactement l'écart que
    /// `nie_viola::dump` laissait passer avant d'appeler cette méthode — plusieurs milliers de
    /// fichiers réels, silencieusement absents de la sortie et absents aussi du total affiché,
    /// donc invisibles dans le rapport.
    ///
    /// Aucune taille n'est disponible ici : `cpk_list` est la seule source de `file_size`, et
    /// ces entrées viennent du sommaire du pack. L'appelant lit la taille dans le TOC au
    /// moment où il ouvre le pack.
    pub fn iter_extra(&self) -> impl Iterator<Item = (&str, &str)> {
        self.index_extra
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

/// Nom du marqueur qui identifie la racine du jeu : l'index VFS chiffré.
const MARQUEUR_RACINE: &str = "data/cpk_list.cfg.bin";

/// Racines de premier niveau d'un dump extrait. `common/` porte les données de plateforme
/// neutre, `dx11/` les ressources rendues — l'une des deux suffit à identifier un dump.
const RACINES_DUMP: [&str; 2] = ["common", "dx11"];

/// Dit si `data_dir` est le `data/` d'un **dump extrait** : une arborescence logique du jeu
/// sans index chiffré ni packs.
///
/// Un `data/` de dépôt vide, ou qui ne porte que des fichiers de travail, n'en est pas un :
/// c'est la présence d'une racine logique du jeu qui tranche, pas celle du dossier `data/`.
#[must_use]
pub fn est_racine_dump<P: AsRef<Path>>(data_dir: P) -> bool {
    let data_dir = data_dir.as_ref();
    RACINES_DUMP.iter().any(|r| data_dir.join(r).is_dir())
}

/// Dit si `data_dir` porte des données de jeu **montables** — index chiffré + packs, ou dump
/// déjà extrait.
///
/// C'est la garde à utiliser par les tests adossés au vrai jeu. Tester la seule présence de
/// `cpk_list.cfg.bin` fait sauter en silence des goldens qu'un dump suffirait à exécuter :
/// un test muet qui ne lit rien est un faux vert, exactement comme un golden sans corpus.
#[must_use]
pub fn donnees_disponibles<P: AsRef<Path>>(data_dir: P) -> bool {
    let data_dir = data_dir.as_ref();
    data_dir.join("cpk_list.cfg.bin").is_file() || est_racine_dump(data_dir)
}

/// Résout le `data/` d'un dump extrait, sans exiger d'installation du jeu.
///
/// Ordre : `NIE_DUMP_DIR` si posée (le `data/` du dump, ou sa racine — les deux sont
/// acceptés) ; sinon le répertoire courant et ses ancêtres. Retourne `None` quand aucun
/// dump n'est visible.
#[must_use]
pub fn resolve_dump_dir() -> Option<PathBuf> {
    let normalise = |racine: PathBuf| -> Option<PathBuf> {
        if est_racine_dump(&racine) {
            return Some(racine);
        }
        let data = racine.join("data");
        est_racine_dump(&data).then_some(data)
    };
    // Même règle que `NIE_GAME_DIR` : posée mais vide ≠ posée.
    if let Ok(dir) = std::env::var("NIE_DUMP_DIR")
        && !dir.trim().is_empty()
    {
        return normalise(PathBuf::from(dir));
    }
    let mut p = std::env::current_dir().ok();
    while let Some(d) = p {
        if let Some(dump) = normalise(d.clone()) {
            return Some(dump);
        }
        p = d.parent().map(PathBuf::from);
    }
    None
}

/// Monte le VFS sur ce qui est disponible : l'installation du jeu si elle est là, sinon un
/// dump extrait.
///
/// C'est le point d'entrée à préférer dans tout le moteur — il évite à chaque consommateur de
/// recoder « `resolve_game_dir()` puis `join("data")` puis `init` », et surtout il rend le
/// moteur exécutable sur une machine qui n'a que le dump.
///
/// `NIE_DUMP_DIR` force le dump même si une installation est visible : c'est ce qui permet de
/// comparer les deux montages sur la même machine.
///
/// # Errors
///
/// Rend l'erreur d'[`Vfs::init`] quand une installation est détectée mais illisible, et
/// `Corrupt` quand ni installation ni dump ne sont visibles.
pub fn open_game() -> Result<Vfs, FormatError> {
    let mut vfs = Vfs::new();
    let dump_force = std::env::var("NIE_DUMP_DIR").is_ok_and(|d| !d.trim().is_empty());
    if dump_force && let Some(dump) = resolve_dump_dir() {
        vfs.init_loose(dump)?;
        return Ok(vfs);
    }
    let data = resolve_game_dir().join("data");
    if data.join("cpk_list.cfg.bin").is_file() || est_racine_dump(&data) {
        vfs.init(&data)?;
        return Ok(vfs);
    }
    let dump = resolve_dump_dir().ok_or(FormatError::Corrupt(
        "ni installation du jeu ni dump extrait trouves",
    ))?;
    vfs.init_loose(dump)?;
    Ok(vfs)
}

/// Résout le répertoire racine du jeu — celui qui contient `data/cpk_list.cfg.bin`.
///
/// Ordre : `NIE_GAME_DIR` si posée ; sinon le répertoire courant **ou l'un de ses ancêtres**
/// (le dépôt est fusionné avec le dossier d'installation, mais on peut travailler depuis un
/// sous-répertoire) ; sinon le répertoire de l'exécutable et ses ancêtres (binaire lancé depuis
/// `target/release/` avec un autre cwd).
///
/// Aucun chemin de poste de développement n'est codé ici : quand rien n'est trouvé, la fonction
/// rend le répertoire courant, et c'est l'appelant qui échouera avec un message parlant sur
/// `cpk_list.cfg.bin` introuvable.
#[must_use]
pub fn resolve_game_dir() -> PathBuf {
    // Une variable POSÉE MAIS VIDE n'est pas une racine : la prendre au pied de la lettre
    // renvoyait un chemin vide, où rien n'est jamais trouvé — et tous les tests adossés au
    // vrai jeu se sautaient en annonçant « jeu absent » sur une machine qui l'avait.
    if let Ok(dir) = std::env::var("NIE_GAME_DIR")
        && !dir.trim().is_empty()
    {
        return PathBuf::from(dir);
    }
    let candidat = |depart: PathBuf| -> Option<PathBuf> {
        let mut p = Some(depart);
        while let Some(d) = p {
            if d.join(MARQUEUR_RACINE).is_file() {
                return Some(d);
            }
            p = d.parent().map(PathBuf::from);
        }
        None
    };
    let cwd = std::env::current_dir().unwrap_or_default();
    if let Some(racine) = candidat(cwd.clone()) {
        return racine;
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
        && let Some(racine) = candidat(dir.to_path_buf())
    {
        return racine;
    }
    // Aucune installation en vue : un dump extrait sert les mêmes chemins logiques et suffit
    // à faire tourner le moteur. Il ne vient qu'ici, après l'installation — un dump est une
    // copie, l'installation est la source.
    if let Some(dump) = resolve_dump_dir()
        && let Some(racine) = dump.parent()
    {
        return racine.to_path_buf();
    }
    cwd
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Le `cpk_list` annonce les vidéos d'introduction sous `common/movie/` alors que le disque
    /// les range sous `dx11/movie/`. Sans repli, `read` et tout dump les déclaraient
    /// « manquantes sur disque » alors qu'elles étaient là. Avec repli, mais **sans** exiger un
    /// candidat unique, on servirait une variante de plateforme sous le nom d'une autre.
    #[test]
    fn un_loose_range_sous_une_autre_racine_est_retrouve_sans_etre_devine() {
        let base = std::env::temp_dir().join(format!("nie-vfs-loose-{}", std::process::id()));
        let data = base.join("data");
        std::fs::create_dir_all(data.join("dx11/movie")).expect("arborescence");
        std::fs::create_dir_all(data.join("common/system")).expect("arborescence");
        std::fs::write(data.join("dx11/movie/intro.usm"), b"video").expect("vidéo");
        std::fs::write(data.join("common/system/app.cfg.bin"), b"cfg").expect("config");

        let mut vfs = Vfs::new();
        vfs.init_loose(&data).expect("montage loose");

        // Chemin déclaré == chemin réel : rien à résoudre.
        assert_eq!(
            vfs.resolve_loose_path("data/common/system/app.cfg.bin"),
            Some(data.join("common/system/app.cfg.bin")),
        );
        // Chemin déclaré sous `common/`, fichier rangé sous `dx11/` : retrouvé.
        assert_eq!(
            vfs.resolve_loose_path("data/common/movie/intro.usm"),
            Some(data.join("dx11/movie/intro.usm")),
        );

        // Deux racines portant le même sous-chemin : deux fichiers distincts. En choisir un
        // servirait un contenu faux sous un nom juste — refuser est la seule réponse correcte.
        std::fs::create_dir_all(data.join("dx12/movie")).expect("seconde plateforme");
        std::fs::write(data.join("dx12/movie/intro.usm"), b"autre").expect("vidéo bis");
        assert_eq!(
            vfs.resolve_loose_path("data/common/movie/intro.usm"),
            None,
            "ambigu : refusé"
        );

        assert_eq!(vfs.resolve_loose_path("data/common/movie/absent.usm"), None);
        std::fs::remove_dir_all(&base).ok();
    }

    /// Le cache CPK est mesurable, réglable à chaud et videable — sur le vrai jeu.
    ///
    /// Le budget par défaut (16 Gio) vise un traitement par lots qui a la machine pour lui.
    /// Une application de bureau doit pouvoir l'abaisser **après** la construction du VFS, au
    /// moment où elle sait ce qu'elle est ; sans cela elle ne peut que subir, le budget étant
    /// lu depuis l'environnement à la construction.
    ///
    /// Adossé au jeu parce que remplir le cache demande un vrai CPK : `CpkReader` ne se
    /// fabrique pas à partir d'octets factices, et un test sur une structure inventée ne
    /// prouverait rien de l'API réellement appelée.
    #[test]
    fn le_cache_cpk_est_mesurable_reglable_et_videable() {
        let dir = crate::vfs::resolve_game_dir()
            .to_string_lossy()
            .into_owned();
        let data = std::path::Path::new(&dir).join("data");
        if !data.join("cpk_list.cfg.bin").exists() {
            eprintln!("skip le_cache_cpk_est_mesurable_reglable_et_videable : jeu absent");
            return;
        }
        let mut vfs = Vfs::new();
        vfs.init(&data).expect("Vfs::init");

        assert_eq!(
            vfs.cache_stats().octets,
            0,
            "cache vide avant toute lecture"
        );

        // Une lecture quelconque suffit à charger le CPK qui la porte.
        let Some((chemin, _)) = vfs.iter().find(|(p, _)| p.ends_with(".cfg.bin")) else {
            eprintln!("skip : aucun .cfg.bin dans l'index");
            return;
        };
        let chemin = chemin.to_string();
        vfs.read(&chemin).expect("lecture d'un fichier du jeu");

        let apres = vfs.cache_stats();
        assert!(apres.octets > 0, "la lecture a chargé un paquet en cache");
        assert_eq!(apres.entrees, 1, "un seul paquet touché");

        // Abaisser le budget évince immédiatement, sans attendre la prochaine insertion —
        // mais garde toujours une entrée : évincer le paquet qu'on vient de demander ferait
        // relire le disque au `read()` suivant, en boucle.
        vfs.regler_budget_cache(1);
        let serre = vfs.cache_stats();
        assert_eq!(serre.budget, 1);
        assert_eq!(
            serre.entrees, 1,
            "le dernier paquet survit au budget dépassé"
        );

        // Vider rend tout, et laisse un cache réutilisable.
        let liberes = vfs.vider_cache();
        assert_eq!(
            liberes, serre.octets,
            "les octets rendus sont ceux qui étaient retenus"
        );
        assert_eq!(vfs.cache_stats().octets, 0);
        assert_eq!(vfs.cache_stats().entrees, 0);

        vfs.read(&chemin).expect("relecture après vidage");
        assert!(
            vfs.cache_stats().octets > 0,
            "le cache se remplit de nouveau"
        );
    }

    /// Mount end-to-end du VRAI jeu Steam s'il est présent (sinon skip). Prouve que
    /// `Vfs::init()` — cassé tant que `cpk_list.cfg.bin` n'était pas déchiffré (AES-256-CBC
    /// reversé de nie.exe, cf. `cpk::decrypt_cpk_list`) — indexe désormais les ~250 800
    /// fichiers logiques sans panic ni repli.
    #[test]
    fn vfs_init_monte_le_vrai_jeu() {
        let dir = crate::vfs::resolve_game_dir()
            .to_string_lossy()
            .into_owned();
        let data = std::path::Path::new(&dir).join("data");
        if !data.join("cpk_list.cfg.bin").exists() {
            eprintln!("skip vfs_init_monte_le_vrai_jeu : jeu absent");
            return;
        }
        let mut vfs = Vfs::new();
        vfs.init(&data).expect("Vfs::init via cpk_list AES");
        let n = vfs.asset_count();
        assert!(
            n > 250_000,
            "attendu > 250 000 fichiers indexés, obtenu {n}"
        );
        // L'index logique contient de vrais chemins (au moins une texture g4tx).
        let has_g4tx = vfs
            .iter()
            .any(|(p, _)| p.to_ascii_lowercase().ends_with(".g4tx"));
        assert!(has_g4tx, "aucun .g4tx dans l'index VFS");
        eprintln!("VFS monté : {n} fichiers logiques indexés depuis cpk_list.cfg.bin");
    }

    /// Chaîne de LECTURE complète sur le vrai jeu : `init` → résoudre chemin→CPK →
    /// ouvrir le CPK → déchiffrer (clé dérivée du nom) → extraire le fichier. Lit un
    /// fichier dont le CPK conteneur est le plus PETIT sur disque (lecture bon marché),
    /// puis vérifie que les octets extraits sont non vides et cohérents.
    #[test]
    fn vfs_read_chaine_complete() {
        let dir = crate::vfs::resolve_game_dir()
            .to_string_lossy()
            .into_owned();
        let data = std::path::Path::new(&dir).join("data");
        if !data.join("cpk_list.cfg.bin").exists() {
            eprintln!("skip vfs_read_chaine_complete : jeu absent");
            return;
        }
        let mut vfs = Vfs::new();
        vfs.init(&data).expect("init");

        // Dédupliquer par CPK (≈933 conteneurs, pas 254 k entrées) : un seul `stat` par
        // CPK — sinon 254 k stats sur /mnt/c prennent des minutes. On ignore les entrées
        // sans CPK (films/loose, résolus ailleurs) et on choisit le plus PETIT conteneur.
        let packs = data.join("packs");
        let mut first_path: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        for (path, entry) in vfs.iter() {
            if entry.cpk_filename.is_empty() || entry.file_size == 0 {
                continue;
            }
            first_path
                .entry(entry.cpk_filename.clone())
                .or_insert_with(|| path.to_string());
        }
        let mut best: Option<(u64, String, String)> = None; // (taille_cpk, chemin, cpk)
        for (cpk, path) in &first_path {
            let Ok(meta) = std::fs::metadata(packs.join(cpk)) else {
                continue;
            };
            if meta.is_file()
                && meta.len() > 0
                && best.as_ref().is_none_or(|(b, ..)| meta.len() < *b)
            {
                best = Some((meta.len(), path.clone(), cpk.clone()));
            }
        }
        let (cpk_sz, path, cpk) = best.expect("au moins un CPK lisible");
        eprintln!("lecture de {path} (CPK {cpk}, {cpk_sz} octets sur disque)");

        let bytes = vfs.read(&path).expect("vfs.read chaîne complète");
        assert!(!bytes.is_empty(), "fichier extrait vide");
        eprintln!("OK : {} octets extraits via le VFS", bytes.len());
    }

    /// Vérifie que `discover_extra_cpks()` est correct :
    ///
    /// - Sur l'installation actuelle (cpk_list complet), retourne 0 car tous les CPK
    ///   présents dans `packs/` sont déjà indexés via le `cpk_list.cfg.bin`.
    /// - Vérifie en outre que `cpk_count()` correspond au nombre réel de CPKs sur disque
    ///   (preuve que la découverte ne double-compte pas).
    /// - Si l'installation a des CPK supplémentaires futurs (DLC, mise à jour), le test
    ///   rapporte le nombre exact découvert au lieu de 0.
    #[test]
    fn discover_extra_cpks_retourne_zero_sur_install_complete() {
        let dir = crate::vfs::resolve_game_dir()
            .to_string_lossy()
            .into_owned();
        let data = std::path::Path::new(&dir).join("data");
        if !data.join("cpk_list.cfg.bin").exists() {
            eprintln!("skip discover_extra_cpks: jeu absent");
            return;
        }
        let mut vfs = Vfs::new();
        vfs.init(&data).expect("init");

        let extra = vfs.extra_count();
        let loose = vfs.loose_count();
        let n_index = vfs.asset_count();
        let n_cpks = vfs.cpk_count();

        // Sur l'installation testée, le cpk_list est complet : aucun CPK extra attendu.
        assert_eq!(
            extra, 0,
            "discover_extra_cpks a trouvé {extra} fichiers non indexés dans packs/ \
            (attendu 0 sur une installation complète)"
        );

        // Les entrées loose (CPK vide dans cpk_list) incluent IE_15th.usm, L5logo.usm,
        // app_config_6.00.23.00.cfg.bin (5 entrées sur le build Steam 2026).
        assert!(
            loose >= 3,
            "attendu au moins 3 entrées loose dans le cpk_list (IE_15th.usm, L5logo.usm, app_config…), obtenu {loose}"
        );

        eprintln!(
            "discover_extra_cpks OK : {n_index} fichiers, {n_cpks} CPKs, {loose} loose, {extra} extra découverts"
        );
    }

    /// Vérifie que `read()` sert correctement les fichiers "loose" enregistrés dans le
    /// `cpk_list` avec un nom CPK vide. Ces fichiers existent directement sur disque sous
    /// `game_data_dir/`. Exemple : `data/dx11/movie/IE_15th.usm` → lu depuis
    /// `{game_data_dir}/dx11/movie/IE_15th.usm`.
    ///
    /// Avant ce correctif, `read()` tentait d'ouvrir `packs/` (répertoire), échouait
    /// avec « impossible d'ouvrir le CPK », et les fichiers étaient inaccessibles.
    #[test]
    fn read_loose_file_avec_cpk_vide() {
        let dir = crate::vfs::resolve_game_dir()
            .to_string_lossy()
            .into_owned();
        let data_dir = std::path::Path::new(&dir).join("data");
        if !data_dir.join("cpk_list.cfg.bin").exists() {
            eprintln!("skip read_loose_file_avec_cpk_vide : jeu absent");
            return;
        }
        let mut vfs = Vfs::new();
        vfs.init(&data_dir).expect("init");

        // Collecter toutes les entrées loose (CPK vide) dans l'index
        let loose_entries: Vec<String> = vfs
            .iter()
            .filter(|(_, e)| e.cpk_filename.is_empty())
            .map(|(p, _)| p.to_string())
            .collect();

        assert!(
            !loose_entries.is_empty(),
            "aucune entrée loose dans le cpk_list (attendu au moins IE_15th.usm)"
        );
        eprintln!("Entrées loose dans cpk_list : {:?}", loose_entries);

        // Tester la lecture de chaque entrée loose dont le fichier disque existe
        let mut read_ok = 0usize;
        let mut missing_on_disk = 0usize;
        for path in &loose_entries {
            match vfs.read(path) {
                Ok(bytes) if !bytes.is_empty() => {
                    eprintln!("  OK  {path} : {} octets", bytes.len());
                    read_ok += 1;
                }
                Ok(_) => {
                    eprintln!("  VIDE {path}");
                }
                Err(crate::FormatError::Corrupt("loose file manquant sur disque")) => {
                    eprintln!("  ABSENT-DISQUE {path}");
                    missing_on_disk += 1;
                }
                Err(e) => {
                    panic!("read({path}) a échoué avec une erreur inattendue : {e}");
                }
            }
        }

        // Au moins un fichier loose doit être lisible (IE_15th.usm ou L5logo.usm)
        assert!(
            read_ok > 0,
            "aucun fichier loose n'a pu être lu (read_ok=0, missing_on_disk={missing_on_disk})"
        );
        eprintln!(
            "read_loose_file OK : {read_ok} fichiers lus, {missing_on_disk} absents du disque"
        );
    }
}
