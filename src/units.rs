use bevy::math::Vec3;
use portal::pair::Vec3 as Source;

/// Source calls one unit an inch. The only place units or axes change.
pub const METRES_PER_UNIT: f32 = 0.0254;

/// Source forward x, left y, up z onto Bevy's right x, up y, back z.
pub fn to_bevy_dir(v: Source) -> Vec3 {
    Vec3::new(-v[1], v[2], -v[0])
}

pub fn to_bevy(p: Source) -> Vec3 {
    to_bevy_dir(p) * METRES_PER_UNIT
}

pub fn from_bevy_dir(v: Vec3) -> Source {
    [-v.z, -v.x, v.y]
}

pub fn from_bevy(p: Vec3) -> Source {
    from_bevy_dir(p / METRES_PER_UNIT)
}
