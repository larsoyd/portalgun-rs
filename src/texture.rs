// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use bevy::asset::RenderAssetUsages;
use bevy::image::{
    CompressedImageFormats, ImageAddressMode, ImageSampler, ImageSamplerDescriptor, ImageType,
};
use bevy::prelude::*;
use bevy::render::render_resource::TextureDimension;
use portal::keyvalues;
use portal::vpk::SearchPath;

/// IMAGE_FORMAT_BGR888
const BGR888: i32 = 3;

pub fn base_texture(search: &SearchPath, material: &str) -> Result<String, String> {
    let path = format!("materials/{material}.vmt");
    let bytes = search.read(&path).map_err(|e| format!("{path}: {e}"))?;
    let root =
        keyvalues::parse(&String::from_utf8_lossy(&bytes)).map_err(|e| format!("{path}: {e}"))?;
    root.first()
        .and_then(|shader| shader.get("$basetexture"))
        .map(|t| t.trim().replace('\\', "/"))
        .ok_or_else(|| format!("{path}: no $basetexture"))
}

#[derive(Debug, PartialEq)]
pub enum WorldLook {
    Opaque(String),
    Translucent(String),
    Refract([f32; 3]),
}

pub fn world_look(search: &SearchPath, material: &str) -> Result<WorldLook, String> {
    let path = format!("materials/{material}.vmt");
    let bytes = search.read(&path).map_err(|e| format!("{path}: {e}"))?;
    let root =
        keyvalues::parse(&String::from_utf8_lossy(&bytes)).map_err(|e| format!("{path}: {e}"))?;
    let shader = root.first().ok_or_else(|| format!("{path}: empty"))?;
    if shader.key.eq_ignore_ascii_case("refract") {
        let tint = shader.get("$refracttint").unwrap_or("[1 1 1]");
        return colour(tint)
            .map(WorldLook::Refract)
            .ok_or_else(|| format!("{path}: $refracttint {tint:?}"));
    }
    let base = base_texture(search, material)?;
    Ok(match shader.get("$translucent").map(str::trim) {
        Some("1") => WorldLook::Translucent(base),
        _ => WorldLook::Opaque(base),
    })
}

/// a VMT colour, braces hold 0 to 255 and brackets 0 to 1
pub(crate) fn colour(text: &str) -> Option<[f32; 3]> {
    let t = text.trim();
    let scale = if t.starts_with('{') { 255.0 } else { 1.0 };
    let v: Vec<f32> = t
        .trim_matches(|c| matches!(c, '{' | '}' | '[' | ']'))
        .split_whitespace()
        .map(|x| x.parse::<f32>().ok().map(|x| x / scale))
        .collect::<Option<_>>()?;
    v.try_into().ok()
}

/// A colour texture, repeating like a brush face.
pub fn load(search: &SearchPath, name: &str) -> Result<Image, String> {
    load_as(search, name, true)
}

pub fn frames(search: &SearchPath, name: &str) -> Result<usize, String> {
    let path = format!("materials/{name}.vtf");
    let vtf = search.read(&path).map_err(|e| format!("{path}: {e}"))?;
    Ok(portal::vtf::Vtf::parse(&vtf)
        .map_err(|e| format!("{path}: {e}"))?
        .frames)
}

/// One frame of an animated colour texture, as a texture toggle shows it.
pub fn load_frame(search: &SearchPath, name: &str, frame: usize) -> Result<Image, String> {
    decode(search, name, true, frame)
}

pub fn load_as(search: &SearchPath, name: &str, srgb: bool) -> Result<Image, String> {
    decode(search, name, srgb, 0)
}

fn decode(search: &SearchPath, name: &str, srgb: bool, frame: usize) -> Result<Image, String> {
    let path = format!("materials/{name}.vtf");
    let vtf = search.read(&path).map_err(|e| format!("{path}: {e}"))?;
    let dds = portal::vtf::frame_to_dds(&vtf, frame).map_err(|e| format!("{path}: {e}"))?;
    let format = portal::vtf::Vtf::parse(&vtf)
        .map_err(|e| format!("{path}: {e}"))?
        .format;
    let sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        ..ImageSamplerDescriptor::linear()
    });
    let mut image = Image::from_buffer(
        &dds,
        ImageType::Extension("dds"),
        CompressedImageFormats::BC,
        srgb,
        sampler,
        RenderAssetUsages::RENDER_WORLD,
    )
    .map_err(|e| format!("{path}: {e}"))?;
    // the dds loader calls a one texel high ramp 1D, which cannot hold its mips
    image.texture_descriptor.dimension = TextureDimension::D2;
    // it also widens 24 bit pixels in file order, so BGR888 comes out with red and blue swapped
    if format == BGR888 {
        if let Some(data) = image.data.as_mut() {
            data.as_chunks_mut::<4>()
                .0
                .iter_mut()
                .for_each(|p| p.swap(0, 2));
        }
    }
    Ok(image)
}
