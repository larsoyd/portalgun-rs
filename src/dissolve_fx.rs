// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use portal::entity::dissolve::{emit, near_fade, site, step, Draw, Spark, Sprite};
use portal::mdl::{Hitbox, Mdl};
use portal::pair::Vec3 as Source;
use portal::particles::renderers::{Mesh as Built, Vertex};

use crate::effects::ParticleView;
use crate::objects::Objects;
use crate::particle_material::{MaterialSource, ParticleMaterial};
use crate::random::Xorshift;
use crate::sound::Files;
use crate::units::{from_bevy, from_bevy_dir};

const MODEL: &str = "models/props/metal_box";
const MAIN_VIEW: usize = 32;

struct Stream(Xorshift);

impl Draw for Stream {
    fn float(&mut self, lo: f32, hi: f32) -> f32 {
        self.0.range(lo, hi)
    }
    fn int(&mut self, lo: i32, hi: i32) -> i32 {
        lo + ((self.0.unit() * (hi - lo + 1) as f32) as i32).min(hi - lo)
    }
}

#[derive(Resource)]
struct Sparks {
    live: Vec<Spark>,
    made: usize,
    stream: Stream,
    bones: Vec<Mat4>,
    hitboxes: Vec<Hitbox>,
    materials: [Handle<ParticleMaterial>; 2],
    /// made on the first frame with something to draw, bevy cannot upload an empty mesh
    draws: [Option<(Entity, Handle<Mesh>)>; 2],
}

/// each bone of the model's first frame in model space
fn bone_matrices(mdl: &Mdl) -> Result<Vec<Mat4>, String> {
    let anim = mdl.sequences.first().map_or(0, |s| s.anim);
    let pose = portal::anim::sample(mdl, anim, 0.0).map_err(|e| e.to_string())?;
    let mut out: Vec<Mat4> = Vec::with_capacity(pose.len());
    for (i, p) in pose.iter().enumerate() {
        let local = Mat4::from_rotation_translation(Quat::from_array(p.quat), Vec3::from(p.pos));
        out.push(
            mdl.bones[i]
                .parent
                .map_or(local, |parent| out[parent] * local),
        );
    }
    Ok(out)
}

/// The hitboxes of the dissolving model in the world, each with its two screen axes.
pub fn sites(
    position: Source,
    rotation: [Source; 3],
    bones: &[Mat4],
    hitboxes: &[Hitbox],
    eye: Source,
) -> Vec<(Source, [Source; 2])> {
    let entity = Mat4::from_cols(
        Vec3::from(rotation[0]).extend(0.0),
        Vec3::from(rotation[1]).extend(0.0),
        Vec3::from(rotation[2]).extend(0.0),
        Vec3::from(position).extend(1.0),
    );
    hitboxes
        .iter()
        .map(|h| {
            let world = entity * bones[h.bone];
            let (min, max) = (Vec3::from(h.min), Vec3::from(h.max));
            let centre = world.transform_point3((min + max) * 0.5).to_array();
            let axes = [world.x_axis, world.y_axis, world.z_axis].map(|a| a.truncate().to_array());
            (centre, site(centre, axes, (max - min).to_array(), eye))
        })
        .collect()
}

fn setup(
    mut commands: Commands,
    files: Res<Files>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<ParticleMaterial>>,
    mut exit: MessageWriter<AppExit>,
) {
    let result = (|| -> Result<Sparks, String> {
        let (mdl, _) = crate::viewmodel::read_model(&files, MODEL)?;
        let bones = bone_matrices(&mdl)?;
        let mut load = |name: &str| -> Result<Handle<ParticleMaterial>, String> {
            let source = MaterialSource::read(&files, name)?
                .ok_or_else(|| format!("{name} is not a particle material"))?;
            Ok(source.upload(&mut images, &mut materials))
        };
        Ok(Sparks {
            live: Vec::new(),
            made: 0,
            stream: Stream(Xorshift::default()),
            materials: [load("effects/spark")?, load("effects/combinemuzzle2")?],
            draws: [None, None],
            hitboxes: mdl.hitboxes.clone(),
            bones,
        })
    })();
    match result {
        Ok(sparks) => {
            info!("[dissolve] sites={}", sparks.hitboxes.len());
            commands.insert_resource(sparks);
        }
        Err(reason) => {
            error!("[dissolve] refused: {reason}");
            exit.write(AppExit::error());
        }
    }
}

/// the quad 0x5346d0 builds, a square of half side size turned by roll in radians
fn quad(s: &Spark, right: Vec3, up: Vec3, alpha: f32, out: &mut Built) {
    let (sin, cos) = s.roll.sin_cos();
    let size = s.size();
    let (a, b) = ((sin + cos) * size, (sin - cos) * size);
    let corners = [
        (b, -a, [0.0, 1.0]),
        (-a, -b, [0.0, 0.0]),
        (-b, a, [1.0, 0.0]),
        (a, b, [1.0, 1.0]),
    ];
    let base = out.vertices.len() as u32;
    let colour = [1.0, 1.0, 1.0, (alpha * 254.9).round() / 255.0];
    for (x, y, uv) in corners {
        out.vertices.push(Vertex {
            pos: (Vec3::from(s.pos) + right * x + up * y).to_array(),
            uv,
            uv_next: uv,
            blend: 0.0,
            color: colour,
        });
    }
    out.indices
        .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
}

fn run(
    mut commands: Commands,
    time: Res<Time>,
    objects: Res<Objects>,
    views: Query<(&GlobalTransform, &ParticleView)>,
    sparks: Option<ResMut<Sparks>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut visibility: Query<&mut Visibility>,
) {
    let (Some(mut sparks), dt) = (sparks, time.delta_secs()) else {
        return;
    };
    let Some((camera, _)) = views.iter().find(|(_, v)| v.0 == MAIN_VIEW) else {
        return;
    };
    if dt == 0.0 {
        return;
    }
    let eye = from_bevy(camera.translation());
    let forward = Vec3::from(from_bevy_dir(*camera.forward()));
    let (right, up) = (
        Vec3::from(from_bevy_dir(*camera.right())),
        Vec3::from(from_bevy_dir(*camera.up())),
    );
    let sparks = &mut *sparks;
    step(&mut sparks.live, dt);
    match &objects.dissolving {
        Some(d) => {
            let at = sites(
                d.cube.position,
                d.cube.rotation,
                &sparks.bones,
                &sparks.hitboxes,
                eye,
            );
            let new = emit(d.age, &at, forward.to_array(), &mut sparks.stream);
            sparks.made += new.len();
            sparks.live.extend(new);
        }
        None if sparks.made > 0 => {
            info!("[dissolve] sparks made={}", sparks.made);
            sparks.made = 0;
        }
        None => {}
    }
    for (k, kind) in [Sprite::Spark, Sprite::Muzzle].into_iter().enumerate() {
        let mut built = Built::default();
        for s in sparks.live.iter().filter(|s| s.sprite == kind) {
            let depth = (Vec3::from(s.pos) - Vec3::from(eye)).dot(forward);
            let alpha = near_fade(s.alpha(), depth);
            if alpha > 0.0 {
                quad(s, right, up, alpha, &mut built);
            }
        }
        let shown = !built.indices.is_empty();
        match (&sparks.draws[k], shown) {
            (Some((_, handle)), true) => {
                if let Some(mut mesh) = meshes.get_mut(handle) {
                    *mesh = crate::effects::particle_mesh(built);
                }
            }
            (None, true) => {
                let handle = meshes.add(crate::effects::particle_mesh(built));
                let entity = commands
                    .spawn((
                        Mesh3d(handle.clone()),
                        MeshMaterial3d(sparks.materials[k].clone()),
                        Transform::IDENTITY,
                        RenderLayers::layer(MAIN_VIEW),
                        NoFrustumCulling,
                    ))
                    .id();
                sparks.draws[k] = Some((entity, handle));
            }
            _ => {}
        }
        if let Some((entity, _)) = sparks.draws[k] {
            if let Ok(mut v) = visibility.get_mut(entity) {
                *v = if shown {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                };
            }
        }
    }
}

pub struct DissolveFxPlugin;

impl Plugin for DissolveFxPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup)
            .add_systems(PostUpdate, run.before(TransformSystems::Propagate));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_box_site_sits_on_the_box_and_spans_its_width() {
        let dir = portal::install::root(
            None,
            None,
            std::path::Path::new(&std::env::var("HOME").unwrap()),
        );
        let files = portal::install::Install::open(&dir).unwrap().search;
        let (mdl, _) = crate::viewmodel::read_model(&files, MODEL).unwrap();
        let bones = bone_matrices(&mdl).unwrap();
        let identity = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        let at = sites(
            [10.0, 20.0, 30.0],
            identity,
            &bones,
            &mdl.hitboxes,
            [500.0, 20.0, 30.0],
        );
        assert_eq!(at.len(), 1);
        let (centre, [a, b]) = at[0];
        assert!(
            (0..3).all(|i| (centre[i] - [10.0, 20.0, 30.0][i]).abs() < 0.01),
            "{centre:?}"
        );
        let length = |v: Source| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        assert!(
            (length(a) - 39.59).abs() < 0.01 && (length(b) - 39.59).abs() < 0.01,
            "{a:?} {b:?}"
        );
        assert!(
            a[0].abs() < 1e-3 && b[0].abs() < 1e-3,
            "axes face the eye {a:?} {b:?}"
        );
    }
}
