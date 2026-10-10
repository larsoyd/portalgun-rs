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
