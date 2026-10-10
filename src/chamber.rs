use portal::keyvalues;
use portal::pair::Vec3;
use portal::placement::{RayHit, Surface, World};
use portal::vpk::SearchPath;

const WALL: &str = "concrete/concrete_modular_wall001a";
const FLOOR: &str = "concrete/concrete_modular_floor001a";
const CEILING: &str = "concrete/concrete_modular_ceiling001a";
const METAL: &str = "metal/metalwall048b";

pub struct Block {
    pub min: Vec3,
    pub max: Vec3,
    pub material: &'static str,
    pub surface: Surface,
    /// the material's $surfaceprop, default when it names none
    pub surfaceprop: String,
}

pub struct Face {
    pub normal: Vec3,
    pub corners: [Vec3; 4],
    /// in units along the texture axes, before the texture's scale
    pub uv: [[f32; 2]; 4],
}

pub struct Chamber {
    pub blocks: Vec<Block>,
}

/// vbsp turns %noportal into the surface flag the gun checks
pub fn noportal(vmt: &str) -> portal::Result<bool> {
    let root = keyvalues::parse(vmt)?;
    Ok(root
        .first()
        .and_then(|shader| shader.get("%noportal"))
        .is_some_and(|v| v.trim() == "1"))
}

/// what the material walks and sounds like, the default surface when it names none
pub fn surfaceprop(vmt: &str) -> portal::Result<String> {
    let root = keyvalues::parse(vmt)?;
    Ok(root
        .first()
        .and_then(|shader| shader.get("$surfaceprop"))
        .map_or("default", str::trim)
        .to_ascii_lowercase())
}

/// Interior 768 by 512 by 384 units, z up, a metal strip on the north wall and a ledge in the east.
const LAYOUT: [(Vec3, Vec3, &str); 10] = [
    ([-400.0, -272.0, -16.0], [400.0, 272.0, 0.0], FLOOR),
    ([-400.0, -272.0, 384.0], [400.0, 272.0, 400.0], CEILING),
    ([-400.0, -272.0, 0.0], [-384.0, 272.0, 384.0], WALL),
    ([384.0, -272.0, 0.0], [400.0, 272.0, 384.0], WALL),
    ([-384.0, -272.0, 0.0], [384.0, -256.0, 384.0], WALL),
    ([-384.0, 256.0, 0.0], [-64.0, 272.0, 384.0], WALL),
    ([-64.0, 256.0, 0.0], [64.0, 272.0, 384.0], METAL),
    ([64.0, 256.0, 0.0], [384.0, 272.0, 384.0], WALL),
    ([192.0, -256.0, 0.0], [384.0, -64.0, 128.0], WALL),
    ([-160.0, 96.0, 0.0], [-128.0, 128.0, 32.0], METAL),
];

impl Chamber {
    pub fn test_chamber(search: &SearchPath) -> portal::Result<Chamber> {
        let blocks = LAYOUT
            .iter()
            .map(|&(min, max, material)| {
                let path = format!("materials/{material}.vmt");
                let text = String::from_utf8_lossy(&search.read(&path)?).into_owned();
                let surface = if noportal(&text)? {
                    Surface::Rejects
                } else {
                    Surface::Portalable
                };
                Ok(Block {
                    min,
                    max,
                    material,
                    surface,
                    surfaceprop: surfaceprop(&text)?,
                })
            })
            .collect::<portal::Result<_>>()?;
        Ok(Chamber { blocks })
    }
}

/// Hammer's world alignment, the texture axes of a face by its dominant axis
fn world_aligned(axis: usize, p: Vec3) -> [f32; 2] {
    match axis {
        0 => [p[1], -p[2]],
        1 => [p[0], -p[2]],
        _ => [p[0], -p[1]],
    }
}

impl Block {
    pub fn faces(&self) -> Vec<Face> {
        let mut faces = Vec::new();
        for i in 0..3 {
            let (j, k) = ((i + 1) % 3, (i + 2) % 3);
            for (side, plane) in [(1.0, self.max[i]), (-1.0, self.min[i])] {
                let (u, v) = if side > 0.0 { (j, k) } else { (k, j) };
                let corners = [(0, 0), (1, 0), (1, 1), (0, 1)].map(|(a, b)| {
                    let mut p = [0.0; 3];
                    p[i] = plane;
                    p[u] = if a == 0 { self.min[u] } else { self.max[u] };
                    p[v] = if b == 0 { self.min[v] } else { self.max[v] };
                    p
                });
                let mut normal = [0.0; 3];
                normal[i] = side;
                faces.push(Face {
                    normal,
                    corners,
                    uv: corners.map(|p| world_aligned(i, p)),
                });
            }
        }
        faces
    }

    /// slab test, a ray starting inside the block does not hit it
    fn entry(&self, from: Vec3, dir: Vec3) -> Option<(f32, Vec3)> {
        let (mut near, mut far, mut axis) = (f32::NEG_INFINITY, f32::INFINITY, 0);
        for i in 0..3 {
            if dir[i] == 0.0 {
                if from[i] < self.min[i] || from[i] > self.max[i] {
                    return None;
                }
                continue;
            }
            let a = (self.min[i] - from[i]) / dir[i];
            let b = (self.max[i] - from[i]) / dir[i];
            let (lo, hi) = (a.min(b), a.max(b));
            if lo > near {
                (near, axis) = (lo, i);
            }
            far = far.min(hi);
        }
        if near > far || near < 0.0 {
            return None;
        }
        let mut normal = [0.0; 3];
        normal[axis] = -dir[axis].signum();
        Some((near, normal))
    }
}

impl World for Chamber {
    fn cast(&self, from: Vec3, dir: Vec3, max: f32) -> Option<RayHit> {
        self.blocks
            .iter()
            .filter_map(|b| b.entry(from, dir).map(|(d, n)| (d, n, b.surface)))
            .filter(|&(d, ..)| d <= max)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(distance, normal, surface)| RayHit {
                distance,
                normal,
                surface,
            })
    }
}
