// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use bevy::prelude::*;
use portal::pair::{through_point, Portal};
use portalgun::bodyportal::split;
use portalgun::units::to_bevy;

#[test]
fn a_straddling_body_is_clipped_into_complementary_portal_halves() {
    let entrance = Portal::new([0.0; 3], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0], (32.0, 54.0));
    for (normal, origin, up) in [
        ([1.0, 0.0, 0.0], [-384.0, 0.0, 64.0], [0.0, 0.0, 1.0]),
        ([0.0, 0.0, 1.0], [200.0, 0.0, 0.0], [1.0, 0.0, 0.0]),
        ([0.0, 0.0, -1.0], [200.0, 0.0, 384.0], [1.0, 0.0, 0.0]),
    ] {
        let exit = Portal::new(origin, normal, up, (32.0, 54.0));
        for centre in [-5.0, 0.0, 10.0, 35.99, 36.0] {
            let halves = split(
                Some([entrance, exit]),
                Vec3::Z * centre,
                Vec3::new(16.0, 16.0, 36.0),
            )
            .expect("body intersects the floor portal");
            assert!(halves.source.dot(to_bevy([0.0, 0.0, -0.299]).extend(1.0)) > 0.0);
            assert!(halves.source.dot(to_bevy([0.0, 0.0, -0.301]).extend(1.0)) < 0.0);
            for (z, keep) in [(0.749, true), (0.751, false)] {
                let p = to_bevy(through_point(&entrance, &exit, [0.0, 0.0, z]));
                assert_eq!(halves.exit.dot(p.extend(1.0)) >= 0.0, keep);
            }
            for z in [-30.0, -1.0, 1.0, 30.0] {
                let point = to_bevy([0.0, 0.0, z]);
                let mapped = halves.transform.transform_point3(point);
                assert!(mapped.abs_diff_eq(
                    to_bevy(through_point(&entrance, &exit, [0.0, 0.0, z])),
                    0.0001
                ));
                assert_eq!(halves.source.dot(point.extend(1.0)) >= 0.0, z > 0.0);
                assert_eq!(halves.exit.dot(mapped.extend(1.0)) >= 0.0, z < 0.0);
            }
        }
        assert!(split(
            Some([entrance, exit]),
            Vec3::Z * 36.01,
            Vec3::new(16.0, 16.0, 36.0)
        )
        .is_none());
        assert!(split(
            Some([entrance, exit]),
            Vec3::Z * -5.01,
            Vec3::new(16.0, 16.0, 36.0)
        )
        .is_none());
        assert!(split(
            Some([entrance, exit]),
            Vec3::X * 100.0,
            Vec3::new(16.0, 16.0, 36.0)
        )
        .is_none());
    }
}

#[test]
fn rendered_body_and_gun_share_the_pose_and_opposite_clip_planes() {
    use bevy::camera::visibility::RenderLayers;
    use portal::install::{root, Install};
    use portalgun::body::{BodyPlugin, BODY_LAYER};
    use portalgun::bodyportal::{BodyMaterial, PortalGhost, GHOST_LAYER};
    use portalgun::faces::Portals;
    use portalgun::player::Player;
    use portalgun::portals::Colour;
    use portalgun::sound::Files;
    use portalgun::visual::VisualHistory;
    use portalgun::walker::Walker;
    let home = std::env::var("HOME").unwrap();
    let install = Install::open(&root(None, None, std::path::Path::new(&home))).unwrap();
    let from = Portal::new([0.0; 3], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0], (32.0, 54.0));
    let to = Portal::new(
        [-384.0, 0.0, 64.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        (32.0, 54.0),
    );
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin))
        .init_asset::<Mesh>()
        .init_asset::<Image>()
        .init_resource::<Portals>()
        .insert_resource(Files(install.search))
        .add_plugins(BodyPlugin);
    {
        let mut portals = app.world_mut().resource_mut::<Portals>();
        portals.place(Colour::Blue, from);
        portals.place(Colour::Orange, to);
    }
    for z in [-10.0, -20.0, 100.0, -10.0] {
        let walker = Walker::at([0.0, 0.0, z]);
        app.insert_resource(VisualHistory::new(&walker).sample(&walker, 0.0));
        app.insert_resource(Player(walker));
        app.update();
        let world = app.world_mut();
        let visible = *world
            .query_filtered::<&Visibility, With<PortalGhost>>()
            .single(world)
            .unwrap();
        assert_eq!(visible != Visibility::Hidden, z < 0.0);
        let pieces: Vec<_> = world
            .query::<(
                &Mesh3d,
                &MeshMaterial3d<BodyMaterial>,
                &GlobalTransform,
                &RenderLayers,
            )>()
            .iter(world)
            .map(|(m, a, t, l)| (m.0.id(), a.0.clone(), t.to_matrix(), l.clone()))
            .collect();
        let source: Vec<_> = pieces
            .iter()
            .filter(|p| p.3 == RenderLayers::layer(BODY_LAYER))
            .collect();
        let ghosts: Vec<_> = pieces
            .iter()
            .filter(|p| p.3 == RenderLayers::layer(GHOST_LAYER))
            .collect();
        assert!(source.len() > 40);
        assert_eq!(source.len(), ghosts.len());
        let materials = world.resource::<Assets<BodyMaterial>>();
        for original in source {
            let ghost = ghosts.iter().find(|p| p.0 == original.0).unwrap();
            let original_plane = materials.get(&original.1).unwrap().extension.plane;
            let ghost_plane = materials.get(&ghost.1).unwrap().extension.plane;
            if z < 0.0 {
                assert!(ghost
                    .2
                    .abs_diff_eq(portalgun::views::through(&from, &to) * original.2, 0.001));
                assert!(original_plane.dot(to_bevy([0.0, 0.0, -1.0]).extend(1.0)) < 0.0);
                assert!(
                    ghost_plane
                        .dot(to_bevy(through_point(&from, &to, [0.0, 0.0, -1.0])).extend(1.0))
                        > 0.0
                );
            } else {
                assert_eq!(original_plane, Vec4::ZERO);
                assert_eq!(ghost_plane, Vec4::ZERO);
            }
        }
    }
}
