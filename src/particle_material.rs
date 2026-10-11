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
    fn alpha_mode(&self) -> AlphaMode {
        match self.key.blend {
            Blend::Add | Blend::AddAlpha => AlphaMode::Add,
            Blend::Over => AlphaMode::Blend,
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
        if let Some(depth) = descriptor.depth_stencil.as_mut() {
            depth.depth_write_enabled = Some(false);
            if key.bind_group_data.ignore_z {
                depth.depth_compare = Some(CompareFunction::Always);
            }
        }
        let (src, dst) = match key.bind_group_data.blend {
            Blend::Add => (BlendFactor::One, BlendFactor::One),
            Blend::AddAlpha => (BlendFactor::SrcAlpha, BlendFactor::One),
            Blend::Over => (BlendFactor::SrcAlpha, BlendFactor::OneMinusSrcAlpha),
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

pub struct LoadedMaterial {
    pub handle: Handle<ParticleMaterial>,
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
    pub fn read(search: &SearchPath, name: &str) -> Result<Option<Self>, String> {
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
        if kind == "refract" {
            warn!("[particles] material={name} unsupported=Refract");
            return Ok(None);
        }
        let float = |key: &str, default: f32| {
            shader
                .get(key)
                .and_then(|v| v.trim().parse::<f32>().ok())
                .unwrap_or(default)
        };
        let on = |key: &str| float(key, 0.0) != 0.0;
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
            "sprite" if float("$spriterendermode", 0.0) == 5.0 => {
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
