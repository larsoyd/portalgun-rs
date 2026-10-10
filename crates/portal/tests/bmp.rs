use std::path::Path;

use portal::bmp::Bmp;
use portal::install::root;

fn icon() -> Vec<u8> {
    let dir = root(None, None, Path::new(&std::env::var("HOME").unwrap()));
    let path = dir.join("portal/resource/game-icon.bmp");
    std::fs::read(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"))
}

#[test]
fn the_game_icon_decodes_top_row_first_with_its_alpha() {
    let b = Bmp::parse(&icon()).unwrap();
    assert_eq!((b.width, b.height), (48, 48));
    assert_eq!(b.rgba.len(), 48 * 48 * 4);
    let px = |x: usize, y: usize| &b.rgba[(y * 48 + x) * 4..][..4];
    assert_eq!(px(0, 0)[3], 0);
    assert_eq!(px(24, 24), [0, 0, 0, 255]);
}

fn tiny(height: i32, rows: [[u8; 4]; 2]) -> Vec<u8> {
    let mut f = b"BM".to_vec();
    f.extend([0; 8]);
    f.extend(70u32.to_le_bytes());
    f.extend(56u32.to_le_bytes());
    f.extend(1i32.to_le_bytes());
    f.extend(height.to_le_bytes());
    f.extend(1u16.to_le_bytes());
    f.extend(32u16.to_le_bytes());
    f.extend(3u32.to_le_bytes());
    f.extend([0; 20]);
    for m in [0xff00_0000u32, 0xff_0000, 0xff00, 0xff] {
        f.extend(m.to_le_bytes());
    }
    f.extend(rows.concat());
    f
}

#[test]
fn rows_are_flipped_only_for_a_bottom_up_file() {
    let rows = [[4, 3, 2, 1], [8, 7, 6, 5]];
    assert_eq!(
        Bmp::parse(&tiny(2, rows)).unwrap().rgba,
        [5, 6, 7, 8, 1, 2, 3, 4]
    );
    assert_eq!(
        Bmp::parse(&tiny(-2, rows)).unwrap().rgba,
        [1, 2, 3, 4, 5, 6, 7, 8]
    );
}

#[test]
fn anything_but_32_bit_bitfields_is_refused() {
    let mut f = tiny(2, [[0; 4]; 2]);
    f[28] = 24;
    assert!(Bmp::parse(&f).is_err());
    assert!(Bmp::parse(b"PK\x03\x04").is_err());
    let mut no_alpha = tiny(2, [[0; 4]; 2]);
    no_alpha[66..70].fill(0);
    assert!(Bmp::parse(&no_alpha).is_err());
}
