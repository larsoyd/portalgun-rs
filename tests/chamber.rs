use std::path::Path;

use portal::install::{root, Install};
use portal::placement::{place, Failure, Placement, Surface, World};
use portalgun::chamber::{noportal, Chamber};

fn chamber() -> Chamber {
    let dir = root(None, None, Path::new(&std::env::var("HOME").unwrap()));
    let install = Install::open(&dir).unwrap_or_else(|e| panic!("install at {dir:?}: {e}"));
    Chamber::test_chamber(&install.search).unwrap()
}

#[test]
fn noportal_in_the_material_is_what_makes_a_surface_refuse_portals() {
    assert!(noportal("\"LightmappedGeneric\" { \"%noportal\" 1 }").unwrap());
    assert!(!noportal("\"LightmappedGeneric\" { \"%noportal\" 0 }").unwrap());
    assert!(!noportal("\"LightmappedGeneric\" { \"%keywords\" \"portal\" }").unwrap());
}

#[test]
fn a_shot_down_lands_on_the_floor_and_a_portal_can_open_there() {
    let c = chamber();
    let hit = c.cast([0.0, 0.0, 64.0], [0.0, 0.0, -1.0], 1000.0).unwrap();
    assert!((hit.distance - 64.0).abs() < 1e-3, "{}", hit.distance);
    assert_eq!(hit.normal, [0.0, 0.0, 1.0]);
    assert_eq!(hit.surface, Surface::Portalable);
}

#[test]
fn a_ray_too_short_to_reach_anything_misses() {
    let c = chamber();
    assert!(c.cast([0.0, 0.0, 64.0], [0.0, 0.0, -1.0], 63.9).is_none());
    assert!(c.cast([0.0, 0.0, 64.0], [0.0, 0.0, -1.0], 64.1).is_some());
}

#[test]
fn the_metal_strip_refuses_and_the_concrete_beside_it_takes_a_portal() {
    let c = chamber();
    let eye = [0.0, 0.0, 64.0];
    let metal = c.cast(eye, [0.0, 1.0, 0.0], 1000.0).unwrap();
    assert_eq!(metal.surface, Surface::Rejects);
    assert_eq!(metal.normal, [0.0, -1.0, 0.0]);
    let (placed, _) = place(&c, eye, [0.0, 1.0, 0.0], 10000.0, None);
    assert_eq!(placed, Placement::Failed(Failure::BadSurface));
    let wall = c.cast(eye, [-1.0, 0.0, 0.0], 1000.0).unwrap();
    assert_eq!(
        (wall.surface, wall.normal),
        (Surface::Portalable, [1.0, 0.0, 0.0])
    );
    let Placement::Placed { portal, .. } = place(&c, eye, [-1.0, 0.0, 0.0], 10000.0, None).0 else {
        panic!("no portal on the west wall")
    };
    assert_eq!(portal.forward, [1.0, 0.0, 0.0]);
    assert!(
        (portal.origin[0] + 384.0).abs() < 1e-3,
        "{:?}",
        portal.origin
    );
}

#[test]
fn the_nearest_face_wins_when_a_ray_crosses_several_blocks() {
    let c = chamber();
    let hit = c
        .cast([0.0, -160.0, 64.0], [1.0, 0.0, 0.0], 2000.0)
        .unwrap();
    assert!(
        (hit.distance - 192.0).abs() < 1e-3,
        "ledge front at 192, got {}",
        hit.distance
    );
    assert_eq!(hit.normal, [-1.0, 0.0, 0.0]);
}

#[test]
fn a_slanted_ray_over_the_cube_flies_on_to_the_ceiling() {
    let c = chamber();
    let d = std::f32::consts::FRAC_1_SQRT_2;
    let hit = c.cast([-200.0, 112.0, 16.0], [d, 0.0, d], 2000.0).unwrap();
    assert!(
        (hit.distance - 368.0 * 2f32.sqrt()).abs() < 1e-2,
        "{}",
        hit.distance
    );
    assert_eq!(hit.normal, [0.0, 0.0, -1.0]);
}

#[test]
fn a_ray_just_past_the_cubes_edge_lands_on_its_top() {
    let c = chamber();
    let d = std::f32::consts::FRAC_1_SQRT_2;
    let hit = c.cast([-200.0, 112.0, 72.5], [d, 0.0, -d], 2000.0).unwrap();
    assert!(
        (hit.distance - 40.5 * 2f32.sqrt()).abs() < 1e-2,
        "{}",
        hit.distance
    );
    assert_eq!(hit.normal, [0.0, 0.0, 1.0]);
    assert_eq!(hit.surface, Surface::Rejects);
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

#[test]
fn every_face_winds_outward_with_world_aligned_texture_axes() {
    let c = chamber();
    let floor = &c.blocks[0];
    let faces = floor.faces();
    assert_eq!(faces.len(), 6);
    for f in &faces {
        let [a, b, _, d] = f.corners;
        let e1 = [0, 1, 2].map(|i| b[i] - a[i]);
        let e2 = [0, 1, 2].map(|i| d[i] - a[i]);
        let n = cross(e1, e2);
        let len = n.iter().map(|v| v * v).sum::<f32>().sqrt();
        assert_eq!(n.map(|v| v / len), f.normal, "winding of {:?}", f.normal);
    }
    let top = faces.iter().find(|f| f.normal == [0.0, 0.0, 1.0]).unwrap();
    for (p, uv) in top.corners.iter().zip(top.uv) {
        assert_eq!(uv, [p[0], -p[1]]);
    }
    let west = faces.iter().find(|f| f.normal == [-1.0, 0.0, 0.0]).unwrap();
    for (p, uv) in west.corners.iter().zip(west.uv) {
        assert_eq!(uv, [p[1], -p[2]]);
    }
    let north = faces.iter().find(|f| f.normal == [0.0, 1.0, 0.0]).unwrap();
    for (p, uv) in north.corners.iter().zip(north.uv) {
        assert_eq!(uv, [p[0], -p[2]]);
    }
}

#[test]
fn each_block_knows_its_materials_surface() {
    let c = chamber();
    assert_eq!(c.blocks[0].surfaceprop, "concrete", "the floor");
    let metal = c
        .blocks
        .iter()
        .find(|b| b.material.starts_with("metal/"))
        .unwrap();
    assert_eq!(metal.surfaceprop, "metal");
}

#[test]
fn placements_beside_the_ledge_do_not_bury_the_opening_in_it() {
    let c = chamber();
    for y in [-63.9, -60.0, -48.0, -33.0] {
        for z in [32.0, 54.0, 64.0, 100.0, 128.1, 140.0] {
            let (placement, _) = place(&c, [0.0, y, z], [1.0, 0.0, 0.0], 1000.0, None);
            if let Placement::Placed { portal: p, .. } = placement {
                for r in -32..=32 {
                    for u in -54..=54 {
                        let q = p.world([0.1, r as f32, u as f32]);
                        assert!(
                            !c.blocks.iter().any(|b| (0..3)
                                .all(|i| q[i] > b.min[i] + 0.01 && q[i] < b.max[i] - 0.01)),
                            "shot y={y} z={z}, portal={p:?}, buried point={q:?}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn portal_clearance_checks_thin_obstructions_between_the_ray_samples() {
    use portal::pair::Portal;
    use portalgun::chamber::Block;
    let p = Portal::new(
        [0.0, 0.0, 54.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        (32.0, 54.0),
    );
    let mut c = Chamber {
        blocks: vec![Block {
            min: [-1.0, 7.0, 20.0],
            max: [10.0, 8.0, 30.0],
            material: "",
            surface: Surface::Rejects,
            surfaceprop: "default".into(),
        }],
    };
    assert!(!c.clear_portal(&p));
    c.blocks[0].max[0] = 0.0;
    assert!(c.clear_portal(&p), "support behind the opening is allowed");
    c.blocks[0].min = [-1.0, -100.0, -10.0];
    c.blocks[0].max = [100.0, 100.0, 0.0];
    assert!(c.clear_portal(&p), "the bottom can sit flush on the floor");
}
