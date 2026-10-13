// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use portal::keyvalues;
use portal::mdl::Mdl;
use portal::model::MeshData;
use portal::vpk::SearchPath;

const VIEW_MODEL: &str = "models/weapons/v_portalgun";

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Blend {
    Opaque,
    Blend,
    Add,
}

pub struct Look {
    pub texture: Option<String>,
    /// the base texture the material names when the game does not ship it
    pub missing: Option<String>,
    pub blend: Blend,
    pub phong: crate::model_material::Phong,
}

#[derive(Default)]
pub struct Skin {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub joints: Vec<[u16; 4]>,
    pub weights: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
}

pub fn read_gun(search: &SearchPath) -> Result<(Mdl, Vec<MeshData>), String> {
    read_model(search, VIEW_MODEL)
}

pub fn read_model(search: &SearchPath, name: &str) -> Result<(Mdl, Vec<MeshData>), String> {
    let file = |ext: &str| {
        let path = format!("{name}.{ext}");
        search.read(&path).map_err(|e| format!("{path}: {e}"))
    };
    let mdl = Mdl::parse(file("mdl")?).map_err(|e| format!("mdl: {e}"))?;
    let vertices = portal::model::vertices(&file("vvd")?, 0).map_err(|e| format!("vvd: {e}"))?;
    let meshes = portal::model::meshes(&mdl, &vertices, &file("dx90.vtx")?)
        .map_err(|e| format!("vtx: {e}"))?;
    Ok((mdl, meshes))
}

pub fn skin(mesh: &MeshData) -> Skin {
    let v = &mesh.vertices;
    Skin {
        positions: v.iter().map(|x| x.pos).collect(),
        normals: v.iter().map(|x| x.normal).collect(),
        uvs: v.iter().map(|x| x.uv).collect(),
        joints: v
            .iter()
            .map(|x| [x.bones[0], x.bones[1], x.bones[2], 0].map(u16::from))
            .collect(),
        weights: v
            .iter()
            .map(|x| {
                let mut w = [0.0; 4];
                let used = (x.bone_count as usize).min(3);
                w[..used].copy_from_slice(&x.weights[..used]);
                w
            })
            .collect(),
        // Source's front faces wind clockwise
        indices: mesh
            .indices
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|t| [t[0], t[2], t[1]])
            .collect(),
    }
}

/// Each bone's pose_to_bone, the model-to-bone matrix the studio header stores, column major.
pub fn bindposes(mdl: &Mdl) -> Vec<[f32; 16]> {
    mdl.bones
        .iter()
        .map(|b| {
            let m = b.pose_to_bone;
            [
                m[0][0], m[1][0], m[2][0], 0.0, m[0][1], m[1][1], m[2][1], 0.0, m[0][2], m[1][2],
                m[2][2], 0.0, m[0][3], m[1][3], m[2][3], 1.0,
            ]
        })
        .collect()
}

pub fn look(search: &SearchPath, mdl: &Mdl, index: usize) -> Result<Look, String> {
    let name = mdl
        .textures
        .get(index)
        .ok_or_else(|| format!("material {index} of {}", mdl.textures.len()))?;
    let path = mdl
        .cd_textures
        .iter()
        .map(|dir| {
            format!("materials/{dir}{name}.vmt")
                .replace('\\', "/")
                .to_ascii_lowercase()
        })
        .find(|path| search.contains(path))
        .ok_or_else(|| format!("material {name}: no VMT in model search directories"))?;
    let text = search.read(&path).map_err(|e| format!("{path}: {e}"))?;
    let root =
        keyvalues::parse(&String::from_utf8_lossy(&text)).map_err(|e| format!("{path}: {e}"))?;
    let shader = root.first().ok_or_else(|| format!("{path}: empty"))?;
    let base = shader
        .get("$basetexture")
        .map(|t| t.trim().replace('\\', "/").to_ascii_lowercase())
        .ok_or_else(|| format!("{path}: no $basetexture"))?;
    let shipped = search.contains(&format!("materials/{base}.vtf"));
    let on = |k: &str| shader.get(k).is_some_and(|v| v.trim() == "1");
    let blend = match (on("$translucent"), on("$additive")) {
        (_, true) => Blend::Add,
        (true, false) => Blend::Blend,
        _ => Blend::Opaque,
    };
    Ok(Look {
        texture: shipped.then(|| base.clone()),
        missing: (!shipped).then_some(base),
        blend,
        phong: crate::model_material::Phong::parse(shader)?,
    })
}
