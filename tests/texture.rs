use std::path::Path;

use bevy::image::{ImageAddressMode, ImageSampler};
use portal::install::{root, Install};
use portal::vtf::Vtf;
use portalgun::texture::{base_texture, load};

fn install() -> Install {
    let dir = root(None, None, Path::new(&std::env::var("HOME").unwrap()));
    Install::open(&dir).unwrap_or_else(|e| panic!("install at {dir:?}: {e}"))
}

#[test]
fn a_material_names_its_base_texture() {
    let i = install();
    assert_eq!(
        base_texture(&i.search, "metal/metalwall048b").unwrap(),
        "Metal/metalwall048b"
    );
    let e = base_texture(&i.search, "metal/no_such_wall").unwrap_err();
    assert!(e.contains("materials/metal/no_such_wall.vmt"), "{e}");
}

#[test]
fn a_wall_texture_loads_at_its_own_size_tiling_and_in_srgb() {
    let i = install();
    let vtf = i
        .search
        .read("materials/concrete/concrete_modular_wall001a.vtf")
        .unwrap();
    let header = Vtf::parse(&vtf).unwrap();
    let image = load(&i.search, "Concrete/concrete_modular_wall001a").unwrap();
    assert_eq!(
        (image.width(), image.height()),
        (header.width, header.height)
    );
    assert!(image.texture_descriptor.format.is_srgb());
    assert_eq!(
        image.texture_descriptor.mip_level_count as usize,
        header.mips
    );
    let ImageSampler::Descriptor(s) = &image.sampler else {
        panic!("default sampler")
    };
    assert_eq!(s.address_mode_u, ImageAddressMode::Repeat);
    assert_eq!(s.address_mode_v, ImageAddressMode::Repeat);
}

#[test]
fn a_one_texel_high_ramp_stays_a_2d_texture_with_its_mips() {
    use bevy::render::render_resource::TextureDimension;
    let i = install();
    let ramp = load(&i.search, "models/portals/portal-blue-color").unwrap();
    assert_eq!((ramp.width(), ramp.height()), (256, 1));
    assert_eq!(ramp.texture_descriptor.dimension, TextureDimension::D2);
    assert_eq!(ramp.texture_descriptor.mip_level_count, 9);
}

#[test]
fn the_blue_ramp_comes_out_blue() {
    let i = install();
    let ramp = load(&i.search, "models/portals/portal-blue-color").unwrap();
    let data = ramp.data.as_ref().unwrap();
    let [r, _, b, a] = [0, 1, 2, 3].map(|k| data[200 * 4 + k]);
    assert!(b > r, "texel 200 rgb has red {r} over blue {b}");
    assert_eq!(a, 255);
    let orange = load(&i.search, "models/portals/portal-orange-color").unwrap();
    let data = orange.data.as_ref().unwrap();
    assert!(
        data[200 * 4] > data[200 * 4 + 2],
        "orange keeps red over blue"
    );
}
