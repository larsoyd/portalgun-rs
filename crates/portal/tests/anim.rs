// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

mod common;

use common::*;
use portal::anim::{cycle_rate, sample};
use portal::mdl::Mdl;

const RAWPOS: u8 = 0x01;
const RAWROT: u8 = 0x02;
const ANIMPOS: u8 = 0x04;
const ANIMROT: u8 = 0x08;
const DELTA: u8 = 0x10;
const RAWROT2: u8 = 0x20;

fn quat48(q: [f32; 4]) -> Vec<u8> {
    let x = (q[0] * 32768.0 + 32768.0) as u16;
    let y = (q[1] * 32768.0 + 32768.0) as u16;
    let z = (q[2] * 16384.0 + 16384.0) as u16 | if q[3] < 0.0 { 0x8000 } else { 0 };
    [x, y, z].iter().flat_map(|v| v.to_le_bytes()).collect()
}

fn quat64(q: [f32; 4]) -> Vec<u8> {
    let raw = |v: f32| ((v * 1048576.5 + 1048576.0).round() as u64) & 0x1f_ffff;
    let bits = raw(q[0]) | raw(q[1]) << 21 | raw(q[2]) << 42 | if q[3] < 0.0 { 1 << 63 } else { 0 };
    bits.to_le_bytes().to_vec()
}

fn half(v: f32) -> [u8; 2] {
    let bits = v.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exp = ((bits >> 23) & 0xff) as i32 - 127 + 15;
    let mant = ((bits >> 13) & 0x3ff) as u16;
    let h = if v == 0.0 {
        sign
    } else {
        sign | (exp as u16) << 10 | mant
    };
    h.to_le_bytes()
}

fn model() -> Mdl {
    let mut root = bone("Root", -1, [1.0, 2.0, 3.0]);
    root.rot = [0.0, 0.0, 0.5];
    root.rot_scale = [0.01; 3];
    root.pos_scale = [0.5; 3];
    let tracks = value_tracks(&[
        [&[(2, 3, vec![100, 200]), (1, 2, vec![50])], &[], &[]],
        [&[], &[(1, 5, vec![10])], &[]],
    ]);
    let raw_rot = quat48([0.0, 0.0, 0.70710677, 0.70710677]);
    let raw_pos: Vec<u8> = [1.5f32, -2.0, 0.25].iter().flat_map(|v| half(*v)).collect();
    let rot2 = quat64([0.5, -0.5, 0.5, -0.5]);
    let chain = anim_chain(&[
        (0, ANIMROT | ANIMPOS, tracks),
        (1, RAWROT | RAWPOS, [raw_rot, raw_pos].concat()),
        (3, DELTA, vec![]),
        (4, RAWROT2, rot2),
    ]);
    let def = MdlDef {
        bones: vec![
            root,
            bone("Hand", 0, [4.0, 0.0, 0.0]),
            bone("Still", 1, [0.0, 7.0, 0.0]),
            BoneDef {
                rot: [0.3, 0.0, 0.0],
                ..bone("Delta", 1, [0.0, 0.0, 9.0])
            },
            bone("Wide", 1, [0.0, 0.0, 1.0]),
        ],
        anims: vec![AnimDef {
            name: "@a",
            fps: 30.0,
            flags: 0,
            frames: 5,
            data: chain,
        }],
        seqs: vec![],
        textures: vec![],
        cd: vec![],
        meshes: vec![],
        attachments: vec![],
    };
    Mdl::parse(mdl(&def)).unwrap()
}

fn close(a: &[f32], b: &[f32]) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < 2e-3)
}

fn same_rotation(a: [f32; 4], b: [f32; 4]) -> bool {
    let d: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    d.abs() > 0.9999
}

#[test]
fn run_length_tracks_add_the_bone_base() {
    let m = model();
    let p = sample(&m, 0, 0.0).unwrap();
    assert!(
        same_rotation(p[0].quat, angle_quat([1.0, 0.0, 0.5])),
        "{:?}",
        p[0].quat
    );
    assert_eq!(p[0].pos, [1.0, 7.0, 3.0]);
    let p = sample(&m, 0, 2.0).unwrap();
    assert!(
        same_rotation(p[0].quat, angle_quat([2.0, 0.0, 0.5])),
        "{:?}",
        p[0].quat
    );
    let p = sample(&m, 0, 3.5).unwrap();
    assert!(
        same_rotation(p[0].quat, angle_quat([0.5, 0.0, 0.5])),
        "{:?}",
        p[0].quat
    );
}

#[test]
fn frames_blend_between_neighbours() {
    let m = model();
    let p = sample(&m, 0, 0.5).unwrap();
    let a = angle_quat([1.0, 0.0, 0.5]);
    let b = angle_quat([2.0, 0.0, 0.5]);
    let mid: Vec<f32> = a.iter().zip(b).map(|(x, y)| (x + y) * 0.5).collect();
    let n = mid.iter().map(|v| v * v).sum::<f32>().sqrt();
    let mid: Vec<f32> = mid.iter().map(|v| v / n).collect();
    assert!(close(&p[0].quat, &mid), "{:?} vs {mid:?}", p[0].quat);
}

#[test]
fn last_frame_of_a_run_blends_into_the_next_run() {
    let p = sample(&model(), 0, 2.5).unwrap();
    let want = portal::anim::blend(
        angle_quat([2.0, 0.0, 0.5]),
        angle_quat([0.5, 0.0, 0.5]),
        0.5,
    );
    assert!(close(&p[0].quat, &want), "{:?} vs {want:?}", p[0].quat);
}

#[test]
fn raw_values_are_unpacked() {
    let p = sample(&model(), 0, 1.0).unwrap();
    assert!(
        close(&p[1].quat, &[0.0, 0.0, 0.70710677, 0.70710677]),
        "{:?}",
        p[1].quat
    );
    assert_eq!(p[1].pos, [1.5, -2.0, 0.25]);
    assert!(
        close(&p[4].quat, &[0.5, -0.5, 0.5, -0.5]),
        "{:?}",
        p[4].quat
    );
    assert_eq!(p[4].pos, [0.0, 0.0, 1.0]);
}

#[test]
fn bones_without_records_keep_their_base_pose() {
    let p = sample(&model(), 0, 1.0).unwrap();
    assert_eq!(p[2].pos, [0.0, 7.0, 0.0]);
    assert_eq!(p[2].quat, [0.0, 0.0, 0.0, 1.0]);
    assert_eq!(p[3].pos, [0.0, 0.0, 0.0]);
    assert_eq!(p[3].quat, [0.0, 0.0, 0.0, 1.0]);
}

#[test]
fn cycle_rate_spans_frames_minus_one() {
    let m = model();
    assert_eq!(cycle_rate(&m.anims[0]), 30.0 / 4.0);
}

#[test]
fn frame_past_the_end_is_refused() {
    assert!(sample(&model(), 0, 5.0).is_err());
    assert!(sample(&model(), 1, 0.0).is_err());
}
