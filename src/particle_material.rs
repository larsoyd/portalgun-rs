// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::collections::HashMap;

use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::mesh::{MeshVertexAttribute, MeshVertexBufferLayoutRef};
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, CompareFunction,
    RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError, VertexFormat,
};
use bevy::shader::ShaderRef;
use portal::particles::renderers::Card;
use portal::particles::sheet::Sheet;
use portal::vpk::SearchPath;

use crate::effects::ParticleEffects;
use crate::sound::Files;

pub const FRAME_BLEND: MeshVertexAttribute =
    MeshVertexAttribute::new("ParticleFrameBlend", 0x70617274, VertexFormat::Float32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Blend {
    Add,
    AddAlpha,
    Over,
    Premultiplied,
    Refract,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MaterialKey {
    blend: Blend,
    ignore_z: bool,
}
impl From<&ParticleMaterial> for MaterialKey {
    fn from(m: &ParticleMaterial) -> Self {
        m.key
    }
}
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
#[bind_group_data(MaterialKey)]
pub struct ParticleMaterial {
    #[uniform(0)]
    params: Params,
    #[texture(1)]
    #[sampler(2)]
    texture: Handle<Image>,
    key: MaterialKey,
}
#[derive(Clone, Copy, Debug, ShaderType)]
struct Params {
    flags: Vec4,
    card: Vec4,
    depth: Vec4,
}
impl Material for ParticleMaterial {
    fn vertex_shader() -> ShaderRef {
        "embedded://portalgun/shaders/particle.wgsl".into()
    }
    fn fragment_shader() -> ShaderRef {
        Self::vertex_shader()
    }
    fn reads_view_transmission_texture(&self) -> bool {
        self.key.blend == Blend::Refract
    }
    fn alpha_mode(&self) -> AlphaMode {
        match self.key.blend {
            Blend::Add | Blend::AddAlpha => AlphaMode::Add,
            Blend::Over | Blend::Refract => AlphaMode::Blend,
            Blend::Premultiplied => AlphaMode::Premultiplied,
        }
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn specialize(
        _: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.vertex.buffers = vec![layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_UV_0.at_shader_location(1),
            Mesh::ATTRIBUTE_UV_1.at_shader_location(2),
            Mesh::ATTRIBUTE_COLOR.at_shader_location(3),
            FRAME_BLEND.at_shader_location(4),
        ])?];
        descriptor.primitive.cull_mode = None;
        if key.bind_group_data.blend == Blend::Refract {
            descriptor.vertex.shader_defs.push("REFRACT".into());
            if let Some(fragment) = descriptor.fragment.as_mut() {
                fragment.shader_defs.push("REFRACT".into());
            }
        }
        if let Some(depth) = descriptor.depth_stencil.as_mut() {
            depth.depth_write_enabled = Some(false);
            if key.bind_group_data.ignore_z {
                depth.depth_compare = Some(CompareFunction::Always);
            }
        }
        let (src, dst) = match key.bind_group_data.blend {
            Blend::Add => (BlendFactor::One, BlendFactor::One),
            Blend::AddAlpha => (BlendFactor::SrcAlpha, BlendFactor::One),
            Blend::Over | Blend::Refract => (BlendFactor::SrcAlpha, BlendFactor::OneMinusSrcAlpha),
            Blend::Premultiplied => (BlendFactor::One, BlendFactor::OneMinusSrcAlpha),
        };
        if let Some(fragment) = &mut descriptor.fragment {
            for target in fragment.targets.iter_mut().flatten() {
                target.blend = Some(BlendState {
                    color: BlendComponent {
                        src_factor: src,
                        dst_factor: dst,
                        operation: BlendOperation::Add,
                    },
                    alpha: BlendComponent::OVER,
                });
            }
        }
        Ok(())
    }
}

/// Draws nothing in the transmissive pass, so the frame is copied before a refracting effect draws
/// Translated from 0x5ccbd0 (decompiled, client.so)
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug, Default)]
pub struct FrameCopy {}
impl Material for FrameCopy {
    fn vertex_shader() -> ShaderRef {
        "embedded://portalgun/shaders/frame_copy.wgsl".into()
    }
    fn fragment_shader() -> ShaderRef {
        Self::vertex_shader()
    }
    fn reads_view_transmission_texture(&self) -> bool {
        true
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn specialize(
        _: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.vertex.buffers = vec![layout
            .0
            .get_layout(&[Mesh::ATTRIBUTE_POSITION.at_shader_location(0)])?];
        if let Some(depth) = descriptor.depth_stencil.as_mut() {
            depth.depth_write_enabled = Some(false);
        }
        Ok(())
    }
}

pub struct LoadedMaterial {
    pub handle: Handle<ParticleMaterial>,
    pub frame_copy: Option<Handle<FrameCopy>>,
    pub height: f32,
    pub sheet: Option<Sheet>,
    pub card: Option<Card>,
}
#[derive(Resource, Default)]
pub struct ParticleMaterials(pub HashMap<String, LoadedMaterial>);

pub struct MaterialSource {
    pub image: Image,
    pub height: f32,
    pub sheet: Option<Sheet>,
    pub card: Option<Card>,
    pub blend: Blend,
    params: Params,
    ignore_z: bool,
}
impl MaterialSource {
    pub fn upload(
        self,
        images: &mut Assets<Image>,
        materials: &mut Assets<ParticleMaterial>,
    ) -> Handle<ParticleMaterial> {
        materials.add(ParticleMaterial {
            params: self.params,
            texture: images.add(self.image),
            key: MaterialKey {
                blend: self.blend,
                ignore_z: self.ignore_z,
            },
        })
    }

    pub fn read_gun(search: &SearchPath, name: &str) -> Result<Self, String> {
        Self::read_mode(search, name, true)?
            .ok_or_else(|| format!("unsupported gun effect material {name}"))
    }

    pub fn read(search: &SearchPath, name: &str) -> Result<Option<Self>, String> {
        Self::read_mode(search, name, false)
    }

    fn read_mode(search: &SearchPath, name: &str, beam: bool) -> Result<Option<Self>, String> {
        if name.is_empty() {
            return Ok(None);
        }
        let name = name.replace('\\', "/");
        let path = format!("materials/{}.vmt", name.trim_end_matches(".vmt"));
        let bytes = search.read(&path).map_err(|e| e.to_string())?;
        let roots = portal::keyvalues::parse(&String::from_utf8_lossy(&bytes))
            .map_err(|e| e.to_string())?;
        let shader = roots.first().ok_or_else(|| format!("{path}: empty"))?;
        let kind = shader.key.to_ascii_lowercase();
        let float = |key: &str, default: f32| {
            shader
                .get(key)
                .and_then(|v| v.trim().parse::<f32>().ok())
                .unwrap_or(default)
        };
        let on = |key: &str| float(key, 0.0) != 0.0;
        // Translated from 0x61fb0 (decompiled, stdshader_dx9.so)
        if kind == "refract" {
            let textures = ["$basetexture", "$refracttinttexture", "$envmap"];
            if textures.iter().any(|k| shader.get(k).is_some())
                || on("$masked")
                || on("$fadeoutonsilhouette")
                || !on("$vertexcolormodulate")
            {
                return Err(format!("{path}: unsupported Refract combo"));
            }
            let normal = shader
                .get("$normalmap")
                .ok_or_else(|| format!("{path}: Refract without $normalmap"))?
                .trim()
                .replace('\\', "/");
            let tint = shader.get("$refracttint").unwrap_or("[1 1 1]");
            let [r, g, b] = crate::texture::colour(tint)
                .ok_or_else(|| format!("{path}: $refracttint {tint:?}"))?
                .map(|c| c.powf(2.2));
            return Ok(Some(Self {
                image: crate::texture::load_as(search, &normal, false)?,
                height: 1.0,
                sheet: None,
                card: None,
                blend: Blend::Refract,
                params: Params {
                    flags: Vec4::ZERO,
                    card: Vec4::new(
                        float("$refractamount", 0.0),
                        float("$bluramount", 0.0).clamp(0.0, 1.0).trunc(),
                        0.0,
                        0.0,
                    ),
                    depth: Vec4::new(r, g, b, 0.0),
                },
                ignore_z: on("$ignorez"),
            }));
        }
        let (mut image, height, sheet, alpha_bits) = if let Some(base) = shader.get("$basetexture")
        {
            let base = base.trim().replace('\\', "/");
            let bytes = search
                .read(&format!("materials/{base}.vtf"))
                .map_err(|e| e.to_string())?;
            let vtf = portal::vtf::Vtf::parse(&bytes).map_err(|e| e.to_string())?;
            let sheet = portal::vtf::sheet(&bytes)
                .map_err(|e| e.to_string())?
                .map(Sheet::parse)
                .transpose()
                .map_err(|e| e.to_string())?;
            let mut image = crate::texture::load(search, &base)?;
            let clamp = |flag| {
                if vtf.flags & flag != 0 {
                    ImageAddressMode::ClampToEdge
                } else {
                    ImageAddressMode::Repeat
                }
            };
            image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                address_mode_u: clamp(4),
                address_mode_v: clamp(8),
                ..ImageSamplerDescriptor::linear()
            });
            (image, vtf.height as f32, sheet, vtf.flags & 0x3000 != 0)
        } else {
            (Image::default(), 1.0, None, false)
        };
        let mut params = Params {
            flags: Vec4::new(1.0, 1.0, 1.0, 0.0),
            card: Vec4::new(1.0, 0.0, -1.0, 0.0),
            depth: Vec4::ZERO,
        };
        let (blend, card) = match kind.as_str() {
            "spritecard" => {
                let add_self = float("$addself", 0.0);
                if on("$dualsequence")
                    || on("$addbasetexture2")
                    || shader.get("$ramptexture").is_some()
                {
                    return Err(format!("{path}: unsupported SpriteCard sequence/ramp"));
                }
                let blend = if on("$addoverblend") || add_self != 0.0 {
                    Blend::Premultiplied
                } else if on("$additive") {
                    Blend::AddAlpha
                } else {
                    Blend::Over
                };
                params.card = Vec4::new(
                    float("$overbrightfactor", 1.0),
                    add_self,
                    if add_self == 0.0 { 0.01 } else { -1.0 },
                    float("$blendframes", 1.0),
                );
                params.depth = Vec4::new(
                    float("$depthblend", 1.0),
                    float("$depthblendscale", 50.0),
                    0.0,
                    0.0,
                );
                let d = Card::default();
                (
                    blend,
                    Some(Card {
                        min_size: float("$minsize", d.min_size),
                        max_size: float("$maxsize", d.max_size),
                        start_fade: float("$startfadesize", d.start_fade),
                        end_fade: float("$endfadesize", d.end_fade),
                        max_distance: float("$maxdistance", d.max_distance),
                        far_interval: float("$farfadeinterval", d.far_interval),
                    }),
                )
            }
            "unlitgeneric" => {
                params.flags.x = f32::from(on("$vertexcolor"));
                params.flags.y = f32::from(on("$vertexalpha"));
                let alpha = on("$vertexalpha") || (alpha_bits && !on("$alphatest"));
                (
                    if on("$additive") {
                        if alpha {
                            Blend::AddAlpha
                        } else {
                            Blend::Add
                        }
                    } else {
                        Blend::Over
                    },
                    None,
                )
            }
            "sprite" if beam || float("$spriterendermode", 0.0) == 5.0 => {
                image.texture_descriptor.format =
                    image.texture_descriptor.format.remove_srgb_suffix();
                params.flags.z = 0.0;
                params.flags.w = 1.0;
                (Blend::AddAlpha, None)
            }
            _ => return Err(format!("{path}: unsupported shader {}", shader.key)),
        };
        Ok(Some(Self {
            image,
            height,
            sheet,
            card,
            blend,
            params,
            ignore_z: on("$ignorez"),
        }))
    }
}

pub(crate) fn load(
    mut commands: Commands,
    fx: Res<ParticleEffects>,
    files: Res<Files>,
    mut images: ResMut<Assets<Image>>,
    mut assets: ResMut<Assets<ParticleMaterial>>,
    mut copies: ResMut<Assets<FrameCopy>>,
) {
    let mut names: Vec<_> = fx.defs.iter().map(|d| d.material.as_str()).collect();
    names.sort();
    names.dedup();
    let mut materials = ParticleMaterials::default();
    for name in names {
        match MaterialSource::read(&files, name) {
            Ok(Some(m)) => {
                let handle = assets.add(ParticleMaterial {
                    params: m.params,
                    texture: images.add(m.image),
                    key: MaterialKey {
                        blend: m.blend,
                        ignore_z: m.ignore_z,
                    },
                });
                materials.0.insert(
                    name.into(),
                    LoadedMaterial {
                        handle,
                        frame_copy: (m.blend == Blend::Refract).then(|| copies.add(FrameCopy {})),
                        height: m.height,
                        sheet: m.sheet,
                        card: m.card,
                    },
                );
            }
            Ok(None) => {}
            Err(e) => panic!("[particles] refused: {e}"),
        }
    }
    info!(
        "[particles] definitions={} materials={}",
        fx.defs.len(),
        materials.0.len()
    );
    commands.insert_resource(materials);
}

#[cfg(test)]
mod tests {
    use super::*;
    use portal::install::Install;

    fn search() -> SearchPath {
        let home = std::env::var("HOME").unwrap();
        let root = portal::install::root(None, None, std::path::Path::new(&home));
        Install::open(&root).unwrap().search
    }

    fn material(m: &MaterialSource) -> ParticleMaterial {
        ParticleMaterial {
            params: m.params,
            texture: Handle::default(),
            key: MaterialKey {
                blend: m.blend,
                ignore_z: m.ignore_z,
            },
        }
    }

    #[test]
    fn the_fizzle_warp_refracts_a_blurred_copy_of_the_frame_through_its_normal_map() {
        let search = search();
        let warp = MaterialSource::read(&search, "particle/warp2_warp.vmt")
            .unwrap()
            .expect("warp2_warp loads");
        assert_eq!(warp.blend, Blend::Refract);
        assert_eq!(warp.params.card, Vec4::new(0.5, 1.0, 0.0, 0.0));
        assert_eq!(warp.params.depth, Vec4::new(1.0, 1.0, 1.0, 0.0));
        assert_eq!(warp.params.flags.z, 0.0, "vertex colour stays raw");
        assert!(!warp.image.texture_descriptor.format.is_srgb());
        assert!(warp.card.is_none());
        let m = material(&warp);
        assert_eq!(m.alpha_mode(), AlphaMode::Blend);
        assert!(m.reads_view_transmission_texture());
        let energy = MaterialSource::read(&search, "effects/energyball.vmt")
            .unwrap()
            .unwrap();
        assert!(!material(&energy).reads_view_transmission_texture());
    }
}
