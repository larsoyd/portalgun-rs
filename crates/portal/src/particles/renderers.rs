//! The geometry of Portal's three particle renderers, as client.so builds it on the CPU.

use super::collection::{Collection, Particle, Vec3};
use super::sheet::{Sample, Sheet};
use crate::pcf::Function;

/// float magic that rounds to nearest and leaves the integer in the low mantissa bits (0x7fdb78)
const MAGIC: f32 = 8_388_608.0;
const NORM_EPS: f32 = 1e-10;

/// The view the geometry faces, in the particles' own units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    pub pos: Vec3,
    pub right: Vec3,
    pub up: Vec3,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vertex {
    pub pos: Vec3,
    pub uv: [f32; 2],
    /// the packed r, g, b, a bytes over 255, faded further on cards
    pub color: [f32; 4],
}

/// What the material brings to the geometry: the texture's height for rope mapping, its
/// sprite sheet, and SpriteCard's sizes when it is one.
#[derive(Debug, Clone, Copy, Default)]
pub struct Look<'a> {
    pub height: f32,
    pub sheet: Option<&'a Sheet>,
    pub card: Option<Card>,
}

/// SpriteCard's size limits and fades, as fractions of the distance to the eye.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Card {
    pub min_size: f32,
    pub max_size: f32,
    pub start_fade: f32,
    pub end_fade: f32,
    pub max_distance: f32,
    pub far_interval: f32,
}

impl Default for Card {
    /// the values SpriteCard's InitParams gives a material that leaves them out
    fn default() -> Self {
        Card {
            min_size: 0.0,
            max_size: 20.0,
            start_fade: 10.0,
            end_fade: 20.0,
            max_distance: 100_000.0,
            far_interval: 400.0,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Renderer {
    Sprites {
        rate: f32,
        fit_lifetime: bool,
        fps: bool,
    },
    Trail {
        rate: f32,
        fade_in: f32,
        max_length: f32,
        min_length: f32,
    },
    Rope {
        subdivisions: usize,
        texel_size: f32,
        scroll: f32,
    },
}

/// The low byte of x rounded to nearest, wrapping above 255 as the float trick does.
pub fn byte(x: f32) -> u8 {
    ((x + MAGIC).to_bits() & 0xff) as u8
}

impl Renderer {
    /// Only screen aligned sprites without a visibility proxy are ported, as Portal's files use.
    pub fn from_function(f: &Function) -> Option<Renderer> {
        let visible = f.int_or("Visibility Proxy Input Control Point Number", -1) < 0;
        Some(match f.name.to_ascii_lowercase().as_str() {
            "render_animated_sprites" if visible && f.int_or("orientation_type", 0) == 0 => {
                Renderer::Sprites {
                    rate: f.float_or("animation rate", 0.1),
                    fit_lifetime: f.bool_or("animation_fit_lifetime", false),
                    fps: f.bool_or("use animation rate as FPS", false),
                }
            }
            "render_sprite_trail" if visible => Renderer::Trail {
                rate: f.float_or("animation rate", 0.1),
                fade_in: f.float_or("length fade in time", 0.0),
                max_length: f.float_or("max length", 2000.0),
                min_length: f.float_or("min length", 0.0),
            },
            "render_rope" => {
                let texel = f.float_or("texel_size", 4.0);
                Renderer::Rope {
                    subdivisions: f.int_or("subdivision_count", 3).max(1) as usize,
                    texel_size: if texel <= 0.0 { 1.0 } else { texel },
                    scroll: f.float_or("texture_scroll_rate", 0.0),
                }
            }
            _ => return None,
        })
    }

    /// Triangles for the collection, far particles first. Trails and ropes draw nothing with
    /// a SpriteCard material, as their Render skips it.
    pub fn build(&self, c: &Collection, cam: &Camera, look: &Look) -> Mesh {
        let mut mesh = Mesh::default();
        if look.card.is_some() && !matches!(self, Renderer::Sprites { .. }) {
            return mesh;
        }
        match *self {
            Renderer::Sprites {
                rate,
                fit_lifetime,
                fps,
            } => {
                for (p, radius, alpha) in render_list(c, cam) {
                    let seq = sequence(p);
                    let span = look.sheet.map_or(1.0, |s| s.span(seq));
                    let rate = rate * 1024.0 / if fps { span } else { 1.0 };
                    if let Some(card) = &look.card {
                        let frame = frame(look.sheet, p, c.cur_time, rate);
                        sprite_card(&mut mesh, p, radius, alpha, cam, card, frame);
                        continue;
                    }
                    let rate = match fit_lifetime {
                        true if p.life > 0.0 => 1024.0 / p.life,
                        true => 0.0,
                        false => rate,
                    };
                    let frame = frame(look.sheet, p, c.cur_time, rate);
                    sprite(&mut mesh, p, radius, alpha, cam, frame.a);
                }
            }
            Renderer::Trail {
                rate,
                fade_in,
                max_length,
                min_length,
                ..
            } => {
                let inv_dt = if c.dt == 0.0 { 1.0 } else { 1.0 / c.dt };
                for (p, radius, alpha) in render_list(c, cam) {
                    let age = c.cur_time - p.creation;
                    let fade = if age < fade_in { age / fade_in } else { 1.0 };
                    let d0 = sub(p.prev, p.xyz);
                    let l2 = dot(d0, d0) + NORM_EPS;
                    let len = inv_dt * p.trail * fade * l2.sqrt();
                    if len > 0.0 {
                        let len = min_length.max(max_length.min(len));
                        let d = scale(d0, len / l2.sqrt());
                        let rect = frame(look.sheet, p, c.cur_time, rate * 1024.0).a;
                        trail(&mut mesh, p, d, len.min(radius), alpha, cam, rect);
                    }
                }
            }
            Renderer::Rope {
                subdivisions,
                texel_size,
                scroll,
            } => rope(
                &mut mesh,
                c,
                cam,
                subdivisions,
                scroll * c.cur_time,
                1.0 / (look.height * texel_size),
            ),
        }
        mesh
    }
}

/// The render list (0x6878b0): alpha as a byte from ALPHA times ALPHA2, sorted far to near.
fn render_list<'a>(c: &'a Collection, cam: &Camera) -> Vec<(&'a Particle, f32, u8)> {
    let mut list: Vec<(f32, &Particle)> = (c.particles.iter())
        .map(|p| {
            let d = sub(cam.pos, p.xyz);
            (dot(d, d), p)
        })
        .collect();
    list.sort_by(|a, b| b.0.total_cmp(&a.0));
    list.into_iter()
        .map(|(_, p)| (p, p.radius, byte(p.alpha * p.alpha2 * 255.0)))
        .filter(|(_, _, a)| *a != 0)
        .collect()
}

fn color(p: &Particle, alpha: u8) -> [f32; 4] {
    let [r, g, b] = p.tint.map(|v| byte(v * 255.0));
    [r, g, b, alpha].map(|b| b as f32 / 255.0)
}

/// (int)SEQUENCE_NUMBER, kept to the sheet's 64 slots
fn sequence(p: &Particle) -> usize {
    (p.sequence as i32).clamp(0, 63) as usize
}

/// The sheet sample for the particle's age, rate being samples a second.
fn frame(sheet: Option<&Sheet>, p: &Particle, now: f32, rate: f32) -> Sample {
    let Some(sheet) = sheet else {
        let whole = [0.0, 0.0, 1.0, 1.0];
        return Sample {
            a: whole,
            b: whole,
            blend: 1.0,
        };
    };
    let seq = sequence(p);
    let i = ((now - p.creation) * rate) as u32 as usize;
    sheet.sample(
        seq,
        if sheet.clamps(seq) {
            i.min(1023)
        } else {
            i & 1023
        },
    )
}

fn quad(mesh: &mut Mesh, corners: [(Vec3, [f32; 2]); 4], color: [f32; 4]) {
    let base = mesh.vertices.len() as u32;
    for (pos, uv) in corners {
        mesh.vertices.push(Vertex { pos, uv, color });
    }
    mesh.indices.extend([0, 1, 2, 0, 2, 3].map(|i| base + i));
}

/// Orientation 0 (0x6758d0): a 2r square turned by ROTATION in the view plane.
fn sprite(mesh: &mut Mesh, p: &Particle, r: f32, alpha: u8, cam: &Camera, rect: [f32; 4]) {
    let [l, t, rt, b] = rect;
    let (s, c) = p.rotation.sin_cos();
    let at = |x: f32, y: f32| add(p.xyz, add(scale(cam.right, x * r), scale(cam.up, y * r)));
    quad(
        mesh,
        [
            (at(s - c, -(s + c)), [l, b]),
            (at(-(s + c), c - s), [l, t]),
            (at(c - s, s + c), [rt, t]),
            (at(s + c, s - c), [rt, b]),
        ],
        color(p, alpha),
    );
}

/// A streak from the particle back along its motion (0x67a660), as wide as its radius allows.
fn trail(mesh: &mut Mesh, p: &Particle, d: Vec3, w: f32, alpha: u8, cam: &Camera, rect: [f32; 4]) {
    let [l, t, r, b] = rect;
    let side = normalized(cross(sub(p.xyz, cam.pos), d));
    let v0 = add(p.xyz, scale(side, 0.5 * w));
    let v1 = add(p.xyz, scale(side, -0.5 * w));
    quad(
        mesh,
        [
            (v0, [l, b]),
            (v1, [r, b]),
            (add(v1, d), [r, t]),
            (add(v0, d), [l, t]),
        ],
        color(p, alpha),
    );
}

/// spritecard_vs20 orientation 0: sized and faded by distance, the quad expanded in view space.
/// Vertex colour stays in stored values, so the linear fade reaches it raised to one over 2.2.
fn sprite_card(
    mesh: &mut Mesh,
    p: &Particle,
    radius: f32,
    alpha: u8,
    cam: &Camera,
    card: &Card,
    frame: Sample,
) {
    let d = sub(p.xyz, cam.pos);
    let dist = dot(d, d).sqrt();
    let rad = (card.min_size * dist).max(radius);
    let (start, end) = (card.start_fade * dist, card.end_fade * dist);
    let mut fade = if rad > end {
        0.0
    } else if rad > start {
        1.0 - (rad - start) / (end - start)
    } else {
        1.0
    };
    let near = (card.max_distance - card.far_interval).max(1.0);
    let far = ((dist - near) * (1.0 / (card.max_distance - near))).clamp(0.0, 1.0);
    fade *= 1.0 - far;
    let rad = if rad > end || far >= 1.0 { 0.0 } else { rad };
    let size = (card.max_size * dist).min(rad);
    let mut color = color(p, alpha);
    for (k, v) in color.iter_mut().enumerate() {
        *v *= if k < 3 { fade.powf(1.0 / 2.2) } else { fade };
    }
    let (s, c) = p.rotation.sin_cos();
    let [l, t, r, b] = frame.a;
    let corner = |cx: f32, cy: f32| {
        let (kx, ky) = (2.0 * cx - 1.0, 2.0 * cy - 1.0);
        let (a, bb) = (c * kx + s * ky, c * ky - s * kx);
        let (ys, yc) = p.yaw.sin_cos();
        // view z is taken as right cross up, toward the eye
        let back = cross(cam.right, cam.up);
        let off = add(
            add(scale(cam.right, -yc * a), scale(cam.up, bb)),
            scale(back, ys * a),
        );
        (
            add(p.xyz, scale(off, size)),
            [r + cx * (l - r), b + cy * (t - b)],
        )
    };
    quad(
        mesh,
        [
            corner(0.0, 0.0),
            corner(1.0, 0.0),
            corner(1.0, 1.0),
            corner(0.0, 1.0),
        ],
        color,
    );
}

#[derive(Clone, Copy)]
struct Seg {
    pos: Vec3,
    rgb: Vec3,
    alpha: f32,
    width: f32,
    tex: f32,
}

/// CBeamSegDraw: each segment's edge faces the camera, averaged with the one before.
struct Beam<'m> {
    mesh: &'m mut Mesh,
    cam: Vec3,
    total: usize,
    drawn: usize,
    seg: Option<Seg>,
    last: Vec3,
}

impl Beam<'_> {
    fn next(&mut self, s: Seg) {
        if let Some(cur) = self.seg {
            let perp = normalized(cross(sub(cur.pos, s.pos), sub(cur.pos, self.cam)));
            let n = if self.drawn > 1 {
                normalized(scale(add(perp, self.last), 0.5))
            } else {
                perp
            };
            self.specify(cur, n);
            self.last = perp;
        }
        self.seg = Some(s);
        self.drawn += 1;
        if self.drawn == self.total {
            self.specify(s, self.last);
        }
    }

    fn specify(&mut self, s: Seg, n: Vec3) {
        let [r, g, b] = s.rgb.map(|v| byte(v * 255.0));
        let color = [r, g, b, byte(s.alpha * 255.0)].map(|b| b as f32 / 255.0);
        let half = scale(n, 0.5 * s.width);
        let base = self.mesh.vertices.len() as u32;
        self.mesh.vertices.push(Vertex {
            pos: add(s.pos, half),
            uv: [0.0, s.tex],
            color,
        });
        self.mesh.vertices.push(Vertex {
            pos: sub(s.pos, half),
            uv: [1.0, s.tex],
            color,
        });
        if base >= 2 {
            let b0 = base - 2;
            self.mesh
                .indices
                .extend([b0, b0 + 1, b0 + 2, b0 + 1, b0 + 3, b0 + 2]);
        }
    }
}

fn catmull_rom(p: [Vec3; 4], t: f32) -> Vec3 {
    let (t2, t3) = (t * t, t * t * t);
    [0, 1, 2].map(|k| {
        let [a, b, c, d] = p.map(|v| v[k]);
        0.5 * (2.0 * b
            + (c - a) * t
            + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2
            + (3.0 * b - a - 3.0 * c + d) * t3)
    })
}

/// One beam through the particles in order (0x672e60), smoothed between them.
fn rope(mesh: &mut Mesh, c: &Collection, cam: &Camera, subdiv: usize, scroll: f32, per_unit: f32) {
    let ps = &c.particles;
    if ps.len() < 2 {
        return;
    }
    let seg = |p: &Particle, tex: f32| Seg {
        pos: p.xyz,
        rgb: p.tint,
        alpha: p.alpha,
        width: p.radius,
        tex,
    };
    let mut beam = Beam {
        mesh,
        cam: cam.pos,
        total: (ps.len() - 1) * subdiv + 1,
        drawn: 0,
        seg: None,
        last: [0.0; 3],
    };
    let mut length = 0.0;
    let mut prev = seg(&ps[0], scroll * per_unit);
    beam.next(prev);
    for i in 1..ps.len() {
        let d = sub(ps[i].xyz, ps[i - 1].xyz);
        length += dot(d, d).sqrt();
        let cur = seg(&ps[i], (length + scroll) * per_unit);
        let points = [
            ps[i.saturating_sub(2)].xyz,
            ps[i - 1].xyz,
            ps[i].xyz,
            ps[(i + 1).min(ps.len() - 1)].xyz,
        ];
        for j in 1..subdiv {
            let t = j as f32 / subdiv as f32;
            let lerp = |a: f32, b: f32| a + (b - a) * t;
            beam.next(Seg {
                pos: catmull_rom(points, t),
                rgb: [0, 1, 2].map(|k| lerp(prev.rgb[k], cur.rgb[k]).clamp(0.0, 1.0)),
                alpha: lerp(prev.alpha, cur.alpha).clamp(0.0, 1.0),
                width: lerp(prev.width, cur.width),
                tex: lerp(prev.tex, cur.tex),
            });
        }
        beam.next(cur);
        prev = cur;
    }
}

fn add(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scale(a: Vec3, s: f32) -> Vec3 {
    a.map(|v| v * s)
}

fn dot(a: Vec3, b: Vec3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn normalized(v: Vec3) -> Vec3 {
    scale(v, 1.0 / (dot(v, v) + NORM_EPS).sqrt())
}
