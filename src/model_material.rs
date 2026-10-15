// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

#[derive(Debug, Clone, Default)]
pub struct Phong {
    pub normal: Option<String>,
    pub exponent: Option<String>,
    pub warp: Option<String>,
    pub boost: f32,
    pub fresnel: [f32; 3],
    pub enabled: bool,
    pub self_illum: bool,
}

impl Phong {
    pub fn parse(shader: &portal::keyvalues::Kv) -> Result<Self, String> {
        let texture = |name| {
            shader
                .get(name)
                .map(|s| s.trim().replace('\\', "/").to_ascii_lowercase())
        };
        let number = |key, default| -> Result<f32, String> {
            shader.get(key).map_or(Ok(default), |s| {
                s.trim()
                    .parse::<f32>()
                    .ok()
                    .filter(|v| v.is_finite())
                    .ok_or_else(|| format!("invalid {key}: {s}"))
            })
        };
        let mut fresnel = [0.0, 0.5, 1.0];
        if let Some(value) = shader.get("$phongfresnelranges") {
            let values: Vec<_> = value
                .trim_matches(|c: char| c == '[' || c == ']' || c.is_whitespace())
                .split_whitespace()
                .map(str::parse::<f32>)
                .collect();
            if values.len() != 3 {
                return Err(format!("invalid $phongfresnelranges: {value}"));
            }
            for (target, parsed) in fresnel.iter_mut().zip(values) {
                *target = parsed
                    .ok()
                    .filter(|v| v.is_finite())
                    .ok_or_else(|| format!("invalid $phongfresnelranges: {value}"))?;
            }
        }
        Ok(Self {
            normal: texture("$bumpmap"),
            exponent: texture("$phongexponenttexture"),
            warp: texture("$lightwarptexture"),
            boost: number("$phongboost", 1.0)?,
            fresnel,
            enabled: shader.get("$phong").is_some_and(|v| v.trim() == "1"),
            self_illum: shader.get("$selfillum").is_some_and(|v| v.trim() == "1"),
        })
    }
}

pub fn exponent(red: f32) -> f32 {
    1.0 + 149.0 * red
}
pub fn fresnel(ndotv: f32, ranges: [f32; 3]) -> f32 {
    let f = (1.0 - ndotv).clamp(0.0, 1.0).powi(2);
    if f > 0.5 {
        ranges[1] + (ranges[2] - ranges[1]) * (2.0 * f - 1.0)
    } else {
        ranges[0] + (ranges[1] - ranges[0]) * (2.0 * f)
    }
}

pub fn apply(
    look: &crate::viewmodel::Look,
    files: &portal::vpk::SearchPath,
    images: &mut bevy::prelude::Assets<bevy::prelude::Image>,
    material: &mut crate::bodyportal::BodyMaterial,
) -> Result<(), String> {
    use bevy::prelude::*;
    let p = &look.phong;
    let mut image = |name: &Option<String>, srgb| -> Result<Option<Handle<Image>>, String> {
        name.as_deref()
            .map(|name| crate::texture::load_as(files, name, srgb).map(|image| images.add(image)))
            .transpose()
    };
    material.base.normal_map_texture = image(&p.normal, false)?;
    // the game ships materials that name a missing exponent with phong off
    material.extension.exponent = image(&p.exponent.clone().filter(|_| p.enabled), false)?;
    material.extension.lightwarp = image(&p.warp, false)?;
    material.extension.phong = Vec4::new(p.fresnel[0], p.fresnel[1], p.fresnel[2], p.boost);
    material.extension.model_flags = Vec4::new(
        p.enabled as u8 as f32,
        p.warp.is_some() as u8 as f32,
        p.self_illum as u8 as f32,
        0.0,
    );
    Ok(())
}
