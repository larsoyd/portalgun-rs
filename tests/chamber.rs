// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

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
fn the_sandbox_comes_from_its_vmf_with_every_side_shown() {
    let c = chamber();
    let bounds: Vec<_> = c.blocks.iter().map(|b| (b.min, b.max)).collect();
    assert_eq!(
        bounds,
        [
            ([-400.0, -272.0, -16.0], [400.0, 272.0, 0.0]),
            ([-400.0, -272.0, 384.0], [400.0, 272.0, 400.0]),
            ([-400.0, -272.0, 0.0], [-384.0, 272.0, 384.0]),
            ([384.0, -272.0, 0.0], [400.0, 272.0, 384.0]),
            ([-384.0, -272.0, 0.0], [384.0, -256.0, 384.0]),
            ([-384.0, 256.0, 0.0], [-64.0, 272.0, 384.0]),
            ([-64.0, 256.0, 0.0], [64.0, 272.0, 384.0]),
            ([64.0, 256.0, 0.0], [384.0, 272.0, 384.0]),
            ([192.0, -256.0, 0.0], [384.0, -64.0, 128.0]),
            ([-160.0, 96.0, 0.0], [-128.0, 128.0, 32.0]),
        ]
    );
    for b in &c.blocks {
        assert_eq!(b.sides.len(), 6);
        assert!(b.sides.iter().all(|s| s.points.len() == 4), "{:?}", b.min);
    }
    assert_eq!(c.blocks[6].sides[0].material, "metal/metalwall048b");
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
    let faces: Vec<_> = floor.faces().collect();
    assert_eq!(faces.len(), 6);
    for f in &faces {
        let (a, b, d) = (f.points[0], f.points[1], f.points[3]);
        let e1 = [0, 1, 2].map(|i| b[i] - a[i]);
        let e2 = [0, 1, 2].map(|i| d[i] - a[i]);
        let n = cross(e1, e2);
        let len = n.iter().map(|v| v * v).sum::<f32>().sqrt();
        assert_eq!(
            n.map(|v| v / len),
            f.plane.normal,
            "winding of {:?}",
            f.plane.normal
        );
    }
    let texels = |p: f32| p / 0.25;
    let side = |n: [f32; 3]| *faces.iter().find(|f| f.plane.normal == n).unwrap();
    let top = side([0.0, 0.0, 1.0]);
    for (p, uv) in top.points.iter().zip(&top.uv) {
        assert_eq!(*uv, [texels(p[0]), texels(-p[1])]);
    }
    let west = side([-1.0, 0.0, 0.0]);
    for (p, uv) in west.points.iter().zip(&west.uv) {
        assert_eq!(*uv, [texels(p[1]), texels(-p[2])]);
    }
    let north = side([0.0, 1.0, 0.0]);
    for (p, uv) in north.points.iter().zip(&north.uv) {
        assert_eq!(*uv, [texels(p[0]), texels(-p[2])]);
    }
}

#[test]
fn each_block_knows_its_materials_surface() {
    let c = chamber();
    assert_eq!(c.blocks[0].surfaceprop, "concrete", "the floor");
    let metal = c
        .blocks
        .iter()
        .find(|b| b.sides[0].material.starts_with("metal/"))
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
        blocks: vec![Block::cuboid(
            [-1.0, 7.0, 20.0],
            [10.0, 8.0, 30.0],
            "",
            Surface::Rejects,
            "default",
        )],
    };
    assert!(!c.clear_portal(&p));
    c.blocks[0] = Block::cuboid(
        [-1.0, 7.0, 20.0],
        [0.0, 8.0, 30.0],
        "",
        Surface::Rejects,
        "default",
    );
    assert!(c.clear_portal(&p), "support behind the opening is allowed");
    c.blocks[0] = Block::cuboid(
        [-1.0, -100.0, -10.0],
        [100.0, 100.0, 0.0],
        "",
        Surface::Rejects,
        "default",
    );
    assert!(c.clear_portal(&p), "the bottom can sit flush on the floor");
}

fn panel(x: f32, material: &str) -> String {
    let (a, b) = (x, x + 4.0);
    let side = |p: &str| {
        format!(
            "side {{ \"plane\" \"{p}\" \"material\" \"{material}\" \
             \"uaxis\" \"[0 1 0 0] 0.25\" \"vaxis\" \"[0 0 -1 0] 0.25\" }}"
        )
    };
    format!(
        "solid {{ \"id\" \"1\" {} {} {} {} {} {} }}",
        side(&format!("({a} 64 128) ({b} 64 128) ({b} -64 128)")),
        side(&format!("({a} -64 0) ({b} -64 0) ({b} 64 0)")),
        side(&format!("({a} 64 128) ({a} -64 128) ({a} -64 0)")),
        side(&format!("({b} 64 0) ({b} -64 0) ({b} -64 128)")),
        side(&format!("({b} 64 128) ({a} 64 128) ({a} 64 0)")),
        side(&format!("({b} -64 0) ({a} -64 0) ({a} -64 128)")),
    )
}

#[test]
fn glass_refuses_portals_and_the_orange_light_lets_shots_through() {
    let dir = root(None, None, Path::new(&std::env::var("HOME").unwrap()));
    let install = Install::open(&dir).unwrap();
    let text = format!(
        "world {{ {} {} }}",
        panel(100.0, "glass/glasswindow_refract01"),
        panel(200.0, "lights/light_orange001")
    );
    let c = Chamber::from_vmf(&install.search, &text).unwrap();
    let eye = [0.0, 0.0, 64.0];
    let glass = c.cast(eye, [1.0, 0.0, 0.0], 1000.0).unwrap();
    assert_eq!((glass.distance, glass.surface), (100.0, Surface::Rejects));
    let light = c.cast([150.0, 0.0, 64.0], [1.0, 0.0, 0.0], 1000.0).unwrap();
    assert_eq!(
        (light.distance, light.surface),
        (50.0, Surface::PassThrough)
    );
    assert_eq!(c.blocks[0].surfaceprop, "glass");
}

#[test]
fn a_brush_mixing_surfaceprops_is_refused() {
    let dir = root(None, None, Path::new(&std::env::var("HOME").unwrap()));
    let install = Install::open(&dir).unwrap();
    let wall = panel(100.0, "concrete/concrete_modular_wall001a");
    let mixed = wall.replacen(
        "concrete/concrete_modular_wall001a",
        "metal/metalwall048b",
        1,
    );
    let err = Chamber::from_vmf(&install.search, &format!("world {{ {mixed} }}"))
        .err()
        .unwrap()
        .to_string();
    assert!(err.contains("refused") && err.contains("metal"), "{err}");
    let nodraw = wall.replacen("concrete/concrete_modular_wall001a", "tools/toolsnodraw", 1);
    assert!(Chamber::from_vmf(&install.search, &format!("world {{ {nodraw} }}")).is_ok());
}
