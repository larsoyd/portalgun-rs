// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use portal::pair::Portal;
use portal::placement::{
    orientation, place, snap_to_floor, Failure, Placement, RayHit, Surface, World, BUMP_LIMIT,
    HALF_HEIGHT, HALF_WIDTH,
};

/// flat rectangles: centre, normal, in-plane axis u, half extents along u and normal x u, surface
struct Panel([f32; 3], [f32; 3], [f32; 3], (f32, f32), Surface);

struct Panels(Vec<Panel>);

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

impl World for Panels {
    fn cast(&self, from: [f32; 3], dir: [f32; 3], max: f32) -> Option<RayHit> {
        let mut best: Option<RayHit> = None;
        for Panel(c, n, u, half, surface) in &self.0 {
            let denom = dot(dir, *n);
            if denom.abs() < 1e-6 {
                continue;
            }
            let t = dot([c[0] - from[0], c[1] - from[1], c[2] - from[2]], *n) / denom;
            if !(0.0..=max).contains(&t) || best.as_ref().is_some_and(|b| b.distance <= t) {
                continue;
            }
            let p = [
                from[0] + dir[0] * t - c[0],
                from[1] + dir[1] * t - c[1],
                from[2] + dir[2] * t - c[2],
            ];
            let v = cross(*n, *u);
            if dot(p, *u).abs() <= half.0 && dot(p, v).abs() <= half.1 {
                best = Some(RayHit {
                    distance: t,
                    normal: *n,
                    surface: *surface,
                });
            }
        }
        best
    }
}

const X: [f32; 3] = [1.0, 0.0, 0.0];
const Y: [f32; 3] = [0.0, 1.0, 0.0];
const Z: [f32; 3] = [0.0, 0.0, 1.0];

/// a wall facing +x, 1000 wide (y) and 600 tall (z), bottom edge at z = 0
fn wall(surface: Surface) -> Panel {
    Panel([0.0, 0.0, 300.0], X, Y, (500.0, 300.0), surface)
}

fn floor() -> Panel {
    Panel([500.0, 0.0, 0.0], Z, X, (500.0, 500.0), Surface::Portalable)
}

fn placed(p: (Placement, [f32; 3])) -> Portal {
    match p.0 {
        Placement::Placed { portal, .. } => portal,
        Placement::Failed(f) => panic!("failed: {f:?} at {:?}", p.1),
    }
}

fn shoot(world: &Panels, at: [f32; 3], other: Option<&Portal>) -> (Placement, [f32; 3]) {
    let from = [300.0, at[1], at[2]];
    let d = [at[0] - from[0], at[1] - from[1], at[2] - from[2]];
    let len = dot(d, d).sqrt();
    place(world, from, d.map(|v| v / len), 10_000.0, other)
}

#[test]
fn a_shot_into_the_middle_of_a_wall_opens_there_upright() {
    let world = Panels(vec![wall(Surface::Portalable)]);
    let (p, _) = shoot(&world, [0.0, 10.0, 300.0], None);
    let Placement::Placed { portal, quality } = p else {
        panic!("{p:?}")
    };
    assert!(
        (portal.origin[1] - 10.0).abs() < 0.01 && (portal.origin[2] - 300.0).abs() < 0.01,
        "{:?}",
        portal.origin
    );
    assert_eq!(portal.forward, X);
    assert_eq!(portal.up, Z);
    assert_eq!(quality, 1.0);
    assert_eq!(portal.half, (HALF_WIDTH, HALF_HEIGHT));
}

#[test]
fn floors_point_the_portal_top_along_the_shot() {
    let (forward, up) = orientation(Z, [0.6, 0.0, -0.8]);
    assert_eq!(forward, Z);
    assert!((up[0] - 1.0).abs() < 1e-5 && up[2].abs() < 1e-6, "{up:?}");
    let (_, up) = orientation([0.0, 0.6, 0.8], [0.0, 1.0, 0.0]);
    assert!(up[2] > 0.5, "sloped surfaces keep world up {up:?}");
}

#[test]
fn a_shot_near_an_edge_is_bumped_onto_the_surface() {
    let world = Panels(vec![wall(Surface::Portalable)]);
    let (p, _) = shoot(&world, [0.0, 480.0, 300.0], None);
    let Placement::Placed { portal, quality } = p else {
        panic!("{p:?}")
    };
    assert!(
        portal.origin[1] <= 500.0 - HALF_WIDTH + 0.5,
        "{:?}",
        portal.origin
    );
    assert!(
        portal.origin[1] >= 500.0 - HALF_WIDTH - 12.0,
        "{:?}",
        portal.origin
    );
    assert!(quality > 0.3 && quality < 1.0, "{quality}");
}

#[test]
fn a_shot_next_to_the_floor_sits_on_it() {
    let world = Panels(vec![wall(Surface::Portalable), floor()]);
    let portal = placed(shoot(&world, [0.0, 0.0, 30.0], None));
    assert!(
        (portal.origin[2] - HALF_HEIGHT).abs() < 0.6,
        "bottom edge flush: {:?}",
        portal.origin
    );
}

#[test]
fn a_panel_smaller_than_a_portal_cannot_fit() {
    let world = Panels(vec![Panel(
        [0.0, 0.0, 300.0],
        X,
        Y,
        (20.0, 300.0),
        Surface::Portalable,
    )]);
    assert!(matches!(
        shoot(&world, [0.0, 0.0, 300.0], None).0,
        Placement::Failed(Failure::CantFit)
    ));
}

#[test]
fn glass_moving_things_and_misses_fail() {
    let world = Panels(vec![wall(Surface::Rejects)]);
    assert!(matches!(
        shoot(&world, [0.0, 0.0, 300.0], None).0,
        Placement::Failed(Failure::BadSurface)
    ));
    let world = Panels(vec![wall(Surface::PassThrough)]);
    assert!(matches!(
        shoot(&world, [0.0, 0.0, 300.0], None).0,
        Placement::Failed(Failure::PassThrough)
    ));
    let world = Panels(vec![]);
    assert!(matches!(
        shoot(&world, [0.0, 0.0, 300.0], None).0,
        Placement::Failed(Failure::Missed)
    ));
}

#[test]
fn a_shot_beside_the_other_portal_moves_aside() {
    let world = Panels(vec![wall(Surface::Portalable)]);
    let other = placed(shoot(&world, [0.0, 0.0, 300.0], None));
    let portal = placed(shoot(&world, [0.0, 30.0, 300.0], Some(&other)));
    assert!(
        portal.origin[1] >= 2.0 * HALF_WIDTH - 0.5,
        "{:?}",
        portal.origin
    );
}

#[test]
fn a_tight_spot_between_portal_and_edge_fails_as_overlap() {
    let world = Panels(vec![Panel(
        [0.0, 0.0, 300.0],
        X,
        Y,
        (2.4 * HALF_WIDTH, 300.0),
        Surface::Portalable,
    )]);
    let other = placed(shoot(&world, [0.0, 0.0, 300.0], None));
    assert!(matches!(
        shoot(&world, [0.0, 30.0, 300.0], Some(&other)).0,
        Placement::Failed(Failure::Overlap)
    ));
}

#[test]
fn bumps_never_exceed_the_limit() {
    // a wide strip only just taller than a portal: a shot at its very top edge must move down
    let tall = HALF_HEIGHT + 4.0;
    let world = Panels(vec![Panel(
        [0.0, 0.0, 300.0],
        X,
        Y,
        (500.0, tall),
        Surface::Portalable,
    )]);
    let portal = placed(shoot(&world, [0.0, 0.0, 300.0 + tall - 1.0], None));
    assert!((portal.origin[2] - 300.0).abs() < BUMP_LIMIT);
    // a scrap of wall far from any space big enough
    let world = Panels(vec![
        Panel([0.0, 0.0, 300.0], X, Y, (10.0, 10.0), Surface::Portalable),
        Panel(
            [0.0, 400.0, 300.0],
            X,
            Y,
            (100.0, 300.0),
            Surface::Portalable,
        ),
    ]);
    assert!(matches!(
        shoot(&world, [0.0, 0.0, 300.0], None).0,
        Placement::Failed(Failure::CantFit)
    ));
}

#[test]
fn walls_standing_in_front_push_it_out_of_the_corner() {
    let side = Panel(
        [150.0, 200.0, 300.0],
        [0.0, -1.0, 0.0],
        X,
        (150.0, 300.0),
        Surface::Portalable,
    );
    let world = Panels(vec![wall(Surface::Portalable), side]);
    let portal = placed(shoot(&world, [0.0, 190.0, 300.0], None));
    assert!(
        portal.origin[1] <= 200.0 - HALF_WIDTH + 0.5,
        "{:?}",
        portal.origin
    );
}

fn panel(y: (f32, f32), z: (f32, f32), surface: Surface) -> Panel {
    Panel(
        [0.0, (y.0 + y.1) / 2.0, (z.0 + z.1) / 2.0],
        X,
        Y,
        ((y.1 - y.0) / 2.0, (z.1 - z.0) / 2.0),
        surface,
    )
}

fn origin_of(p: (Placement, [f32; 3])) -> [f32; 3] {
    placed(p).origin
}

#[test]
fn floor_portals_are_not_snapped_toward_their_bottom_edge() {
    let wall = Panel(
        [400.0 - HALF_HEIGHT - 1.0, 0.0, 100.0],
        X,
        Y,
        (500.0, 100.0),
        Surface::Portalable,
    );
    let world = Panels(vec![floor(), wall]);
    let from = [700.0, 0.0, 300.0];
    let d = [-300.0f32, 0.0, -300.0];
    let n = dot(d, d).sqrt();
    let p = placed(place(&world, from, d.map(|v| v / n), 10_000.0, None));
    assert!(p.up[0] < -0.99, "{:?}", p.up);
    assert!((p.origin[0] - 400.0).abs() < 0.01, "{:?}", p.origin);
}

#[test]
fn a_shelf_over_one_corner_pushes_it_away() {
    let (w, h) = (HALF_WIDTH, HALF_HEIGHT);
    let shelf = Panel(
        [10.0, 0.8 * w, 300.0 + 0.8 * h + 10.0],
        [0.0, -1.0, 0.0],
        X,
        (10.0, 0.2 * h + 20.0),
        Surface::Portalable,
    );
    let world = Panels(vec![wall(Surface::Portalable), shelf]);
    let o = origin_of(shoot(&world, [0.0, 0.0, 300.0], None));
    assert!(o[1].abs() > 1.0 || (o[2] - 300.0).abs() > 1.0, "{o:?}");
}

#[test]
fn a_portal_on_another_parallel_wall_does_not_crowd_it() {
    let world = Panels(vec![wall(Surface::Portalable)]);
    let behind = Portal::new([-100.0, 0.0, 300.0], X, Z, (HALF_WIDTH, HALF_HEIGHT));
    let o = origin_of(shoot(&world, [0.0, 0.0, 300.0], Some(&behind)));
    assert!(o[1].abs() < 0.01 && (o[2] - 300.0).abs() < 0.01, "{o:?}");
}

#[test]
fn it_keeps_off_glass_beside_the_wall() {
    let world = Panels(vec![
        panel((-500.0, 0.0), (0.0, 600.0), Surface::Portalable),
        panel((0.0, 500.0), (0.0, 600.0), Surface::Rejects),
    ]);
    let o = origin_of(shoot(&world, [0.0, -10.0, 300.0], None));
    assert!(o[1] <= -HALF_WIDTH + 0.5, "{o:?}");
}

#[test]
fn a_gap_at_the_middle_of_an_edge_moves_it() {
    let edge = HALF_WIDTH;
    let world = Panels(vec![
        panel((-500.0, edge - 10.0), (0.0, 600.0), Surface::Portalable),
        panel((edge + 10.0, 500.0), (0.0, 600.0), Surface::Portalable),
        panel(
            (edge - 10.0, edge + 10.0),
            (0.0, 290.0),
            Surface::Portalable,
        ),
        panel(
            (edge - 10.0, edge + 10.0),
            (310.0, 600.0),
            Surface::Portalable,
        ),
    ]);
    let o = origin_of(shoot(&world, [0.0, 0.0, 300.0], None));
    assert!(o[1].abs() > 1.0 || (o[2] - 300.0).abs() > 1.0, "{o:?}");
}

#[test]
fn upright_portals_drop_flush_onto_a_floor_just_below() {
    let world = Panels(vec![wall(Surface::Portalable), floor()]);
    let mut p = Portal::new(
        [0.0, 0.0, HALF_HEIGHT + 1.0],
        X,
        Z,
        (HALF_WIDTH, HALF_HEIGHT),
    );
    snap_to_floor(&world, &mut p);
    assert!((p.origin[2] - HALF_HEIGHT).abs() < 0.01, "{:?}", p.origin);
    let mut high = Portal::new(
        [0.0, 0.0, HALF_HEIGHT + 2.0],
        X,
        Z,
        (HALF_WIDTH, HALF_HEIGHT),
    );
    snap_to_floor(&world, &mut high);
    assert_eq!(high.origin[2], HALF_HEIGHT + 2.0);
    // leaning back past the limit: the floor lies within reach along its down but it stays put
    let z = 0.6 * (HALF_HEIGHT + 1.0);
    let mut leaning = Portal::new([0.0, 0.0, z], X, [0.0, 0.8, 0.6], (HALF_WIDTH, HALF_HEIGHT));
    snap_to_floor(&world, &mut leaning);
    assert_eq!(leaning.origin[2], z);
}

#[test]
fn the_orange_light_lets_a_shot_through_and_glass_or_noportal_refuse() {
    use portal::placement::surface_of;
    assert_eq!(
        surface_of("lights/light_orange001", false, b'C'),
        Surface::PassThrough
    );
    assert_eq!(
        surface_of("Lights/Light_Orange001", true, b'Y'),
        Surface::PassThrough
    );
    assert_eq!(
        surface_of("x/lights/light_orange001b", false, b'C'),
        Surface::PassThrough
    );
    assert_eq!(
        surface_of("lights/light_orange002", false, b'C'),
        Surface::Portalable
    );
    assert_eq!(
        surface_of("metal/metalwall048b", true, b'M'),
        Surface::Rejects
    );
    assert_eq!(
        surface_of("glass/glasswindow_refract01", false, b'Y'),
        Surface::Rejects
    );
    assert_eq!(
        surface_of("glass/glasswindow_refract01", false, b'X'),
        Surface::Portalable
    );
    assert_eq!(
        surface_of("concrete/concrete_modular_wall001a", false, b'C'),
        Surface::Portalable
    );
}
