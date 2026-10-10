use bevy::asset::RenderAssetUsages;
use bevy::image::{
    CompressedImageFormats, ImageAddressMode, ImageSampler, ImageSamplerDescriptor, ImageType,
};
use bevy::prelude::*;
use portal::keyvalues;
use portal::vpk::SearchPath;

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

/// A colour texture, repeating like a brush face.
pub fn load(search: &SearchPath, name: &str) -> Result<Image, String> {
    let path = format!("materials/{name}.vtf");
    let vtf = search.read(&path).map_err(|e| format!("{path}: {e}"))?;
    let dds = portal::vtf::to_dds(&vtf).map_err(|e| format!("{path}: {e}"))?;
    let sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        ..ImageSamplerDescriptor::linear()
    });
    Image::from_buffer(
        &dds,
        ImageType::Extension("dds"),
        CompressedImageFormats::BC,
        true,
        sampler,
        RenderAssetUsages::RENDER_WORLD,
    )
    .map_err(|e| format!("{path}: {e}"))
}
