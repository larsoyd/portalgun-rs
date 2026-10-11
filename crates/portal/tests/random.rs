// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use portal::particles::random::{pow_fixed, sha256, RandomTable, SimdRandom, TABLE_OFFSET};

fn hex(d: [u8; 32]) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn sha256_matches_the_standard_vectors() {
    assert_eq!(
        hex(sha256(b"")),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        hex(sha256(b"abc")),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    let long = vec![b'a'; 1_000_000];
    assert_eq!(
        hex(sha256(&long)),
        "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
    );
    assert_eq!(
        hex(sha256(&[0x61; 64])),
        "ffe054fe7ae0cb6dc65c3af9b61d5209f439851db43d0ba5997337df154668eb"
    );
}

#[test]
fn a_table_that_is_not_portals_is_refused() {
    let mut fake = vec![0u8; TABLE_OFFSET + 16384];
    fake[TABLE_OFFSET] = 1;
    assert!(RandomTable::from_client(&fake).is_err());
    assert!(RandomTable::from_client(&fake[..100]).is_err());
}

#[test]
fn table_lookups_wrap_at_4096() {
    let t = RandomTable::from_values((0..4096).map(|i| i as f32 / 4096.0).collect());
    assert_eq!(t.get(5), 5.0 / 4096.0);
    assert_eq!(t.get(4096 + 7), 7.0 / 4096.0);
    assert_eq!(t.get(u32::MAX), 4095.0 / 4096.0);
}

#[test]
fn the_simd_stream_follows_source_seeding() {
    let mut r = SimdRandom::new();
    assert_eq!(
        r.draw(),
        [
            58804.0 / 65536.0,
            46829.0 / 65536.0,
            12879.0 / 65536.0,
            10980.0 / 65536.0
        ]
    );
    assert_eq!(
        r.draw(),
        [
            60880.0 / 65536.0,
            41137.0 / 65536.0,
            17058.0 / 65536.0,
            24476.0 / 65536.0
        ]
    );
    assert_eq!(
        r.draw(),
        [
            49754.0 / 65536.0,
            52994.0 / 65536.0,
            18689.0 / 65536.0,
            55300.0 / 65536.0
        ]
    );
}

#[test]
fn fixed_point_powers_use_quarters() {
    let x = 0.81f32;
    assert_eq!(pow_fixed(x, 1.0), x, "exponent one is exact");
    assert!((pow_fixed(x, 0.5) - 0.9).abs() < 1e-6);
    assert!((pow_fixed(x, 2.0) - 0.6561).abs() < 1e-6);
    assert!((pow_fixed(x, 0.25) - x.sqrt().sqrt()).abs() < 1e-6);
    assert!(
        (pow_fixed(x, 2.9) - x.powf(2.75)).abs() < 1e-5,
        "2.9 truncates to 11 quarters"
    );
    assert!((pow_fixed(x, -1.0) - 1.0 / x).abs() < 1e-5);
    assert_eq!(
        pow_fixed(0.0, -1.0),
        1.0 / f32::EPSILON,
        "zero is lifted before the reciprocal"
    );
}
