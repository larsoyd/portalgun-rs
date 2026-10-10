mod common;
mod dds;

use common::Blob;
use portal::vtf::{to_dds, Vtf};

/// mips listed largest first, stored smallest first like the file
fn vtf(minor: u32, size: (u16, u16), format: i32, flags: u32, mips: &[Vec<u8>]) -> Vec<u8> {
    let header = if minor >= 3 { 104 } else { 80 };
    let mut b = Blob::new(header);
    b.bytes_at(0, b"VTF\0");
    b.i32(4, 7);
    b.i32(8, minor as i32);
    b.i32(12, header as i32);
    b.0[16..18].copy_from_slice(&size.0.to_le_bytes());
    b.0[18..20].copy_from_slice(&size.1.to_le_bytes());
    b.i32(20, flags as i32);
    b.0[24] = 1;
    b.i32(52, format);
    b.0[56] = mips.len() as u8;
    b.i32(57, 13);
    b.0[61] = 4;
    b.0[62] = 4;
    b.0[63] = 1;
    let thumb = vec![0xee; 8];
    if minor >= 3 {
        b.i32(68, 2);
        b.0[80] = 0x01;
        b.0[88] = 0x30;
        let t = b.bytes(&thumb);
        b.i32(84, t as i32);
        let hi = b.at();
        b.i32(92, hi as i32);
    } else {
        b.bytes(&thumb);
    }
    for m in mips.iter().rev() {
        b.bytes(m);
    }
    b.0
}

fn dxt1_chain() -> Vec<Vec<u8>> {
    vec![vec![1; 32], vec![2; 8], vec![3; 8], vec![4; 8]]
}

#[test]
fn dxt1_mips_come_out_largest_first() {
    let bytes = vtf(2, (8, 8), 13, 0, &dxt1_chain());
    let v = Vtf::parse(&bytes).unwrap();
    assert_eq!((v.width, v.height, v.mips, v.format), (8, 8, 4, 13));
    let d = dds::Dds::parse(to_dds(&bytes).unwrap()).unwrap();
    assert_eq!((d.width(), d.height(), d.mip_count()), (8, 8, 4));
    assert_eq!(d.format(), dds::Format::Bc1);
    assert_eq!(d.level_data(0, 0).unwrap(), &[1; 32]);
    assert_eq!(d.level_data(0, 3).unwrap(), &[4; 8]);
}

#[test]
fn version_three_finds_the_image_through_resources() {
    let mips = vec![vec![10, 20, 30, 11, 21, 31], vec![12, 22, 32]];
    let bytes = vtf(3, (2, 1), 3, 0, &mips);
    let d = dds::Dds::parse(to_dds(&bytes).unwrap()).unwrap();
    assert_eq!(d.mip_count(), 2);
    let rgba = d.decode_rgba(0, 0).unwrap();
    assert_eq!(&rgba[..4], &[30, 20, 10, 255]);
    assert_eq!(&rgba[4..8], &[31, 21, 11, 255]);
}

#[test]
fn uncompressed_layouts_keep_their_channels() {
    for (format, px, want) in [
        (0, vec![1, 2, 3, 4], [1, 2, 3, 4]),
        (1, vec![4, 3, 2, 1], [1, 2, 3, 4]),
        (2, vec![1, 2, 3], [1, 2, 3, 255]),
        (12, vec![3, 2, 1, 4], [1, 2, 3, 4]),
        (16, vec![3, 2, 1, 9], [1, 2, 3, 255]),
        (5, vec![7], [7, 7, 7, 255]),
        (6, vec![7, 8], [7, 7, 7, 8]),
    ] {
        let bytes = vtf(2, (1, 1), format, 0, &[px]);
        let d = dds::Dds::parse(to_dds(&bytes).unwrap()).unwrap();
        assert_eq!(d.decode_rgba(0, 0).unwrap(), want, "format {format}");
    }
}

#[test]
fn dxt5_keeps_its_alpha_format() {
    let bytes = vtf(2, (4, 4), 15, 0, &[vec![0; 16]]);
    let d = dds::Dds::parse(to_dds(&bytes).unwrap()).unwrap();
    assert_eq!(
        d.format(),
        dds::Format::Bc3 {
            premultiplied: false
        }
    );
}

#[test]
fn cube_maps_and_unknown_formats_are_refused() {
    assert!(to_dds(&vtf(2, (4, 4), 13, 0x4000, &[vec![0; 8]])).is_err());
    assert!(to_dds(&vtf(2, (4, 4), 24, 0, &[vec![0; 128]])).is_err());
    assert!(to_dds(&vtf(2, (8, 8), 13, 0, &[vec![0; 8]])).is_err());
    assert!(to_dds(b"VTX\0").is_err());
}

#[test]
fn the_sheet_resource_comes_out_by_its_tag() {
    let mut bytes = vtf(3, (2, 1), 3, 0, &[vec![0; 6]]);
    assert_eq!(portal::vtf::sheet(&bytes).unwrap(), None);
    let at = bytes.len() as i32;
    bytes[68..72].copy_from_slice(&3i32.to_le_bytes());
    bytes[96] = 0x10;
    bytes[100..104].copy_from_slice(&at.to_le_bytes());
    bytes.extend(3u32.to_le_bytes());
    bytes.extend([7, 8, 9]);
    assert_eq!(portal::vtf::sheet(&bytes).unwrap(), Some(&[7u8, 8, 9][..]));
    bytes.truncate(bytes.len() - 1);
    assert!(portal::vtf::sheet(&bytes).is_err(), "cut short");
}
