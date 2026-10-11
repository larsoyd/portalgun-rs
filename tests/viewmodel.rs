use std::path::Path;

use bevy::math::{Mat4, Quat, Vec3};
use portal::install::{root, Install};
use portal::mdl::Mdl;
use portalgun::viewmodel::{bindposes, look, read_gun, skin, Blend};

fn install() -> Install {
    let dir = root(None, None, Path::new(&std::env::var("HOME").unwrap()));
    Install::open(&dir).unwrap_or_else(|e| panic!("install at {dir:?}: {e}"))
}

fn rest(m: &Mdl) -> Vec<Mat4> {
    let mut world: Vec<Mat4> = Vec::new();
    for b in &m.bones {
        let local = Mat4::from_rotation_translation(Quat::from_array(b.quat), Vec3::from(b.pos));
        world.push(b.parent.map_or(local, |p| world[p] * local));
    }
    world
}

#[test]
fn the_gun_has_its_hands_body_and_glass() {
    let i = install();
    let (mdl, meshes) = read_gun(&i.search).unwrap();
    assert_eq!(
        meshes.iter().map(|m| m.material).collect::<Vec<_>>(),
        [0, 1, 2]
    );
    assert_eq!(mdl.textures.len(), 3);
}

#[test]
fn every_vertex_is_fully_weighted_to_real_bones_and_faces_flip_winding() {
    let i = install();
    let (mdl, meshes) = read_gun(&i.search).unwrap();
    for mesh in &meshes {
        let s = skin(mesh);
        assert_eq!(s.positions.len(), mesh.vertices.len());
        for (j, w) in s.joints.iter().zip(&s.weights) {
            assert!((w.iter().sum::<f32>() - 1.0).abs() < 1e-3, "{w:?}");
            for k in 0..4 {
                assert!(
                    w[k] == 0.0 || (j[k] as usize) < mdl.bones.len(),
                    "{j:?} {w:?}"
                );
            }
        }
        assert_eq!(s.indices.len(), mesh.indices.len());
        assert_eq!(
            s.indices[..3],
            [mesh.indices[0], mesh.indices[2], mesh.indices[1]]
        );
    }
}

#[test]
fn each_bind_pose_undoes_its_bones_rest_transform() {
    let i = install();
    let (mdl, _) = read_gun(&i.search).unwrap();
    for (n, (bind, world)) in bindposes(&mdl).iter().zip(rest(&mdl)).enumerate() {
        let id = Mat4::from_cols_array(bind) * world;
        assert!(id.abs_diff_eq(Mat4::IDENTITY, 1e-3), "bone {n}: {id}");
    }
}

#[test]
fn materials_name_their_texture_and_blend_and_a_missing_texture_is_none() {
    let i = install();
    let (mdl, _) = read_gun(&i.search).unwrap();
    let body = look(&i.search, &mdl, 1).unwrap();
    assert_eq!(
        body.texture.as_deref(),
        Some("models/weapons/v_models/v_portalgun/v_portalgun")
    );
    assert_eq!(body.blend, Blend::Opaque);
    let glass = look(&i.search, &mdl, 2).unwrap();
    assert_eq!(glass.blend, Blend::Add);
    let hands = look(&i.search, &mdl, 0).unwrap();
    assert_eq!(hands.texture, None);
    assert_eq!(
        hands.missing.as_deref(),
        Some("models/weapons/v_models/v_hands/v_hands")
    );
}

#[test]
fn a_vertex_keeps_all_three_of_its_bone_weights() {
    use portal::model::{MeshData, Vertex};
    let v = Vertex {
        bones: [4, 7, 9],
        weights: [0.5, 0.3, 0.2],
        bone_count: 3,
        ..Vertex::default()
    };
    let one = Vertex {
        bone_count: 1,
        weights: [1.0, 0.7, 0.2],
        ..v
    };
    let mesh = MeshData {
        material: 0,
        vertices: vec![v, one],
        indices: vec![0, 1, 0],
    };
    let s = skin(&mesh);
    assert_eq!(s.joints[0], [4, 7, 9, 0]);
    assert_eq!(s.weights[0], [0.5, 0.3, 0.2, 0.0]);
    assert_eq!(s.weights[1], [1.0, 0.0, 0.0, 0.0]);
}

#[test]
fn world_gun_loads_with_its_own_material_search_path_and_forward_muzzle() {
    let install = install();
    let (mdl, meshes) =
        portalgun::viewmodel::read_model(&install.search, "models/weapons/w_portalgun").unwrap();
    assert_eq!(mdl.bones.len(), 1);
    assert!(!meshes.is_empty());
    for piece in meshes {
        let material = look(&install.search, &mdl, piece.material).unwrap();
        let name = material.texture.unwrap();
        assert!(portalgun::texture::load(&install.search, &name).is_ok());
    }
    let muzzle = mdl.attachments.iter().find(|a| a.name == "muzzle").unwrap();
    assert!(muzzle.local[2][3] < -20.0);
}
