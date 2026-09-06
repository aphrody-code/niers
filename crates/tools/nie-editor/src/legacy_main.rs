//! Ancien éditeur Fyrox/OpenGL, conservé via la feature `legacy-fyrox`.
//!
//! Plutôt que de réécrire un éditeur, celui-ci **embarque l'éditeur Fyrox**
//! (`fyroxed_base::Editor`, publié en bibliothèque) et lui apporte ce que Fyrox ne peut pas
//! savoir : comment lire les assets d'*Inazuma Eleven: Victory Road*. On récupère ainsi d'un coup
//! le graphe de scène, l'inspecteur réflexif, les gizmos de déplacement/rotation/échelle,
//! l'undo/redo, le world viewer et le navigateur d'assets — l'essentiel d'un éditeur type Unreal,
//! déjà éprouvé sur de vrais projets.
//!
//! **Rendu GPU natif.** Fyrox rend en OpenGL 3.3 via `fyrox-graphics-gl` : c'est une fenêtre
//! native, pas un canevas de navigateur. Aucun WebGL n'intervient.
//!
//! ## Le pont assets
//!
//! Fyrox n'importe que FBX et son format natif RGS — pas de glTF. Les modèles du jeu, eux,
//! sortent de `nie_formats::assemble` sous forme de GLB. Le pont est donc fait **par code** :
//! `Model` (géométrie + atlas décodés) → `Mesh` Fyrox (surfaces + textures) → scène `.rgs` que
//! l'éditeur ouvre au démarrage. Aucun format intermédiaire sur disque, aucune conversion
//! manuelle demandée à l'utilisatrice.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Parser;

use fyrox::asset::untyped::ResourceKind;
use fyrox::core::algebra::Vector3;
use fyrox::core::log::Log;

use fyrox::core::math::TriangleDefinition;
use fyrox::event_loop::EventLoop;
use fyrox::material::{Material, MaterialResource};
use fyrox::resource::texture::{TextureResource, TextureResourceExtension};
use fyrox::scene::base::BaseBuilder;
use fyrox::scene::mesh::MeshBuilder;
use fyrox::scene::mesh::buffer::{TriangleBuffer, VertexBuffer};
use fyrox::scene::mesh::surface::{Surface, SurfaceData, SurfaceResource};
use fyrox::scene::mesh::vertex::StaticVertex;

use fyrox::scene::Scene;
use fyrox::scene::transform::TransformBuilder;
use fyroxed_base::{Editor, StartupData};

use nie_formats::vfs::Vfs;
use nie_render3d::glb::Model;

#[derive(Parser)]
#[command(
    name = "nie-editor",
    about = "Éditeur de scène 3D niers — éditeur Fyrox alimenté par les vrais assets IEVR"
)]
struct Cli {
    /// Racine du jeu (contient `data/cpk_list.cfg.bin`).
    #[arg(long)]
    game_dir: PathBuf,
    /// Chemin VFS du modèle à ouvrir (ex. `data/common/chr/.../c01000010.g4md`).
    /// Absent = éditeur ouvert sur une scène vide.
    #[arg(long)]
    asset: Option<String>,
    /// Répertoire de travail de l'éditeur (où la scène convertie est écrite).
    /// Défaut : un sous-dossier `nie-editor` du répertoire temporaire.
    #[arg(long)]
    workspace: Option<PathBuf>,
}

fn main() -> Result<()> {
    Log::set_file_name("nie-editor.log");
    let cli = Cli::parse();

    let workspace = cli
        .workspace
        .unwrap_or_else(|| std::env::temp_dir().join("nie-editor"));
    std::fs::create_dir_all(&workspace)
        .with_context(|| format!("création de l'espace de travail {}", workspace.display()))?;

    // La scène n'est construite que si un asset est demandé : l'éditeur reste utilisable seul,
    // pour ouvrir une scène déjà enregistrée.
    let startup_data = match cli.asset.as_deref() {
        Some(asset) => {
            let scene_path = build_scene_file(&cli.game_dir, asset, &workspace)?;
            Some(StartupData {
                working_directory: workspace.clone(),
                scenes: vec![scene_path],
                named_objects: false,
            })
        }
        None => Some(StartupData {
            working_directory: workspace.clone(),
            scenes: vec![],
            named_objects: false,
        }),
    };

    Editor::new(startup_data).run(EventLoop::new().context("création de la boucle d'événements")?);
    Ok(())
}

/// Assemble le modèle du jeu, le convertit en scène Fyrox et l'écrit en `.rgs`.
/// Renvoie le chemin de la scène produite.
fn build_scene_file(game_dir: &Path, asset: &str, workspace: &Path) -> Result<PathBuf> {
    let model = load_game_model(game_dir, asset)?;
    let mut scene = scene_from_model(&model, asset);

    let stem = asset.rsplit('/').next().unwrap_or(asset).replace('.', "_");
    let scene_path = workspace.join(format!("{stem}.rgs"));

    let mut visitor = fyrox::core::visitor::Visitor::new();
    scene
        .save("Scene", &mut visitor)
        .map_err(|e| anyhow::anyhow!("sérialisation de la scène : {e}"))?;
    visitor
        .save_binary_to_file(&scene_path)
        .map_err(|e| anyhow::anyhow!("écriture de {} : {e}", scene_path.display()))?;

    Ok(scene_path)
}

/// Monte le VFS du jeu et assemble le modèle (G4MD + son G4MG frère + son atlas G4TX).
///
/// Même résolution de frères que l'aperçu de `nie-explorer` : un `.g4md` seul ne porte pas la
/// géométrie, elle vit dans le `.g4mg` de même nom dans le même dossier.
fn load_game_model(game_dir: &Path, asset: &str) -> Result<Model> {
    use nie_formats::assemble::{
        EmbeddedTexture, GenericModelInput, MeshComponent, assemble_generic_model,
    };

    let mut vfs = Vfs::new();
    vfs.init(game_dir)
        .map_err(|e| anyhow::anyhow!("montage du VFS {} : {e}", game_dir.display()))?;

    let data = vfs
        .read(asset)
        .map_err(|e| anyhow::anyhow!("lecture de {asset} : {e}"))?;

    let base = asset.rsplit('/').next().unwrap_or(asset);
    let stem = base.rsplit_once('.').map(|(s, _)| s).unwrap_or(base);
    let dir_prefix = asset.strip_suffix(base).unwrap_or("");
    let sibling = |ext: &str| -> Option<Vec<u8>> {
        let candidate = format!("{dir_prefix}{stem}.{ext}");
        if candidate == asset {
            Some(data.clone())
        } else {
            vfs.read(&candidate).ok()
        }
    };

    let g4md = sibling("g4md").context("G4MD introuvable (fichier ou frère de même nom)")?;
    let g4mg = sibling("g4mg").context("G4MG introuvable (frère requis pour la géométrie)")?;

    let mut assembled = assemble_generic_model(GenericModelInput {
        code: stem.to_string(),
        g4md,
        g4mg,
        component: MeshComponent::Generic,
    })
    .map_err(|e| anyhow::anyhow!("assemblage du modèle : {e}"))?;

    if let Some(png) =
        sibling("g4tx").and_then(|g4tx| nie_formats::g4tx_decode::decode_best_to_png(&g4tx, stem))
    {
        assembled.embedded_textures.push(EmbeddedTexture {
            component: MeshComponent::Generic,
            name: format!("{stem}_tex"),
            png_bytes: png,
        });
    }

    let glb = assembled.to_glb_embedded();
    nie_render3d::glb::parse(&glb).map_err(|e| anyhow::anyhow!("relecture du GLB assemblé : {e}"))
}

/// Convertit un [`Model`] (géométrie + atlas déjà décodés en RGBA8) en scène Fyrox.
///
/// Une primitive du modèle = une surface Fyrox, avec son propre matériau : les atlas diffèrent
/// d'une primitive à l'autre (corps, yeux, bouche sont des textures distinctes), les fusionner
/// donnerait un personnage au visage plaqué sur le torse.
fn scene_from_model(model: &Model, label: &str) -> Scene {
    let mut scene = Scene::new();

    for (i, prim) in model.primitives.iter().enumerate() {
        if prim.indices.len() < 3 || prim.positions.is_empty() {
            continue;
        }

        let vertices: Vec<StaticVertex> = (0..prim.positions.len())
            .map(|v| {
                let p = prim.positions[v];
                let uv = prim.uv.get(v).copied().unwrap_or([0.0, 0.0]);
                // Normale absente → vers le haut : une normale nulle rendrait l'éclairage
                // indéfini et la surface uniformément noire.
                let n = prim.normals.get(v).copied().unwrap_or([0.0, 1.0, 0.0]);
                StaticVertex::from_pos_uv_normal(
                    Vector3::new(p[0], p[1], p[2]),
                    fyrox::core::algebra::Vector2::new(uv[0], uv[1]),
                    Vector3::new(n[0], n[1], n[2]),
                )
            })
            .collect();

        let triangles: Vec<TriangleDefinition> = prim
            .indices
            .chunks_exact(3)
            .map(|t| TriangleDefinition([t[0], t[1], t[2]]))
            .collect();

        let Ok(vertex_buffer) = VertexBuffer::new(vertices.len(), vertices) else {
            continue;
        };
        let data = SurfaceData::new(vertex_buffer, TriangleBuffer::new(triangles));

        let mut material = Material::standard();
        if let Some(texture) = prim.texture.and_then(|t| model.textures.get(t))
            && let Some(res) = texture_resource(texture)
        {
            material.bind("diffuseTexture", res);
        }

        let mut surface = Surface::new(SurfaceResource::new_ok(
            Default::default(),
            ResourceKind::Embedded,
            data,
        ));
        surface.set_material(MaterialResource::new_ok(
            Default::default(),
            ResourceKind::Embedded,
            material,
        ));

        MeshBuilder::new(
            BaseBuilder::new()
                .with_name(format!("{label}#{i}"))
                .with_local_transform(TransformBuilder::new().build()),
        )
        .with_surfaces(vec![surface])
        .build(&mut scene.graph);
    }

    add_default_lighting(&mut scene);
    scene
}

/// Crée une texture Fyrox depuis les octets RGBA8 déjà décodés par `g4tx_decode`.
fn texture_resource(texture: &nie_render3d::glb::Texture) -> Option<TextureResource> {
    TextureResource::from_bytes(
        // Identifiant de ressource neuf : ces atlas sont reconstruits à la volée depuis les CPK,
        // ils n'ont pas d'UUID stable dans un projet Fyrox.
        fyrox::core::Uuid::new_v4(),
        fyrox::resource::texture::TextureKind::Rectangle {
            width: texture.width,
            height: texture.height,
        },
        fyrox::resource::texture::TexturePixelKind::RGBA8,
        texture.rgba.clone(),
        ResourceKind::Embedded,
    )
}

/// Lumière directionnelle + caméra cadrées sur l'origine.
///
/// Sans ça, la scène s'ouvre noire : les modèles du jeu n'embarquent aucune lumière, et une scène
/// Fyrox neuve n'en a pas non plus.
fn add_default_lighting(scene: &mut Scene) {
    use fyrox::core::algebra::UnitQuaternion;
    use fyrox::scene::light::{BaseLightBuilder, directional::DirectionalLightBuilder};

    // Orientation par angles d'Euler plutôt que `look_at` : une matrice de vue est l'INVERSE de la
    // transformation d'un noeud, la passer telle quelle éclairerait à l'opposé.
    let rotation = UnitQuaternion::from_euler_angles(-0.9, 0.7, 0.0);

    DirectionalLightBuilder::new(BaseLightBuilder::new(
        BaseBuilder::new().with_name("Soleil").with_local_transform(
            TransformBuilder::new()
                .with_local_position(Vector3::new(0.0, 5.0, 0.0))
                .with_local_rotation(rotation)
                .build(),
        ),
    ))
    .build(&mut scene.graph);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Vérifie la conversion `Model` → scène Fyrox sur un modèle synthétique : une primitive
    /// texturée doit produire un noeud de maillage, plus la lumière par défaut.
    #[test]
    fn convertit_un_modele_en_scene() {
        let model = Model {
            primitives: vec![nie_render3d::glb::Primitive {
                positions: vec![[-1.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
                normals: vec![[0.0, 0.0, 1.0]; 3],
                uv: vec![[0.0, 0.0], [1.0, 0.0], [0.5, 1.0]],
                indices: vec![0, 1, 2],
                texture: Some(0),
            }],
            textures: vec![nie_render3d::glb::Texture {
                width: 1,
                height: 1,
                rgba: vec![255, 0, 0, 255],
            }],
        };

        use fyrox::graph::SceneGraph;
        let scene = scene_from_model(&model, "test");
        let meshes = scene.graph.pair_iter().filter(|(_, n)| n.is_mesh()).count();
        assert_eq!(
            meshes, 1,
            "une primitive doit donner exactement un maillage"
        );
    }
}
