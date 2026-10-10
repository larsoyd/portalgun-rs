mod common;

use common::*;
use portal::mdl::Mdl;
use portal::model::{meshes, vertices};

fn studio(meshes: Vec<(i32, i32, i32)>) -> Mdl {
    Mdl::parse(mdl(&MdlDef {
        bones: vec![bone("Root", -1, [0.0; 3])],
        anims: vec![],
        seqs: vec![],
        textures: vec!["a", "b"],
        cd: vec![],
        meshes,
        attachments: vec![],
    }))
    .unwrap()
}

#[test]
fn vertices_carry_weights_uvs_and_tangents() {
    let v = vertices(&vvd(&[vertex(1.0, 0), vertex(2.0, 0)], &[]), 0).unwrap();
    assert_eq!(v.len(), 2);
    assert_eq!(v[1].pos, [2.0, 0.0, 0.0]);
    assert_eq!(v[1].normal, [0.0, 0.0, 1.0]);
    assert_eq!(v[1].uv, [0.2, 0.5]);
    assert_eq!(v[1].tangent, [1.0, 1.0, 0.0, -1.0]);
    assert_eq!(
        (v[0].bones, v[0].weights, v[0].bone_count),
        ([0, 0, 0], [1.0, 0.0, 0.0], 1)
    );
}

#[test]
fn fixups_pick_the_vertices_of_a_lod() {
    let verts: Vec<_> = (0..6).map(|i| vertex(i as f32, 0)).collect();
    let fixups = [(1, 0, 2), (0, 2, 1), (1, 4, 2)];
    let lod0: Vec<f32> = vertices(&vvd(&verts, &fixups), 0)
        .unwrap()
        .iter()
        .map(|v| v.pos[0])
        .collect();
    assert_eq!(lod0, [0.0, 1.0, 2.0, 4.0, 5.0]);
    let lod1: Vec<f32> = vertices(&vvd(&verts, &fixups), 1)
        .unwrap()
        .iter()
        .map(|v| v.pos[0])
        .collect();
    assert_eq!(lod1, [0.0, 1.0, 4.0, 5.0]);
}

#[test]
fn strip_groups_map_back_to_mesh_vertices() {
    let mdl = studio(vec![(0, 3, 0), (1, 4, 3)]);
    let verts: Vec<_> = (0..7).map(|i| vertex(i as f32, 0)).collect();
    let vtx = vtx(&[
        vec![(vec![2, 1, 0], vec![vec![0, 1, 2]], 1)],
        vec![
            (vec![0, 1, 2], vec![vec![0, 1, 2]], 1),
            (vec![3, 2, 1], vec![vec![2, 1, 0], vec![0, 0, 1]], 1),
        ],
    ]);
    let m = meshes(&mdl, &vertices(&vvd(&verts, &[]), 0).unwrap(), &vtx).unwrap();
    assert_eq!(m.len(), 2);
    assert_eq!(m[0].material, 0);
    let xs: Vec<f32> = m[0].vertices.iter().map(|v| v.pos[0]).collect();
    assert_eq!(xs, [0.0, 1.0, 2.0]);
    assert_eq!(m[0].indices, [2, 1, 0]);
    assert_eq!(m[1].material, 1);
    assert_eq!(m[1].vertices.len(), 4);
    assert_eq!(m[1].vertices[3].pos[0], 6.0);
    assert_eq!(m[1].indices, [0, 1, 2, 1, 2, 3, 3, 3, 2]);
}

#[test]
fn triangle_strips_are_refused() {
    let mdl = studio(vec![(0, 3, 0)]);
    let verts: Vec<_> = (0..3).map(|i| vertex(i as f32, 0)).collect();
    let vtx = vtx(&[vec![(vec![0, 1, 2], vec![vec![0, 1, 2]], 2)]]);
    assert!(meshes(&mdl, &vertices(&vvd(&verts, &[]), 0).unwrap(), &vtx).is_err());
}

#[test]
fn indices_past_the_mesh_are_refused() {
    let mdl = studio(vec![(0, 3, 0)]);
    let verts: Vec<_> = (0..3).map(|i| vertex(i as f32, 0)).collect();
    let vtx = vtx(&[vec![(vec![0, 1, 5], vec![vec![0, 1, 2]], 1)]]);
    assert!(meshes(&mdl, &vertices(&vvd(&verts, &[]), 0).unwrap(), &vtx).is_err());
}
