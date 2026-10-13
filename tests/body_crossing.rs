// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use bevy::camera::visibility::RenderLayers;
use bevy::mesh::VertexAttributeValues;
use bevy::prelude::*;
use portal::install::{root, Install};
use portal::pair::Portal;
use portalgun::body::BodyPlugin;
use portalgun::bodyportal::{BodyMaterial, PortalGhost, GHOST_LAYER};
use portalgun::faces::Portals;
use portalgun::player::{Config, Player};
use portalgun::portals::Colour;
use portalgun::sound::Files;
use portalgun::units::from_bevy;
use portalgun::visual::VisualHistory;
use portalgun::walker::Walker;

fn body_app() -> App {
    let install = Install::open(&root(
        None,
        None,
        std::path::Path::new(&std::env::var("HOME").unwrap()),
    ))
    .unwrap();
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin))
        .init_asset::<Mesh>()
        .init_asset::<Image>()
        .init_resource::<Portals>()
        .insert_resource(Files(install.search))
        .insert_resource(Config(portalgun::settings::Settings::parse("").unwrap()))
        .add_plugins(BodyPlugin);
    app
}

fn pose(app: &mut App, player: Walker, pair: [Portal; 2]) {
    app.insert_resource(VisualHistory::new(&player).sample(&player, 0.0));
    app.insert_resource(Player(player));
    let mut portals = app.world_mut().resource_mut::<Portals>();
    portals.place(Colour::Blue, pair[0]);
    portals.place(Colour::Orange, pair[1]);
    app.update();
}

#[test]
fn grounded_feet_do_not_emit_a_copy_below_the_ceiling() {
    let mut app = body_app();
    let floor = Portal::new([0.0; 3], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0], (32.0, 54.0));
    let ceiling = Portal::new(
        [0.0, 0.0, 384.0],
        [0.0, 0.0, -1.0],
        [1.0, 0.0, 0.0],
        (32.0, 54.0),
    );
    for duck in [false, true] {
        for x in [0.0, 20.0, 40.0, 54.0, 60.0, 70.0] {
            let mut player = Walker::at([x, 0.0, 0.0]);
            player.duck.ducked = duck;
            player.duck.eye = if duck { 28.0 } else { 64.0 };
            pose(&mut app, player, [floor, ceiling]);
            let world = app.world_mut();
            let visible = *world
                .query_filtered::<&Visibility, With<PortalGhost>>()
                .single(world)
                .unwrap();
            let mut points = Vec::new();
            for (mesh, material, transform, layers) in world
                .query::<(
                    &Mesh3d,
                    &MeshMaterial3d<BodyMaterial>,
                    &GlobalTransform,
                    &RenderLayers,
                )>()
                .iter(world)
            {
                if *layers != RenderLayers::layer(GHOST_LAYER) {
                    continue;
                }
                let m = world
                    .resource::<Assets<BodyMaterial>>()
                    .get(&material.0)
                    .unwrap();
                let mesh = world.resource::<Assets<Mesh>>().get(&mesh.0).unwrap();
                if let Some(VertexAttributeValues::Float32x3(vertices)) =
                    mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                {
                    for v in vertices {
                        let p = transform.transform_point(Vec3::from(*v));
                        if m.extension.plane.dot(p.extend(1.0)) >= 0.0 {
                            points.push(Vec3::from(from_bevy(p)));
                        }
                    }
                }
            }
            let min_z = points.iter().map(|p| p.z).fold(f32::INFINITY, f32::min);
            let outside = points
                .iter()
                .filter(|p| !ceiling.inside(ceiling.local(p.to_array())))
                .count();
            assert_eq!(
                visible,
                Visibility::Hidden,
                "grounded ghost duck={duck} x={x} retained={} outside={outside} min_z={min_z}",
                points.len()
            );
        }
    }
}

#[test]
fn walking_body_copy_remains_visible_until_the_hull_crosses() {
    let mut app = body_app();
    let from = Portal::new(
        [0.0, 0.0, 54.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        (32.0, 54.0),
    );
    let to = Portal::new(
        [120.0, 0.0, 54.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        (32.0, 54.0),
    );
    for duck in [false, true] {
        for x in [20.0, 10.0, 5.0, 2.0, 1.0, 0.1, 0.0, -0.1, -2.0, -5.0] {
            let mut player = Walker::at([x, 0.0, 0.0]);
            player.yaw = 180.0;
            player.velocity = [-60.0, 0.0, 0.0];
            player.grounded = true;
            player.duck.ducked = duck;
            player.duck.eye = if duck { 28.0 } else { 64.0 };
            pose(&mut app, player, [from, to]);
            let world = app.world_mut();
            let visible = *world
                .query_filtered::<&Visibility, With<PortalGhost>>()
                .single(world)
                .unwrap();
            assert_eq!(
                visible,
                Visibility::Inherited,
                "body copy vanished duck={duck} x={x}"
            );
        }
    }
}

#[test]
fn portal_copy_uses_collision_centre_and_the_recessed_opening() {
    use portalgun::bodyportal::split;
    let floor = Portal::new([0.0; 3], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0], (32.0, 54.0));
    let exits = [
        Portal::new(
            [384.0, 0.0, 54.0],
            [-1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            (32.0, 54.0),
        ),
        Portal::new(
            [200.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 0.0],
            (32.0, 54.0),
        ),
        Portal::new(
            [200.0, 0.0, 384.0],
            [0.0, 0.0, -1.0],
            [1.0, 0.0, 0.0],
            (32.0, 54.0),
        ),
    ];
    for exit in exits {
        for (entity_z, allowed) in [
            (-5.001, false),
            (-5.0, true),
            (-4.999, true),
            (0.0, true),
            (18.0, true),
            (36.0, true),
        ] {
            for height in [18.0, 36.0] {
                for (bottom, intersects) in
                    [(-0.501, true), (-0.5, true), (-0.499, false), (0.0, false)]
                {
                    let selected = split(
                        Some([floor, exit]),
                        Vec3::Z * entity_z,
                        Vec3::Z * (height + bottom),
                        Vec3::new(16.0, 16.0, height),
                    );
                    assert_eq!(
                        selected.is_some(),
                        allowed && intersects,
                        "entity_z={entity_z} height={height} bottom={bottom} exit={exit:?}"
                    );
                }
            }
        }
        for (axis, edge) in [(Vec3::X, 52.92), (Vec3::Y, 31.36)] {
            for offset in [-0.001, 0.001] {
                let selected = split(
                    Some([floor, exit]),
                    Vec3::ZERO,
                    axis * (edge + 16.0 + offset),
                    Vec3::splat(16.0),
                );
                assert_eq!(
                    selected.is_some(),
                    offset < 0.0,
                    "axis={axis} offset={offset}"
                );
            }
        }
        for (bottom, intersects) in [(-501.001, false), (-501.0, true), (-500.999, true)] {
            assert_eq!(
                split(Some([floor, exit]), Vec3::ZERO, Vec3::Z * bottom, Vec3::ONE).is_some(),
                intersects
            );
        }
    }
}

#[test]
fn seeded_crouch_poses_keep_the_copy_at_the_crossing_plane() {
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;
    let mut app = body_app();
    let pair = [
        Portal::new(
            [0.0, 0.0, 54.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            (32.0, 54.0),
        ),
        Portal::new(
            [128.0, 64.0, 54.0],
            [0.0, -1.0, 0.0],
            [0.0, 0.0, 1.0],
            (32.0, 54.0),
        ),
    ];
    let mut seed = 0x734be51a_u64;
    for case in 0..2048 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let fraction = (seed >> 40) as f32 / (1u32 << 24) as f32;
        let mut player = Walker::at([fraction * 2.0, (fraction - 0.5) * 24.0, 0.0]);
        player.yaw = 135.0 + fraction * 90.0;
        player.velocity = [-20.0 - fraction * 40.0, (fraction - 0.5) * 30.0, 0.0];
        player.grounded = true;
        player.duck.ducked = case % 3 != 0;
        player.duck.eye = if player.duck.ducked { 28.0 } else { 64.0 };
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
            0.01 + fraction * 0.02,
        )));
        pose(&mut app, player, pair);
        let world = app.world_mut();
        assert_eq!(
            *world
                .query_filtered::<&Visibility, With<PortalGhost>>()
                .single(world)
                .unwrap(),
            Visibility::Inherited,
            "copy lost seed={seed} case={case}"
        );
    }
}

#[test]
fn moving_at_the_floor_rim_does_not_push_boots_through_the_ceiling() {
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;
    let mut app = body_app();
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
        0.015,
    )));
    let floor = Portal::new([0.0; 3], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0], (32.0, 54.0));
    let ceiling = Portal::new(
        [0.0, 0.0, 384.0],
        [0.0, 0.0, -1.0],
        [1.0, 0.0, 0.0],
        (32.0, 54.0),
    );
    for duck in [false, true] {
        for tick in 0..240 {
            let mut player = Walker::at([60.0, 0.0, 0.0]);
            player.grounded = true;
            player.duck.ducked = duck;
            player.duck.eye = if duck { 28.0 } else { 64.0 };
            player.yaw = 180.0;
            player.velocity = [-50.0, 0.0, 0.0];
            pose(&mut app, player, [floor, ceiling]);
            let world = app.world_mut();
            let visible = *world
                .query_filtered::<&Visibility, With<PortalGhost>>()
                .single(world)
                .unwrap();
            if visible == Visibility::Hidden {
                continue;
            }
            let mut leaks = Vec::new();
            for (mesh, material, transform, layers) in world
                .query::<(
                    &Mesh3d,
                    &MeshMaterial3d<BodyMaterial>,
                    &GlobalTransform,
                    &RenderLayers,
                )>()
                .iter(world)
            {
                if *layers != RenderLayers::layer(GHOST_LAYER) {
                    continue;
                }
                let m = world
                    .resource::<Assets<BodyMaterial>>()
                    .get(&material.0)
                    .unwrap();
                let mesh = world.resource::<Assets<Mesh>>().get(&mesh.0).unwrap();
                if let Some(VertexAttributeValues::Float32x3(vertices)) =
                    mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                {
                    for v in vertices {
                        let p = transform.transform_point(Vec3::from(*v));
                        let source = from_bevy(p);
                        let local = ceiling.local(source);
                        if m.extension.plane.dot(p.extend(1.0)) >= 0.0
                            && local[0] > 0.0
                            && !ceiling.inside(local)
                        {
                            leaks.push(source);
                        }
                    }
                }
            }
            assert!(
                leaks.is_empty(),
                "ceiling leak duck={duck} tick={tick} vertices={leaks:?}"
            );
        }
    }
}

#[test]
fn floor_support_filter_preserves_airborne_and_crossing_halves() {
    use portalgun::bodyportal::supported_floor;
    let floor = Portal::new([0.0; 3], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0], (32.0, 54.0));
    let wall = Portal::new([0.0; 3], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], (32.0, 54.0));
    let ceiling = Portal::new([0.0; 3], [0.0, 0.0, -1.0], [1.0, 0.0, 0.0], (32.0, 54.0));
    for height in [18.0, 36.0] {
        let half = Vec3::new(16.0, 16.0, height);
        for offset in [-0.03225, -0.03125, -0.03025, 0.0, 0.001] {
            let centre = Vec3::Z * (height + offset);
            assert_eq!(
                supported_floor(&floor, centre, half, true),
                offset >= -0.03125
            );
            assert!(!supported_floor(&floor, centre, half, false));
            assert!(!supported_floor(&wall, Vec3::X * 20.0, half, true));
            assert!(!supported_floor(&ceiling, Vec3::NEG_Z * 40.0, half, true));
        }
    }
}

#[test]
fn floor_placement_roundoff_does_not_reenable_the_grounded_copy() {
    let floor = Portal::new(
        [0.0, 0.0, 0.0000038146973],
        [0.0, 0.0, 1.0],
        [1.0, 0.0, 0.0],
        (32.0, 54.0),
    );
    assert!(portalgun::bodyportal::supported_floor(
        &floor,
        Vec3::Z * 36.0,
        Vec3::new(16.0, 16.0, 36.0),
        true
    ));
}

#[test]
#[allow(clippy::approx_constant)]
fn tilted_walls_do_not_count_as_supporting_floor_portals() {
    for up in [0.001_f32, 0.7070, 0.7071, 0.7072, 1.0] {
        let normal = Vec3::new((1.0 - up * up).sqrt(), 0.0, up);
        let portal = Portal::new(
            [0.0; 3],
            normal.to_array(),
            Vec3::Y.to_array(),
            (32.0, 54.0),
        );
        assert_eq!(
            portalgun::bodyportal::supported_floor(&portal, normal * 60.0, Vec3::splat(16.0), true),
            up > 0.7071
        );
    }
}

#[test]
fn missing_body_parts_hide_the_copy_and_log_each_failure_episode() {
    use bevy::ecs::schedule::SingleThreadedExecutor;
    use portalgun::body::BODY_LAYER;
    let mut app = body_app();
    app.edit_schedule(PostUpdate, |s| {
        s.set_executor(SingleThreadedExecutor::new());
    });
    let floor = Portal::new([0.0; 3], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0], (32.0, 54.0));
    let ceiling = Portal::new(
        [0.0, 0.0, 384.0],
        [0.0, 0.0, -1.0],
        [1.0, 0.0, 0.0],
        (32.0, 54.0),
    );
    pose(&mut app, Walker::at([0.0, 0.0, -4.0]), [floor, ceiling]);
    let world = app.world_mut();
    let ghost = world
        .query_filtered::<Entity, With<PortalGhost>>()
        .single(world)
        .unwrap();
    assert_eq!(
        *world.get::<Visibility>(ghost).unwrap(),
        Visibility::Inherited
    );
    let part = world
        .query::<(Entity, &Mesh3d, &RenderLayers)>()
        .iter(world)
        .find(|(_, _, layers)| **layers == RenderLayers::layer(BODY_LAYER))
        .unwrap()
        .0;
    let saved = *world.get::<Transform>(part).unwrap();
    let path =
        std::env::temp_dir().join(format!("portalgun-body-fault-{}.log", std::process::id()));
    let file = portalgun::diagnostics::LogFile::open(&path).unwrap();
    let subscriber = bevy::log::tracing_subscriber::fmt()
        .with_ansi(false)
        .with_writer(file.clone())
        .finish();
    bevy::log::tracing::subscriber::with_default(subscriber, || {
        app.world_mut().entity_mut(part).remove::<Transform>();
        app.update();
        app.update();
        assert_eq!(
            *app.world().get::<Visibility>(ghost).unwrap(),
            Visibility::Hidden
        );
        assert!(app
            .world()
            .resource::<Assets<BodyMaterial>>()
            .iter()
            .all(|(_, m)| m.extension.plane == Vec4::ZERO));
        app.world_mut().entity_mut(part).insert(saved);
        app.update();
        app.world_mut().entity_mut(part).remove::<Transform>();
        app.update();
    });
    file.flush().unwrap();
    let log = std::fs::read_to_string(&path).unwrap();
    assert_eq!(log.matches("reason=missing_component").count(), 2, "{log}");
    assert!(log.contains(&format!("entity=Some({part:?})")), "{log}");
    drop(file);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn supported_floor_does_not_hide_a_valid_wall_copy_at_the_corner() {
    use bevy::time::TimeUpdateStrategy;
    use portalgun::units::to_bevy_dir;
    use std::time::Duration;
    let floor = Portal::new(
        [54.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        [1.0, 0.0, 0.0],
        (32.0, 54.0),
    );
    let wall = Portal::new(
        [0.0, 0.0, 54.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        (32.0, 54.0),
    );
    for pair in [[floor, wall], [wall, floor]] {
        for duck in [false, true] {
            let mut app = body_app();
            app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
                0.015,
            )));
            for tick in 0..120 {
                let mut player = Walker::at([1.0, 0.0, 0.0]);
                player.grounded = true;
                player.yaw = 180.0;
                player.velocity = [-50.0, 0.0, 0.0];
                player.duck.ducked = duck;
                player.duck.eye = if duck { 28.0 } else { 64.0 };
                pose(&mut app, player, pair);
                let world = app.world_mut();
                let visible = *world
                    .query_filtered::<&Visibility, With<PortalGhost>>()
                    .single(world)
                    .unwrap();
                assert_eq!(
                    visible,
                    Visibility::Inherited,
                    "wall copy hidden pair={pair:?} duck={duck} tick={tick}"
                );
                for (_, material) in world.resource::<Assets<BodyMaterial>>().iter() {
                    if !material.extension.ghost {
                        assert_eq!(
                            material.extension.plane.truncate(),
                            to_bevy_dir(wall.forward),
                            "wrong split pair={pair:?} duck={duck} tick={tick}"
                        );
                    }
                }
            }
        }
    }
}
