// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use bevy::prelude::*;
use portal::pair::Portal;
use portal::view::Plane;

use crate::units::{from_bevy_dir, to_bevy, to_bevy_dir, METRES_PER_UNIT};

/// A portal's frame in Bevy space, columns forward, right, up and origin.
fn frame(p: &Portal) -> Mat4 {
    Mat4::from_cols(
        to_bevy_dir(p.forward).extend(0.0),
        to_bevy_dir(p.right).extend(0.0),
        to_bevy_dir(p.up).extend(0.0),
        to_bevy(p.origin).extend(1.0),
    )
}

/// Translated from 0x27fad0 (decompiled, client.so). In through one portal, out of the other,
/// turned half way round the portal's own up.
pub fn through(from: &Portal, to: &Portal) -> Mat4 {
    frame(to) * Mat4::from_diagonal(Vec4::new(-1.0, -1.0, 1.0, 1.0)) * frame(from).inverse()
}

/// A Source clip plane as Bevy's near_clip_plane for a camera, normal pointing away from it.
pub fn view_clip_plane(plane: Plane, camera: &Transform) -> Vec4 {
    let n = to_bevy_dir(plane.normal);
    let on = n * plane.dist * METRES_PER_UNIT;
    let view_from_world = camera.to_matrix().inverse();
    let nv = view_from_world.transform_vector3(n).normalize();
    // An oblique plane crossing the eye makes the projection singular or clips the far side of the view.
    let distance = nv.dot(view_from_world.transform_point3(on));
    nv.extend(-distance.max(0.01 * METRES_PER_UNIT))
}

/// The side and near planes of a view in Source units. The far plane is at infinity.
pub fn frustum_planes(clip_from_world: &Mat4) -> Vec<Plane> {
    let [r0, r1, r2, r3] = [0, 1, 2, 3].map(|i| clip_from_world.row(i));
    [r3 + r0, r3 - r0, r3 + r1, r3 - r1, r3 - r2]
        .into_iter()
        .map(|h| {
            let len = h.truncate().length();
            Plane {
                normal: from_bevy_dir(h.truncate() / len),
                dist: -h.w / len / METRES_PER_UNIT,
            }
        })
        .collect()
}
