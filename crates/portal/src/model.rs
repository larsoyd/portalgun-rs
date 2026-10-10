use crate::error::malformed;
use crate::mdl::{Mdl, Reader, VERTEX_SIZE};
use crate::{Error, Result};

const TRIANGLE_LIST: u8 = 0x1;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub tangent: [f32; 4],
    pub bones: [u8; 3],
    pub weights: [f32; 3],
    pub bone_count: u8,
}

pub struct MeshData {
    pub material: usize,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

/// The VVD vertices of one LOD, with its fixup table applied.
pub fn vertices(vvd: &[u8], lod: usize) -> Result<Vec<Vertex>> {
    let r = Reader::new(vvd, "VVD");
    if vvd.get(..4) != Some(b"IDSV") || r.i32(4)? != 4 {
        return Err(malformed("VVD", "no IDSV version 4 header"));
    }
    let data = r.i32(56)? as usize;
    let tangents = r.i32(60)? as usize;
    let vertex = |i: usize| -> Result<Vertex> {
        let o = data + VERTEX_SIZE * i;
        let [b0, b1, b2, count] = r.get::<4>(o + 12)?;
        Ok(Vertex {
            weights: r.f32s(o)?,
            bones: [b0, b1, b2],
            bone_count: count,
            pos: r.f32s(o + 16)?,
            normal: r.f32s(o + 28)?,
            uv: r.f32s(o + 40)?,
            tangent: r.f32s(tangents + 16 * i)?,
        })
    };
    let fixups = r.counted(r.i32(48)?, r.i32(52)? as usize, 12, |o| {
        Ok((r.i32(o)?, r.i32(o + 4)? as usize, r.i32(o + 8)? as usize))
    })?;
    if fixups.is_empty() {
        return (0..r.i32(16 + 4 * lod)? as usize).map(vertex).collect();
    }
    fixups
        .iter()
        .filter(|(l, _, _)| *l >= lod as i32)
        .flat_map(|&(_, start, n)| start..start + n)
        .map(vertex)
        .collect()
}

/// LOD 0 of the first model, one mesh per studio mesh with mesh-local indices.
pub fn meshes(mdl: &Mdl, vertices: &[Vertex], vtx: &[u8]) -> Result<Vec<MeshData>> {
    let r = Reader::new(vtx, "VTX");
    if r.i32(0)? != 7 {
        return Err(Error::Unsupported(format!("VTX version {}", r.i32(0)?)));
    }
    let model = mdl
        .body_parts
        .first()
        .and_then(|p| p.first())
        .ok_or_else(|| malformed("MDL", "no body part model"))?;
    let part = r.offset(0, r.i32(32)?)?;
    let vmodel = r.offset(part, r.i32(part + 4)?)?;
    let lod = r.offset(vmodel, r.i32(vmodel + 4)?)?;
    let vmeshes = r.counted(r.i32(lod)?, r.offset(lod, r.i32(lod + 4)?)?, 9, Ok)?;
    if vmeshes.len() != model.meshes.len() {
        return Err(malformed(
            "VTX",
            format!(
                "{} meshes but the MDL has {}",
                vmeshes.len(),
                model.meshes.len()
            ),
        ));
    }
    vmeshes
        .iter()
        .zip(&model.meshes)
        .map(|(&vm, mesh)| {
            let first = model.vertex_index + mesh.vertex_offset;
            let own = vertices
                .get(first..first + mesh.vertex_count)
                .ok_or_else(|| {
                    malformed(
                        "VVD",
                        format!(
                            "mesh vertices {first}+{} past {}",
                            mesh.vertex_count,
                            vertices.len()
                        ),
                    )
                })?;
            let mut indices = Vec::new();
            let groups = r.counted(r.i32(vm)?, r.offset(vm, r.i32(vm + 4)?)?, 25, Ok)?;
            for g in groups {
                let ids = r.counted(r.i32(g)?, r.offset(g, r.i32(g + 4)?)?, 9, |v| {
                    Ok(r.i16(v + 4)? as u16 as u32)
                })?;
                let index_base = r.offset(g, r.i32(g + 12)?)?;
                for s in r.counted(r.i32(g + 16)?, r.offset(g, r.i32(g + 20)?)?, 27, Ok)? {
                    if r.get::<1>(s + 18)?[0] & TRIANGLE_LIST == 0 {
                        return Err(Error::Unsupported("VTX triangle strips".into()));
                    }
                    let start = index_base + 2 * r.i32(s + 4)? as usize;
                    for i in r.counted(r.i32(s)?, start, 2, |o| Ok(r.i16(o)? as u16 as usize))? {
                        let id = *ids.get(i).ok_or_else(|| {
                            malformed(
                                "VTX",
                                format!("index {i} past {} strip vertices", ids.len()),
                            )
                        })?;
                        if id as usize >= own.len() {
                            return Err(malformed(
                                "VTX",
                                format!("vertex {id} past the mesh's {}", own.len()),
                            ));
                        }
                        indices.push(id);
                    }
                }
            }
            Ok(MeshData {
                material: mesh.material,
                vertices: own.to_vec(),
                indices,
            })
        })
        .collect()
}
