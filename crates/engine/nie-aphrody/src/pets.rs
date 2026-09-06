//! Le contrat « pet » de Codex : manifeste `pet.json`, pistes d'animation, minutage, états.
//!
//! ## Provenance
//!
//! Ce module est une **réimplémentation dérivée** du sous-système `pets` de
//! [`openai/codex`](https://github.com/openai/codex) (`codex-rs/tui/src/pets/`), sous licence
//! **Apache-2.0** — voir le fichier `NOTICE` à la racine de la crate. Ce qui en est repris : le
//! schéma du manifeste, les valeurs par défaut, les bornes de validation, la sémantique
//! `loop_start` / `fallback`, l'algorithme de choix de frame par accumulation de durées, et le
//! modèle d'état à durée de vie. Les **assets** de Codex ne sont pas repris : ils ne sont pas
//! dans leur dépôt (ils viennent d'un CDN) et ne sont donc pas couverts par cette licence.
//!
//! ## Un écart assumé, et pourquoi
//!
//! Codex fige la géométrie de ses pets intégrés à 8 × 9 cellules (1536 × 1872) et **rejette**
//! tout atlas d'une autre taille. Aphrody v2 est un 8 × **11** (1536 × 2288) : suivre cette
//! constante à la lettre rejetterait notre propre pet. La géométrie est donc lue dans le
//! **manifeste** — ce que Codex fait déjà pour ses pets personnalisés — et sa validation stricte
//! est appliquée à la géométrie déclarée. La cellule (192 × 208) et le nombre de colonnes (8)
//! sont, eux, identiques de part et d'autre : Aphrody v2 est une extension du même contrat, pas
//! un format concurrent.
//!
//! ## Portabilité — ce module ne lit jamais l'horloge
//!
//! Aucun `Instant`, aucun `SystemTime` : le temps écoulé est **passé en argument**, en
//! millisecondes. C'est ce qui rend le module utilisable tel quel dans un navigateur
//! (`performance.now()`), sur mobile, dans un terminal ou dans un test déterministe — là où un
//! `Instant::now()` interne aurait imposé une horloge et rendu les tests dépendants du temps
//! réel.

use std::collections::BTreeMap;

use crate::Error;

/// Largeur d'une cellule, en pixels. Commune à Codex et à Aphrody v2.
pub const CELLULE_LARGEUR: u32 = 192;
/// Hauteur d'une cellule, en pixels. Commune à Codex et à Aphrody v2.
pub const CELLULE_HAUTEUR: u32 = 208;
/// Nombre de colonnes par défaut. Commun aux deux contrats.
pub const COLONNES_DEFAUT: u32 = 8;
/// Nombre de lignes par défaut, **côté Codex** (Aphrody v2 en a 11).
pub const LIGNES_DEFAUT: u32 = 9;
/// Plafond de frames d'un pet, repris de Codex.
pub const MAX_FRAMES: usize = 256;
/// Plafond de cadence, repris de Codex : une piste au-delà est refusée.
pub const FPS_MAX: f64 = 60.0;
/// Cadence par défaut quand le manifeste n'en donne pas.
pub const FPS_DEFAUT: f64 = 8.0;
/// Nom de l'animation de repli par défaut.
pub const REPLI_DEFAUT: &str = "idle";
/// Nom de fichier d'atlas par défaut.
pub const ATLAS_DEFAUT: &str = "spritesheet.webp";

/// La grille d'un atlas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Grille {
    /// Largeur d'une cellule.
    #[serde(default = "largeur_defaut")]
    pub width: u32,
    /// Hauteur d'une cellule.
    #[serde(default = "hauteur_defaut")]
    pub height: u32,
    /// Colonnes.
    #[serde(default = "colonnes_defaut")]
    pub columns: u32,
    /// Lignes.
    #[serde(default = "lignes_defaut")]
    pub rows: u32,
}

const fn largeur_defaut() -> u32 {
    CELLULE_LARGEUR
}
const fn hauteur_defaut() -> u32 {
    CELLULE_HAUTEUR
}
const fn colonnes_defaut() -> u32 {
    COLONNES_DEFAUT
}
const fn lignes_defaut() -> u32 {
    LIGNES_DEFAUT
}

impl Default for Grille {
    fn default() -> Self {
        Self {
            width: CELLULE_LARGEUR,
            height: CELLULE_HAUTEUR,
            columns: COLONNES_DEFAUT,
            rows: LIGNES_DEFAUT,
        }
    }
}

impl Grille {
    /// Dimensions de l'atlas qu'implique cette grille.
    #[must_use]
    pub const fn atlas(&self) -> (u32, u32) {
        (self.width * self.columns, self.height * self.rows)
    }

    /// Nombre de cellules.
    #[must_use]
    pub const fn cellules(&self) -> usize {
        (self.columns as usize) * (self.rows as usize)
    }

    /// Rectangle `(x, y, largeur, hauteur)` de la cellule d'indice `sprite_index`.
    ///
    /// L'indexation est **ligne par ligne** : `index = ligne * colonnes + colonne`, comme chez
    /// Codex. Rend `None` hors grille plutôt qu'un rectangle plausible qui rognerait le voisin.
    #[must_use]
    pub const fn cellule(&self, sprite_index: usize) -> Option<(u32, u32, u32, u32)> {
        if sprite_index >= self.cellules() {
            return None;
        }
        let colonnes = self.columns as usize;
        let ligne = (sprite_index / colonnes) as u32;
        let colonne = (sprite_index % colonnes) as u32;
        Some((
            colonne * self.width,
            ligne * self.height,
            self.width,
            self.height,
        ))
    }

    /// Vérifie que la grille est cohérente et tient sous les bornes de Codex.
    ///
    /// # Erreurs
    /// Si une dimension est nulle, ou si la grille dépasse [`MAX_FRAMES`].
    pub fn valider(&self) -> Result<(), Error> {
        if self.width == 0 || self.height == 0 || self.columns == 0 || self.rows == 0 {
            return Err(Error::Invalid(
                "grille : aucune dimension ne peut être nulle".into(),
            ));
        }
        if self.cellules() > MAX_FRAMES {
            return Err(Error::Invalid(format!(
                "grille de {} cellules : le plafond est {MAX_FRAMES}",
                self.cellules()
            )));
        }
        Ok(())
    }

    /// Vérifie que l'atlas fourni fait **exactement** la taille qu'implique la grille.
    ///
    /// Codex refuse un atlas d'une autre taille plutôt que de le recadrer : un atlas décalé d'un
    /// pixel produirait des frames qui mordent sur leurs voisines, défaut qui ne se voit qu'une
    /// fois l'animation en marche. Même choix ici.
    ///
    /// # Erreurs
    /// Si les dimensions ne correspondent pas.
    pub fn valider_atlas(&self, largeur: u32, hauteur: u32) -> Result<(), Error> {
        let (al, ah) = self.atlas();
        if largeur != al || hauteur != ah {
            return Err(Error::Invalid(format!(
                "atlas {largeur}x{hauteur} : la grille {}x{} en cellules {}x{} en exige {al}x{ah}",
                self.columns, self.rows, self.width, self.height
            )));
        }
        Ok(())
    }
}

/// Une frame d'une piste : quelle cellule, et combien de temps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct FrameTemps {
    /// Indice de la cellule dans la grille.
    pub sprite_index: usize,
    /// Durée d'affichage, en millisecondes.
    pub duree_ms: u64,
}

/// Une piste d'animation nommée.
///
/// `loop_start == None` veut dire **une seule passe** : la piste ne boucle pas, et la dernière
/// frame passe la main à `repli`. Ne pas supposer qu'une piste boucle parce qu'elle a plusieurs
/// frames — c'est l'avertissement explicite du contrat amont, et la source d'erreur la plus
/// courante en le lisant.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Piste {
    /// Les frames, dans l'ordre.
    pub frames: Vec<FrameTemps>,
    /// Indice de la frame où reboucler, ou `None` pour une piste à passe unique.
    pub loop_start: Option<usize>,
    /// Animation qui prend le relais quand une piste à passe unique se termine.
    pub repli: String,
}

impl Piste {
    /// Durée totale de la piste, en millisecondes.
    #[must_use]
    pub fn duree_totale_ms(&self) -> u64 {
        self.frames.iter().map(|f| f.duree_ms).sum()
    }
}

/// Ce qu'une frame courante apprend à l'appelant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tick {
    /// Cellule à afficher.
    pub sprite_index: usize,
    /// Millisecondes avant le prochain changement, ou `None` quand l'image est figée.
    ///
    /// C'est ce délai qu'il faut planifier — **pas** une cadence fixe. Une piste dont les
    /// frames durent 1680, 660 puis 1920 ms redessinée à 8 fps gaspillerait 20 réveils sur 21.
    pub delai_ms: Option<u64>,
}

/// Frame à afficher après `ecoule_ms` passées dans la piste.
///
/// Reprend la sémantique de Codex :
/// - une piste d'une seule frame est figée (aucun réveil planifié) ;
/// - avec `loop_start`, le temps au-delà de la durée totale reboucle **sur le seul segment de
///   boucle**, le préfixe n'étant joué qu'une fois — c'est ce qui permet à une animation
///   d'avoir une amorce puis un cycle ;
/// - sans `loop_start`, une fois la durée écoulée la dernière frame reste affichée, à charge de
///   l'appelant de basculer sur [`Piste::repli`].
#[must_use]
pub fn frame_courante(piste: &Piste, ecoule_ms: u64) -> Option<Tick> {
    if piste.frames.len() <= 1 {
        return piste.frames.first().map(|f| Tick {
            sprite_index: f.sprite_index,
            delai_ms: None,
        });
    }
    let total = piste.duree_totale_ms();
    let effectif = match piste.loop_start.filter(|i| *i < piste.frames.len()) {
        Some(depart) => {
            let prefixe: u64 = piste.frames[..depart].iter().map(|f| f.duree_ms).sum();
            let boucle: u64 = piste.frames[depart..].iter().map(|f| f.duree_ms).sum();
            if ecoule_ms >= total && boucle > 0 {
                prefixe + (ecoule_ms.saturating_sub(prefixe)) % boucle
            } else {
                ecoule_ms
            }
        }
        None => {
            if ecoule_ms >= total {
                return piste.frames.last().map(|f| Tick {
                    sprite_index: f.sprite_index,
                    delai_ms: None,
                });
            }
            ecoule_ms
        }
    };
    frame_a(piste, effectif)
}

fn frame_a(piste: &Piste, ecoule_ms: u64) -> Option<Tick> {
    let mut curseur = 0u64;
    for f in &piste.frames {
        let fin = curseur + f.duree_ms;
        if ecoule_ms < fin {
            return Some(Tick {
                sprite_index: f.sprite_index,
                delai_ms: Some(fin - ecoule_ms),
            });
        }
        curseur = fin;
    }
    piste.frames.last().map(|f| Tick {
        sprite_index: f.sprite_index,
        delai_ms: None,
    })
}

// ---------------------------------------------------------------------------------------------
// Le manifeste `pet.json`
// ---------------------------------------------------------------------------------------------

/// Une piste telle qu'elle est écrite dans le manifeste.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PisteManifeste {
    /// Indices de cellules, dans l'ordre.
    pub frames: Vec<usize>,
    /// Cadence, en images par seconde.
    #[serde(default)]
    pub fps: Option<f64>,
    /// La piste boucle-t-elle ? Défaut : oui.
    #[serde(rename = "loop", default)]
    pub boucle: Option<bool>,
    /// Animation de repli pour une piste à passe unique.
    #[serde(default)]
    pub fallback: Option<String>,
}

/// Le manifeste `pet.json` — clés en `camelCase`, comme chez Codex.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifeste {
    /// Identifiant du pet.
    pub id: String,
    /// Nom affichable.
    #[serde(default)]
    pub display_name: Option<String>,
    /// Description libre.
    #[serde(default)]
    pub description: Option<String>,
    /// Chemin de l'atlas, **relatif au dossier du pet**.
    #[serde(default)]
    pub spritesheet_path: Option<String>,
    /// Géométrie de la grille.
    #[serde(default)]
    pub frame: Grille,
    /// Pistes, par nom.
    #[serde(default)]
    pub animations: BTreeMap<String, PisteManifeste>,
}

/// Un pet chargé et validé : géométrie, pistes minutées, identité.
#[derive(Debug, Clone)]
pub struct PetCodex {
    /// Identifiant.
    pub id: String,
    /// Nom affichable (défaut : l'identifiant).
    pub nom: String,
    /// Description.
    pub description: String,
    /// Chemin relatif de l'atlas.
    pub atlas: String,
    /// Géométrie.
    pub grille: Grille,
    /// Pistes minutées, par nom.
    pub pistes: BTreeMap<String, Piste>,
}

/// Un chemin d'atlas doit rester **relatif et interne** au dossier du pet.
///
/// Un manifeste est une donnée, éventuellement téléchargée : accepter `../..` ou `/etc/passwd`
/// y ferait lire un fichier arbitraire. Refuser ici est la seule barrière — l'appelant qui
/// concatène ensuite n'a plus aucun moyen de savoir d'où le chemin vient.
fn valider_chemin_atlas(chemin: &str) -> Result<(), Error> {
    if chemin.is_empty() {
        return Err(Error::Invalid("chemin d'atlas vide".into()));
    }
    if chemin.starts_with('/') || chemin.starts_with('\\') || chemin.contains(':') {
        return Err(Error::Invalid(format!(
            "chemin d'atlas non relatif : « {chemin} »"
        )));
    }
    if chemin.split(['/', '\\']).any(|s| s == "..") {
        return Err(Error::Invalid(format!(
            "chemin d'atlas sortant du dossier : « {chemin} »"
        )));
    }
    Ok(())
}

impl Manifeste {
    /// Parse un `pet.json`.
    ///
    /// # Erreurs
    /// Si le JSON est invalide ou si le manifeste ne passe pas [`Manifeste::valider`].
    pub fn depuis_json(json: &str) -> Result<PetCodex, Error> {
        let m: Self = serde_json::from_str(json)?;
        m.valider()
    }

    /// Valide le manifeste et rend le pet minuté.
    ///
    /// Contrôles repris de Codex : grille cohérente et sous le plafond de frames, chemin d'atlas
    /// relatif interne, cadence dans `]0, 60]`, indices de cellule dans la grille, et pistes non
    /// vides.
    ///
    /// # Erreurs
    /// Au premier contrôle qui échoue, avec la raison — jamais un repli silencieux.
    pub fn valider(self) -> Result<PetCodex, Error> {
        self.frame.valider()?;
        let atlas = self
            .spritesheet_path
            .unwrap_or_else(|| ATLAS_DEFAUT.to_string());
        valider_chemin_atlas(&atlas)?;

        let mut pistes = BTreeMap::new();
        for (nom, p) in self.animations {
            if p.frames.is_empty() {
                return Err(Error::Invalid(format!(
                    "animation « {nom} » sans aucune frame"
                )));
            }
            if p.frames.len() > MAX_FRAMES {
                return Err(Error::Invalid(format!(
                    "animation « {nom} » : {} frames, le plafond est {MAX_FRAMES}",
                    p.frames.len()
                )));
            }
            let fps = p.fps.unwrap_or(FPS_DEFAUT);
            if !(fps > 0.0 && fps <= FPS_MAX) {
                return Err(Error::Invalid(format!(
                    "animation « {nom} » : fps {fps} hors de ]0, {FPS_MAX}]"
                )));
            }
            if let Some(hors) = p.frames.iter().find(|i| **i >= self.frame.cellules()) {
                return Err(Error::Invalid(format!(
                    "animation « {nom} » : cellule {hors} hors d'une grille de {} cellules",
                    self.frame.cellules()
                )));
            }
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "fps borné à ]0,60]"
            )]
            let duree_ms = (1000.0 / fps).round().max(1.0) as u64;
            let boucle = p.boucle.unwrap_or(true);
            pistes.insert(
                nom,
                Piste {
                    frames: p
                        .frames
                        .into_iter()
                        .map(|sprite_index| FrameTemps {
                            sprite_index,
                            duree_ms,
                        })
                        .collect(),
                    loop_start: boucle.then_some(0),
                    repli: p.fallback.unwrap_or_else(|| REPLI_DEFAUT.to_string()),
                },
            );
        }

        Ok(PetCodex {
            nom: self.display_name.unwrap_or_else(|| self.id.clone()),
            description: self.description.unwrap_or_default(),
            id: self.id,
            atlas,
            grille: self.frame,
            pistes,
        })
    }
}

impl PetCodex {
    /// La piste nommée.
    #[must_use]
    pub fn piste(&self, nom: &str) -> Option<&Piste> {
        self.pistes.get(nom)
    }

    /// La piste à jouer après `ecoule_ms` dans `nom`, en suivant les replis.
    ///
    /// Une piste à passe unique arrivée à son terme passe la main à son repli ; la chaîne est
    /// suivie au plus **quatre** fois pour qu'un manifeste où deux pistes se replient l'une sur
    /// l'autre ne fasse pas tourner l'appelant en rond.
    #[must_use]
    pub fn resoudre(&self, nom: &str, ecoule_ms: u64) -> Option<(&str, Tick)> {
        // Le nom rendu est emprunté à la CLÉ de la table, jamais à l'argument : sans cela, la
        // valeur de retour aurait la durée de vie de `nom` au premier tour et celle de `self`
        // aux suivants, ce que le compilateur refuse à juste titre.
        let (mut courant, mut piste) = self.pistes.get_key_value(nom)?;
        for _ in 0..4 {
            let finie = piste.loop_start.is_none() && ecoule_ms >= piste.duree_totale_ms();
            if !finie || piste.repli == *courant {
                break;
            }
            match self.pistes.get_key_value(piste.repli.as_str()) {
                Some((n, p)) => (courant, piste) = (n, p),
                None => break,
            }
        }
        frame_courante(piste, ecoule_ms).map(|t| (courant.as_str(), t))
    }
}

// ---------------------------------------------------------------------------------------------
// L'état ambiant
// ---------------------------------------------------------------------------------------------

/// Ce que le pet raconte de la session, et pour combien de temps.
///
/// Les durées de vie viennent de Codex : passé ce délai, l'état ne veut plus rien dire et le pet
/// retombe au repos plutôt que d'afficher indéfiniment un « en cours » vieux d'une journée.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Etat {
    /// Une tâche tourne.
    EnCours,
    /// Le pet attend une saisie.
    Attente,
    /// Un résultat est prêt à relire.
    Relecture,
    /// Quelque chose a échoué.
    Echec,
}

impl Etat {
    /// Nom de l'animation associée.
    #[must_use]
    pub const fn animation(self) -> &'static str {
        match self {
            Self::EnCours => "running",
            Self::Attente => "waiting",
            Self::Relecture => "review",
            Self::Echec => "failed",
        }
    }

    /// Durée de vie de l'état, en millisecondes.
    #[must_use]
    pub const fn duree_vie_ms(self) -> u64 {
        match self {
            Self::EnCours => 3 * 60 * 1000,
            Self::Echec => 60 * 60 * 1000,
            Self::Attente => 24 * 60 * 60 * 1000,
            Self::Relecture => 7 * 24 * 60 * 60 * 1000,
        }
    }
}

/// L'état ambiant du pet : un état daté, ou rien.
#[derive(Debug, Clone, Copy, Default)]
pub struct Ambiance {
    etat: Option<(Etat, u64)>,
}

impl Ambiance {
    /// Pose un état, daté de `maintenant_ms`.
    pub const fn poser(&mut self, etat: Etat, maintenant_ms: u64) {
        self.etat = Some((etat, maintenant_ms));
    }

    /// Retire l'état courant.
    pub const fn effacer(&mut self) {
        self.etat = None;
    }

    /// L'état encore valide à `maintenant_ms`, et depuis combien de temps il dure.
    #[must_use]
    pub fn etat(&self, maintenant_ms: u64) -> Option<(Etat, u64)> {
        let (etat, pose) = self.etat?;
        let age = maintenant_ms.saturating_sub(pose);
        (age < etat.duree_vie_ms()).then_some((etat, age))
    }

    /// Nom de l'animation à jouer et temps écoulé dedans.
    ///
    /// Sans état valide, c'est [`REPLI_DEFAUT`] joué depuis `maintenant_ms` — le repos est
    /// l'état par défaut, pas une absence d'animation.
    #[must_use]
    pub fn animation(&self, maintenant_ms: u64) -> (&'static str, u64) {
        self.etat(maintenant_ms)
            .map_or((REPLI_DEFAUT, maintenant_ms), |(e, age)| {
                (e.animation(), age)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn piste(frames: &[usize], duree_ms: u64, loop_start: Option<usize>) -> Piste {
        Piste {
            frames: frames
                .iter()
                .map(|i| FrameTemps {
                    sprite_index: *i,
                    duree_ms,
                })
                .collect(),
            loop_start,
            repli: "idle".into(),
        }
    }

    #[test]
    fn la_grille_par_defaut_est_celle_de_codex() {
        let g = Grille::default();
        assert_eq!(g.atlas(), (1536, 1872), "8x9 cellules de 192x208");
        assert_eq!(g.cellules(), 72);
        // La cellule et le nombre de colonnes sont ceux d'Aphrody v2 : seul le nombre de lignes
        // diffère (11 chez nous), et c'est pourquoi la géométrie se lit dans le manifeste.
        assert_eq!((g.width, g.height, g.columns), (192, 208, 8));
    }

    #[test]
    fn la_grille_aphrody_v2_est_acceptee_la_ou_codex_la_rejetterait() {
        let g = Grille {
            rows: 11,
            ..Grille::default()
        };
        g.valider()
            .expect("11 lignes reste sous le plafond de 256 cellules");
        assert_eq!(g.atlas(), (1536, 2288));
        assert_eq!(g.cellules(), 88);
        g.valider_atlas(1536, 2288).expect("atlas conforme");
        assert!(
            g.valider_atlas(1536, 1872).is_err(),
            "un atlas 8x9 ne remplit pas 8x11"
        );
    }

    #[test]
    fn l_indexation_des_cellules_est_ligne_par_ligne() {
        let g = Grille::default();
        assert_eq!(g.cellule(0), Some((0, 0, 192, 208)));
        assert_eq!(
            g.cellule(8),
            Some((0, 208, 192, 208)),
            "index 8 = début de la ligne 1"
        );
        assert_eq!(g.cellule(9), Some((192, 208, 192, 208)));
        assert_eq!(
            g.cellule(72),
            None,
            "hors grille rend None, pas un rectangle plausible"
        );
    }

    #[test]
    fn une_piste_figee_ne_planifie_aucun_reveil() {
        let t = frame_courante(&piste(&[3], 100, Some(0)), 10_000).expect("tick");
        assert_eq!(
            t,
            Tick {
                sprite_index: 3,
                delai_ms: None
            }
        );
    }

    #[test]
    fn le_delai_rendu_est_celui_de_la_frame_pas_une_cadence() {
        let p = Piste {
            frames: vec![
                FrameTemps {
                    sprite_index: 0,
                    duree_ms: 1680,
                },
                FrameTemps {
                    sprite_index: 1,
                    duree_ms: 660,
                },
            ],
            loop_start: Some(0),
            repli: "idle".into(),
        };
        assert_eq!(frame_courante(&p, 0).expect("tick").delai_ms, Some(1680));
        assert_eq!(frame_courante(&p, 1679).expect("tick").delai_ms, Some(1));
        let t = frame_courante(&p, 1680).expect("tick");
        assert_eq!((t.sprite_index, t.delai_ms), (1, Some(660)));
    }

    #[test]
    fn la_boucle_ne_rejoue_pas_le_prefixe() {
        // Amorce = frame 0, cycle = frames 1 et 2. Après un tour complet, on doit retomber dans
        // le cycle, jamais sur l'amorce.
        let p = piste(&[0, 1, 2], 100, Some(1));
        assert_eq!(frame_courante(&p, 0).expect("t").sprite_index, 0);
        // 300 ms = durée totale → rebouclage sur le segment [1..], pas sur 0.
        assert_eq!(frame_courante(&p, 300).expect("t").sprite_index, 1);
        assert_eq!(frame_courante(&p, 400).expect("t").sprite_index, 2);
        assert_eq!(frame_courante(&p, 500).expect("t").sprite_index, 1);
    }

    #[test]
    fn une_passe_unique_se_fige_sur_sa_derniere_frame() {
        let p = piste(&[4, 5, 6], 100, None);
        let t = frame_courante(&p, 10_000).expect("t");
        assert_eq!((t.sprite_index, t.delai_ms), (6, None));
    }

    #[test]
    fn une_passe_unique_finie_passe_la_main_a_son_repli() {
        let json = r#"{
            "id": "aphrody",
            "frame": { "rows": 11 },
            "animations": {
                "idle":   { "frames": [0, 1], "fps": 10 },
                "waving": { "frames": [24, 25], "fps": 10, "loop": false, "fallback": "idle" }
            }
        }"#;
        let pet = Manifeste::depuis_json(json).expect("manifeste");
        // Pendant la piste : c'est bien `waving`.
        let (nom, _) = pet.resoudre("waving", 0).expect("tick");
        assert_eq!(nom, "waving");
        // Une fois finie : le repli prend la main, sans que l'appelant ait à le savoir.
        let (nom, t) = pet.resoudre("waving", 5_000).expect("tick");
        assert_eq!(nom, "idle");
        assert!(
            t.delai_ms.is_some(),
            "idle boucle, donc un réveil est planifié"
        );
    }

    #[test]
    fn le_manifeste_refuse_ce_que_codex_refuse() {
        let cas = [
            (
                r#"{"id":"a","animations":{"x":{"frames":[]}}}"#,
                "sans aucune frame",
            ),
            (
                r#"{"id":"a","animations":{"x":{"frames":[0],"fps":0}}}"#,
                "hors de",
            ),
            (
                r#"{"id":"a","animations":{"x":{"frames":[0],"fps":61}}}"#,
                "hors de",
            ),
            (
                r#"{"id":"a","animations":{"x":{"frames":[999]}}}"#,
                "hors d'une grille",
            ),
            (
                r#"{"id":"a","spritesheetPath":"../evade.webp"}"#,
                "sortant du dossier",
            ),
            (
                r#"{"id":"a","spritesheetPath":"/etc/passwd"}"#,
                "non relatif",
            ),
            (r#"{"id":"a","frame":{"rows":0}}"#, "nulle"),
            (r#"{"id":"a","frame":{"rows":99}}"#, "plafond"),
        ];
        for (json, attendu) in cas {
            let err = Manifeste::depuis_json(json).expect_err(json);
            let msg = format!("{err}");
            assert!(
                msg.contains(attendu),
                "attendu « {attendu} », obtenu « {msg} » pour {json}"
            );
        }
    }

    #[test]
    fn les_defauts_du_manifeste_sont_ceux_de_codex() {
        let pet = Manifeste::depuis_json(r#"{"id":"a","animations":{"idle":{"frames":[0,1]}}}"#)
            .expect("manifeste");
        assert_eq!(pet.atlas, "spritesheet.webp");
        assert_eq!(pet.nom, "a", "sans displayName, le nom est l'identifiant");
        let idle = pet.piste("idle").expect("idle");
        assert_eq!(idle.frames[0].duree_ms, 125, "8 fps par défaut → 125 ms");
        assert_eq!(idle.loop_start, Some(0), "une piste boucle par défaut");
        assert_eq!(idle.repli, "idle");
    }

    #[test]
    fn un_etat_expire_et_le_pet_retombe_au_repos() {
        let mut a = Ambiance::default();
        a.poser(Etat::EnCours, 1_000);
        assert_eq!(a.animation(1_500), ("running", 500));
        // 3 minutes plus tard, l'état ne veut plus rien dire.
        assert_eq!(a.etat(1_000 + Etat::EnCours.duree_vie_ms()), None);
        assert_eq!(a.animation(1_000 + Etat::EnCours.duree_vie_ms()).0, "idle");
    }

    #[test]
    fn les_durees_de_vie_sont_celles_du_contrat_amont() {
        assert_eq!(Etat::EnCours.duree_vie_ms(), 3 * 60 * 1000);
        assert_eq!(Etat::Echec.duree_vie_ms(), 60 * 60 * 1000);
        assert_eq!(Etat::Attente.duree_vie_ms(), 24 * 60 * 60 * 1000);
        assert_eq!(Etat::Relecture.duree_vie_ms(), 7 * 24 * 60 * 60 * 1000);
    }

    #[test]
    fn un_repli_circulaire_ne_fait_pas_tourner_en_rond() {
        let json = r#"{
            "id": "a",
            "animations": {
                "x": { "frames": [0], "loop": false, "fallback": "y" },
                "y": { "frames": [1], "loop": false, "fallback": "x" }
            }
        }"#;
        let pet = Manifeste::depuis_json(json).expect("manifeste");
        // Une piste d'une frame est figée : elle rend un tick, jamais une boucle infinie.
        assert!(pet.resoudre("x", 10_000).is_some());
    }
}
