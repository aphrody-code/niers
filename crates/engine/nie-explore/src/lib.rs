//! Moteur partagé d'aperçu des entrées du VFS (CPK) : détecte le format d'un fichier lu depuis
//! [`nie_formats::vfs::Vfs`] et produit un résumé structuré lisible (une ligne par entrée),
//! ou `None` si aucun décodeur ne reconnaît le contenu (l'appelant retombe sur un hexdump brut).
//!
//! Utilisé par `niers vfs cat`/`stat` (crate `nie-cli`) **et** par le backend Tauri de l'app
//! desktop `nie-explorer` (`apps/inacord/src-tauri`) — un seul décodeur-dispatch, deux
//! façades (CLI texte / IPC JSON), pour éviter que les deux dérivent (cf. `CLAUDE.md`,
//! anti-doublon).

#![forbid(unsafe_code)]

pub mod audio;
pub mod bande_son;
pub mod bridge;
pub mod cinema;
pub mod depot;
pub mod export;
pub mod folder_roles;
pub mod listing;

use nie_formats::{
    cfgbin, col, cri_audio, dxbc, g4cm, g4la, g4ma, g4md, g4mt, g4pk, g4sk, g4tx, g4vs, level5,
    navm, nxtch, objbin,
};

/// Essaie les parseurs spécialisés `nie-data` dont la source est un T2B « entries »
/// (`chara_base`, `chara_text`) sur le JSON ponté depuis `t2b`. Chaque parseur est tolérant
/// (liste vide si les noeuds attendus sont absents) : on n'affiche que ce qui matche vraiment,
/// jamais une entrée fabriquée. Les ~30 autres modules T2B de `nie-data` suivent le même
/// principe — cf. `bridge::t2b_to_json` pour les brancher.
fn append_nie_data_t2b(out: &mut Vec<String>, t2b: &cfgbin::CfgBinFile) {
    let json = bridge::t2b_to_json(t2b);

    let charas = nie_data::chara_base::parse_all_chara_base(&json);
    if !charas.is_empty() {
        out.push(format!(
            "nie-data     chara_base : {} personnage(s)",
            charas.len()
        ));
        for c in charas.iter().take(8) {
            out.push(format!(
                "  {:<12} id={} nom_hash={}",
                c.internal_code, c.chara_id, c.name_hash
            ));
        }
        if charas.len() > 8 {
            out.push(format!("  … {} de plus", charas.len() - 8));
        }
    }

    let nouns = nie_data::chara_text::parse_all_nouns(&json);
    if !nouns.is_empty() {
        out.push(format!(
            "nie-data     chara_text (NOUN_INFO) : {} nom(s) localisé(s)",
            nouns.len()
        ));
        for n in nouns.iter().take(8) {
            out.push(format!("  {} — {}", n.hash_id, n.name));
        }
        if nouns.len() > 8 {
            out.push(format!("  … {} de plus", nouns.len() - 8));
        }
    }

    // `skill_text.cfg.bin` est un T2B distinct de `skill_config.cfg.bin` (RDBN) — sa forme
    // racine (`NOUN_INFO_BEGIN`/`TEXT_INFO_BEGIN`) est celle de CETTE branche, pas de
    // `append_nie_data_rdbn`. C'est la MÊME forme générique que `chara_text` (table de texte
    // NOUN_INFO/TEXT_INFO) : n'essayer que si `chara_text` n'a rien trouvé, pour éviter de
    // rapporter deux fois le même contenu sous deux étiquettes différentes sur un même fichier.
    if nouns.is_empty() {
        let skill_texts = nie_data::skill::parse_skill_text(&json);
        if !skill_texts.names.is_empty() {
            out.push(format!(
                "nie-data     skill_text (NOUN_INFO) : {} nom(s) localisé(s)",
                skill_texts.names.len()
            ));
            for (hash, name) in skill_texts.names.iter().take(8) {
                out.push(format!("  {hash} — {name}"));
            }
            if skill_texts.names.len() > 8 {
                out.push(format!("  … {} de plus", skill_texts.names.len() - 8));
            }
        }
        if !skill_texts.descriptions.is_empty() {
            out.push(format!(
                "nie-data     skill_text (TEXT_INFO) : {} description(s)",
                skill_texts.descriptions.len()
            ));
        }
    }
}

/// Essaie les parseurs spécialisés `nie-data` dont la source est un RDBN « lists »
/// (`skill`/`skill_technic`) sur le JSON ponté depuis `lists`. Même principe de tolérance que
/// [`append_nie_data_t2b`] — les ~65 autres modules RDBN suivent le même principe.
fn append_nie_data_rdbn(out: &mut Vec<String>, lists: &[cfgbin::RdbnList]) {
    let json = bridge::rdbn_to_json(lists);

    let skills = nie_data::skill::parse_skill_config(&json);
    if !skills.is_empty() {
        out.push(format!(
            "nie-data     skill (m_skillInfoList) : {} technique(s)",
            skills.len()
        ));
        for s in skills.iter().take(8) {
            out.push(format!("  {:<10} id={}", s.skill_id_str, s.skill_id));
        }
        if skills.len() > 8 {
            out.push(format!("  … {} de plus", skills.len() - 8));
        }
    }

    let technics = nie_data::skill_technic::parse_skill_technic_config(&json);
    if !technics.is_empty() {
        out.push(format!(
            "nie-data     skill_technic (m_SkillTechnicInfoList) : {} entrée(s)",
            technics.len()
        ));
    }
}

/// Les `n` premiers octets de `data`, formatés en hex majuscule espacé (`"47 34 4D 44"`).
#[must_use]
pub fn hex_prefix(data: &[u8], n: usize) -> String {
    data.iter()
        .take(n)
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Résumé lisible d'un conteneur Level-5 générique (en-tête commun 16 octets, cf.
/// [`nie_formats::level5`]) : G4CM/G4MA/G4VS/G4LA/NAVM/PXCL — le corps n'est pas toujours décodé
/// (sémantique non confirmée pour certains), mais l'en-tête + l'invariant de taille le sont.
#[must_use]
pub fn describe_l5_container(
    label: &str,
    header: level5::Level5Header,
    file_size: usize,
    consistent: bool,
) -> Vec<String> {
    vec![
        format!("format      {label}"),
        format!(
            "header_size 0x{:X}  type_id 0x{:02X}",
            header.header_size, header.type_id
        ),
        format!(
            "data_size   {} octets (fichier {file_size} octets)",
            header.data_size
        ),
        format!("cohérent    {consistent}"),
    ]
}

/// Détecte le format d'un fichier VFS et produit un résumé structuré (une ligne par entrée).
/// `None` si aucun décodeur ne reconnaît le contenu — l'appelant retombe sur un hexdump.
///
/// `path` sert uniquement à discriminer les formats sans magic fiable (T2B/RDBN partagent
/// l'extension `.cfg.bin` ; le T2B n'a pas de magic du tout) — jamais à deviner un format que
/// les octets ne confirment pas.
#[must_use]
pub fn describe_content(path: &str, data: &[u8]) -> Option<Vec<String>> {
    if nie_lua::is_lua52_bytecode(data) {
        let mut out = vec!["format      Lua 5.2 bytecode (script moteur .lua.bin)".to_string()];
        let name = path.rsplit('/').next().unwrap_or(path);
        match nie_lua::discover_host_calls(data, name) {
            Ok(calls) => {
                out.push(format!(
                    "appels hôte {} (VM stubbée — pas d'exécution de la vraie logique)",
                    calls.len()
                ));
                for c in calls.iter().take(40) {
                    out.push(format!("  {c}"));
                }
                if calls.len() > 40 {
                    out.push(format!("  … {} de plus", calls.len() - 40));
                }
            }
            Err(e) => out.push(format!("  (échec d'introspection : {e})")),
        }
        return Some(out);
    }

    if g4mt::is_g4mt(data) {
        let mut out = vec!["format      G4MT (animation Level-5)".to_string()];
        match g4mt::Motion::parse(data) {
            Some(motion) => {
                out.push(format!("clips       {}", motion.clips.len()));
                out.push(format!("cibles      {}", motion.target_hashes.len()));
                for c in motion.clips.iter().take(15) {
                    out.push(format!(
                        "  clip  {:<24} frames {:>4}-{:<4} ({} fr) fps={}{}",
                        c.name,
                        c.start_frame,
                        c.end_frame,
                        c.frame_count(),
                        c.fps,
                        if c.is_additive() { "  [additive]" } else { "" }
                    ));
                }
                if motion.clips.len() > 15 {
                    out.push(format!("  … {} clip(s) de plus", motion.clips.len() - 15));
                }
            }
            None => out.push("  (échec du décodage structurel)".to_string()),
        }
        return Some(out);
    }

    if g4sk::is_g4sk(data) {
        let mut out = vec!["format      G4SK (squelette)".to_string()];
        if let Ok(header) = g4sk::parse_header(data) {
            let bones = g4sk::parse_hierarchy(data, &header);
            out.push(format!(
                "os          {} ({})",
                bones.bones.len(),
                if bones.heuristic {
                    "heuristique"
                } else {
                    "table réelle"
                }
            ));
            for b in bones.bones.iter().take(20) {
                out.push(format!(
                    "  #{:<3} parent={:<4} {}",
                    b.index, b.parent_index, b.name
                ));
            }
            if bones.bones.len() > 20 {
                out.push(format!("  … {} os de plus", bones.bones.len() - 20));
            }
        }
        return Some(out);
    }

    if g4md::is_g4md(data) {
        let mut out = vec!["format      G4MD (métadonnées de modèle)".to_string()];
        if let Ok(md) = g4md::parse(data) {
            out.push(format!("sous-mailles {}", md.submeshes.len()));
            out.push(format!("matériaux    {}", md.header.material_count));
            out.push(format!("os référencés {}", md.header.bone_count));
            for (i, name) in md.material_base_names.iter().enumerate().take(10) {
                out.push(format!("  mat[{i}] {name}"));
            }
            for (i, sm) in md.submeshes.iter().enumerate().take(10) {
                out.push(format!(
                    "  sm[{i}] vtx={} idx={} stride={} mat={}",
                    sm.vertex_count, sm.index_count, sm.stride, sm.material_index
                ));
            }
        }
        return Some(out);
    }

    if g4tx::is_g4tx(data) {
        let mut out = vec!["format      G4TX (texture)".to_string()];
        if let Ok(g) = g4tx::parse(data) {
            out.push(format!("textures    {}", g.textures.len()));
            for t in &g.textures {
                out.push(format!(
                    "  id={:<3} {:<24} {}x{}  {}",
                    t.id,
                    t.name,
                    t.width,
                    t.height,
                    if t.is_dds { "DDS" } else { "NXTCH" }
                ));
                for s in &t.sub_textures {
                    out.push(format!(
                        "      région {:<20} {}x{} @({},{})",
                        s.name, s.width, s.height, s.x, s.y
                    ));
                }
            }
            out.push("  (décodage PNG disponible via nie_formats::g4tx_decode)".to_string());
        }
        return Some(out);
    }

    if col::is_pxcl(data)
        && let Ok(p) = col::parse(data)
    {
        return Some(describe_l5_container(
            "PXCL (collision PhysX cooked, non décodée)",
            p.header,
            p.file_size,
            p.is_size_consistent(),
        ));
    }

    if dxbc::is_dxbc(data) {
        let mut out = vec!["format      DXBC (shader compilé, format Microsoft)".to_string()];
        if let Ok(d) = dxbc::parse(data) {
            out.push(format!(
                "chunks      {}  cohérent={}",
                d.chunks.len(),
                d.is_size_consistent()
            ));
            for c in &d.chunks {
                out.push(format!(
                    "  {}  {} octets",
                    String::from_utf8_lossy(&c.fourcc),
                    c.size
                ));
            }
        }
        return Some(out);
    }

    if g4pk::is_g4pk(data) || g4pk::is_g4ra(data) {
        let mut out = vec!["format      G4PK/G4RA (archive)".to_string()];
        if let Ok(g) = g4pk::parse(data) {
            out.push(format!(
                "sous-fichiers {}  variante={:?}",
                g.files.len(),
                g.header.kind
            ));
            for f in g.files.iter().take(20) {
                out.push(format!("  {:<32} {} octets", f.name, f.size));
            }
            if g.files.len() > 20 {
                out.push(format!("  … {} de plus", g.files.len() - 20));
            }
        }
        return Some(out);
    }

    if navm::is_navm(data)
        && let Ok(n) = navm::parse(data)
    {
        let mut out = describe_l5_container(
            "NAVM (navmesh)",
            n.header,
            n.file_size,
            n.is_size_consistent(),
        );
        out.push(format!("sections    {:?}", n.section_counts));
        return Some(out);
    }

    if g4cm::is_g4cm(data)
        && let Ok(c) = g4cm::parse(data)
    {
        // Le `.g4cm` est décodé pour de bon depuis que le codec a rejoint `nie-formats` :
        // afficher ses objets et ses canaux, pas seulement l'en-tête du conteneur.
        let mut out = describe_l5_container(
            "G4CM (caméra de cutscene)",
            c.header,
            data.len(),
            c.header.is_size_consistent(data.len()),
        );
        out.push(format!(
            "objets      {} ({})",
            c.objects.len(),
            c.names.join(", ")
        ));
        out.push(format!("canaux      {}", c.channels.len()));
        if let Some((a, b)) = c.frame_range() {
            out.push(format!("frames      {a} → {b} ({} temps)", c.times.len()));
        }
        out.push(format!(
            "échantillons décodés {:.0} %",
            f64::from(c.decoded_ratio()) * 100.0
        ));
        return Some(out);
    }

    if g4ma::is_g4ma(data)
        && let Ok(m) = g4ma::parse(data)
    {
        return Some(describe_l5_container(
            "G4MA (animation de matériau, non décodée)",
            m.header,
            m.file_size,
            m.is_size_consistent(),
        ));
    }

    if g4vs::is_g4vs(data)
        && let Ok(v) = g4vs::parse(data)
    {
        return Some(describe_l5_container(
            "G4VS (effet d'événement, non décodé)",
            v.header,
            v.file_size,
            v.is_size_consistent(),
        ));
    }

    if g4la::is_g4la(data)
        && let Ok(l) = g4la::parse(data)
    {
        return Some(describe_l5_container(
            "G4LA (animation de lumière, non décodée)",
            l.header,
            l.file_size,
            l.is_size_consistent(),
        ));
    }

    if nxtch::is_nxtch(data)
        && let Ok(h) = nxtch::parse_header(data)
    {
        return Some(vec![
            "format      NXTCH (chunk texture Switch)".to_string(),
            format!(
                "dimensions  {}x{}  format={:?}",
                h.width,
                h.height,
                h.texture_format()
            ),
            format!(
                "mipmaps     {}  taille_donnees={}",
                h.mipmap_count, h.texture_data_size
            ),
        ]);
    }

    // `objbin::is_objb` ne fait que détecter le pied de page générique T2B (`01 74 32 62`,
    // commun à TOUS les cfg.bin arborescents) — on le restreint donc à l'extension `.objbin`
    // pour ne pas préempter le rendu générique T2B/RDBN ci-dessous sur les autres cfg.bin.
    if path.to_ascii_lowercase().ends_with(".objbin") && objbin::is_objb(data) {
        let mut out = vec!["format      OBJBIN (objet de menu)".to_string()];
        if let Ok(o) = objbin::parse(data) {
            out.push(format!(
                "nom         {}  type_moteur={}",
                o.name, o.engine_type
            ));
            out.push(format!("composants  {}", o.components.len()));
        }
        return Some(out);
    }

    if cri_audio::is_adx(data) {
        return Some(vec![
            "format      ADX (audio Criware brut)".to_string(),
            "  (décodage WAV disponible via nie_formats::cri_audio::adx_decode)".to_string(),
        ]);
    }
    if cri_audio::is_hca(data) {
        return Some(vec![
            "format      HCA (audio Criware chiffré, clé par fichier)".to_string(),
        ]);
    }
    if data.starts_with(b"AFS2") {
        let mut out = vec!["format      AWB/AFS2 (banque audio Criware)".to_string()];
        if let Ok(awb) = cri_audio::Awb::parse(data) {
            out.push(format!(
                "entrées     {}  subkey=0x{:04X}",
                awb.entries.len(),
                awb.subkey
            ));
        }
        return Some(out);
    }
    if data.starts_with(b"@UTF") {
        let mut out = vec!["format      @UTF (table Criware, ACB probable)".to_string()];
        if let Ok(acb) = cri_audio::acb_parse(data) {
            out.push(format!("nom         {}  cues={}", acb.name, acb.cue_count));
        }
        return Some(out);
    }
    if data.starts_with(b"CRID") {
        let mut out = vec!["format      USM/CRID (vidéo Criware Sofdec2)".to_string()];
        if let Ok(u) = cri_audio::usm_demux(data) {
            out.push(format!(
                "vidéo       {:?}  {}x{}  {} frame(s)  pistes audio={}",
                u.video_codec,
                u.width,
                u.height,
                u.frame_count,
                u.audio_tracks.len()
            ));
        }
        return Some(out);
    }

    if path.to_ascii_lowercase().ends_with(".cfg.bin") {
        if cfgbin::is_rdbn(data) {
            if let Ok(rdbn) = cfgbin::parse(data) {
                let lists = cfgbin::read_values(&rdbn, data);
                let mut out = vec![format!(
                    "format      RDBN (cfg.bin à listes), version {}",
                    rdbn.header.version
                )];
                for l in lists.iter().take(20) {
                    out.push(format!(
                        "  liste {:<28} type={:<24} {} ligne(s)",
                        l.name,
                        l.type_name,
                        l.rows.len()
                    ));
                }
                if lists.len() > 20 {
                    out.push(format!("  … {} liste(s) de plus", lists.len() - 20));
                }
                if let Some(first) = lists.first()
                    && let Some(row) = first.rows.first()
                {
                    let fields: Vec<&str> = row.fields.iter().map(|(n, _)| n.as_str()).collect();
                    out.push(format!(
                        "  champs de « {} » : {}",
                        first.name,
                        fields.join(", ")
                    ));
                }
                append_nie_data_rdbn(&mut out, &lists);
                return Some(out);
            }
        } else if let Ok(t2b) = cfgbin::cfgbin_parse(data) {
            let mut out = vec![format!(
                "format      T2B (cfg.bin arborescent), {} entrée(s) racine",
                t2b.entries.len()
            )];
            for e in t2b.entries.iter().take(20) {
                out.push(format!(
                    "  {:<28} {} variable(s), {} enfant(s)",
                    e.name,
                    e.variables.len(),
                    e.children.len()
                ));
            }
            if t2b.entries.len() > 20 {
                out.push(format!("  … {} entrée(s) de plus", t2b.entries.len() - 20));
            }
            append_nie_data_t2b(&mut out, &t2b);
            return Some(out);
        }
    }

    None
}
