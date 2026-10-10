use crate::pair::{Portal, Vec3};
use crate::placement::{HALF_HEIGHT, HALF_WIDTH};

/// How far in front of the surface the face is drawn (DrawSimplePortalMesh's default).
const LIFT: f32 = 0.25;
const OPEN_RATE: f32 = 2.0;
const STATIC_FALL: f32 = 1.0;
/// a camera this close in front drops the static (PORTAL_HALF_HEIGHT)
const NEAR: f32 = 54.0;

/// What stage 2 of PortalRefract writes: the ramp coordinate and its alpha, up to 1.5.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Overlay {
    pub u: f32,
    pub alpha: f32,
}

/// What stage 0 reads: where in the copy of the screen, and how much it darkens it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Refraction {
    pub at: [f32; 2],
    pub shade: f32,
}

/// A portal's m_fOpenAmount and m_fStaticAmount.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Fade {
    pub open: f32,
    pub statics: f32,
}

impl Fade {
    /// C_Prop_Portal::ClientThink.
    pub fn think(&mut self, dt: f32) {
        if self.statics > 0.0 {
            self.statics = (self.statics - dt * STATIC_FALL).max(0.0);
        }
        if self.open < 1.0 {
            self.open = (self.open + dt * OPEN_RATE).min(1.0);
        }
    }

    /// The portal opened somewhere new: it opens again and its partner turns to static.
    pub fn moved(&mut self, partner: Option<&mut Fade>) {
        self.open = 0.0;
        if let Some(p) = partner {
            p.statics = 1.0;
        }
    }
}

/// A corner of the face as (forward, right, up) from the portal's origin, by texture coordinate.
pub fn corner(uv: [f32; 2]) -> Vec3 {
    [
        LIFT,
        HALF_WIDTH * (1.0 - 2.0 * uv[0]),
        HALF_HEIGHT * (1.0 - 2.0 * uv[1]),
    ]
}

fn saturate(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

fn ease(x: f32) -> f32 {
    (x * -2.0 + 3.0) * (x * x)
}

/// t0 of portal_refract_vs20.
pub fn ellipse(uv: [f32; 2]) -> [f32; 2] {
    uv.map(|v| v * 1.075 - 0.0375)
}

pub fn radius(t0: [f32; 2]) -> f32 {
    let p = t0.map(|v| 2.0 * v - 1.0);
    (p[0] * p[0] + p[1] * p[1]).sqrt()
}

pub fn opening(open: f32) -> f32 {
    ease(saturate(open))
}

/// Stage 1, the stencil hole.
pub fn in_hole(open: f32, t0: [f32; 2]) -> bool {
    let s = opening(open);
    s * s - radius(t0) >= 0.0
}

pub fn shader_time(t: f32) -> f32 {
    t - (t * 0.001).floor() * 1000.0
}

/// t5 of portal_refract_vs20, time already wrapped.
pub fn noise_coords(uv: [f32; 2], open: f32, time: f32) -> [f32; 4] {
    let k = 1.0 / (open + 0.001).clamp(0.0, 1.0);
    let q = uv.map(|v| ((v - 0.5) * k + 0.5) * 0.3);
    [q[0] + 0.0275 * time, q[1], q[0] - 0.0275 * time, q[1]]
}

/// Stage 2, the rim and static, with keep one minus the static amount.
pub fn overlay(
    open: f32,
    keep: f32,
    t0: [f32; 2],
    t5: [f32; 4],
    mut noise: impl FnMut([f32; 2]) -> [f32; 2],
) -> Overlay {
    let n0 = noise([t5[0], t5[1]]);
    let s = opening(open);
    let s2 = s * s;
    let d = radius(t0);
    let inside = if s2 - d >= 0.0 { 1.0 } else { 0.0 };
    let lo = s2 - 0.3;
    let edge_in = saturate((d - lo) * (1.0 / (s2 - lo)));
    let edge_out = 1.0 - saturate((d - s2) * 13.333_333);
    let edge = if inside == 1.0 { edge_in } else { edge_out };
    let keep = saturate(keep);
    let m = edge * saturate(s * 2.5).max(1.0 - keep);
    let n1 = noise([t5[3] - n0[0] * 0.02, t5[2] - n0[1] * 0.02]);
    let n2 = noise([t5[0] - n1[0] * 0.02, t5[1] - n1[1] * 0.02]);
    let h = (n1[1] + n2[1]) * 0.5;
    let x = saturate((h - (m - 0.875)) * (1.0 / ((m + 0.875) - (m - 0.875))));
    let sx = 1.0 - ease(x);
    let y = saturate(keep * (1.0 / h));
    let sy = 1.0 - ease(y);
    let alpha = saturate(m * sx + inside * sy) * 1.5;
    let u = alpha * sx.sqrt() * (t0[1].abs().powf(1.5) * 0.8 + 0.2);
    Overlay { u, alpha }
}

/// The written alpha: the target clamps it and the alpha test drops 1/255 and under.
pub fn coverage(alpha: f32) -> f32 {
    if alpha > 1.0 / 255.0 {
        alpha.min(1.0)
    } else {
        0.0
    }
}

pub fn srgb_decode(x: f32) -> f32 {
    if x <= 0.04045 {
        x / 12.92
    } else {
        ((x + 0.055) / 1.055).powf(2.4)
    }
}

pub fn srgb_encode(x: f32) -> f32 {
    if x <= 0.003_130_8 {
        x * 12.92
    } else {
        1.055 * x.powf(1.0 / 2.4) - 0.055
    }
}

/// The ramp's linear colour scaled, then written to an sRGB target, as a stored value.
pub fn overlay_rgb(ramp: [f32; 3], scale: f32) -> [f32; 3] {
    ramp.map(|c| srgb_encode(saturate(c * scale)))
}

/// A clip position as the screen coordinate the shaders read the screen copy at.
pub fn screen_uv(clip: [f32; 4]) -> [f32; 2] {
    let w = 1.0 / clip[3];
    [(clip[0] + clip[3]) * 0.5 * w, (clip[3] - clip[1]) * 0.5 * w]
}

/// Stage 0, the refraction while opening. along_t and along_b are how far the screen coordinate
/// moves for one Source unit along the face's tangent and binormal. None outside the ellipse.
pub fn refraction(
    open: f32,
    t0: [f32; 2],
    uv: [f32; 2],
    along_t: [f32; 2],
    along_b: [f32; 2],
) -> Option<Refraction> {
    let p = t0.map(|v| 2.0 * v - 1.0);
    let d = radius(t0);
    let s = opening(open);
    let f = 1.0 - saturate(d).powf(64.0);
    let a = saturate((d + s * -1.5) * (1.0 / (s * -0.5)));
    let dir = p.map(|v| v * (1.0 / d));
    let off = dir.map(|v| (s * s) * -v * f * ease(a) * 32.0);
    let at = [0, 1].map(|i| uv[i] + off[0] * along_t[i] - off[1] * along_b[i]);
    let lo = s * s - 0.01;
    let b = saturate((d - lo) * (1.0 / (saturate(s + s) - lo)));
    let shade = (2.0 * b - 1.0).abs() * 0.15 + 0.85;
    (1.0 - d >= 0.0).then_some(Refraction { at, shade })
}

/// Whether the camera is close enough in front for DrawRenderFixMesh to clear the static.
pub fn near(portal: &Portal, eye: Vec3) -> bool {
    let l = portal.local(eye);
    l[0] >= -1.0 && l[0] * l[0] + l[1] * l[1] + l[2] * l[2] < NEAR * NEAR
}

/// CPortalStaticProxy::ComputeStaticAmount, without the depth doubler.
pub fn static_amount(statics: f32, linked: bool, last_view: bool) -> f32 {
    if !linked || last_view {
        1.0
    } else {
        statics
    }
}
