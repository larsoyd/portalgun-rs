use std::path::Path;

use portal::anim::sample;
use portal::install::{root, Install};
use portal::mdl::Mdl;
use portalgun::viewanim::{frame_at, GunAnim};

fn gun() -> Mdl {
    let dir = root(None, None, Path::new(&std::env::var("HOME").unwrap()));
    let install = Install::open(&dir).unwrap_or_else(|e| panic!("install at {dir:?}: {e}"));
    Mdl::parse(
        install
            .search
            .read("models/weapons/v_portalgun.mdl")
            .unwrap(),
    )
    .unwrap()
}

fn seq(m: &Mdl, label: &str) -> usize {
    m.sequence_named(label).unwrap()
}

#[test]
fn the_draw_runs_thirty_frames_at_thirty_a_second() {
    let m = gun();
    let mut a = GunAnim::default();
    assert!(a.play(&m, "ACT_VM_DRAW", 10.0));
    assert!((a.left(&m, 10.0) - 1.0).abs() < 1e-5);
    assert!((a.left(&m, 10.25) - 0.75).abs() < 1e-5);
    assert_eq!(a.left(&m, 12.0), 0.0);
    assert!(a.play(&m, "ACT_VM_PRIMARYATTACK", 0.0));
    assert!((a.left(&m, 0.0) - 0.5).abs() < 1e-5);
    assert!(a.play(&m, "ACT_VM_IDLE", 0.0));
    assert_eq!(a.left(&m, 0.0), 0.0);
    assert!(!a.play(&m, "ACT_VM_RECOIL1", 0.0));
}

#[test]
fn loops_wrap_and_one_shots_hold_their_last_frame() {
    let m = gun();
    assert!((frame_at(&m, seq(&m, "draw"), 0.5) - 15.0).abs() < 1e-4);
    assert_eq!(frame_at(&m, seq(&m, "draw"), 5.0), 30.0);
    let low = seq(&m, "lowidle");
    assert!((frame_at(&m, low, 1.5 / 30.0) - 0.5).abs() < 1e-4);
    assert!((frame_at(&m, low, 0.5 / 30.0) - 0.5).abs() < 1e-4);
    assert_eq!(frame_at(&m, seq(&m, "draw"), -1.0), 0.0);
}

#[test]
fn a_new_sequence_fades_in_over_its_fade_time() {
    let m = gun();
    let mut a = GunAnim::default();
    a.play(&m, "ACT_VM_DRAW", 0.0);
    a.play(&m, "ACT_VM_PRIMARYATTACK", 0.5);
    let draw = sample(&m, 3, frame_at(&m, seq(&m, "draw"), 0.6)).unwrap();
    let fire = sample(&m, 1, frame_at(&m, seq(&m, "fire1"), 0.1)).unwrap();
    let half = a.pose(&m, 0.6).unwrap();
    let moved = (0..draw.len())
        .filter(|&i| (0..3).any(|k| (draw[i].pos[k] - fire[i].pos[k]).abs() > 0.1))
        .collect::<Vec<_>>();
    assert!(!moved.is_empty(), "draw and fire poses differ somewhere");
    for &i in &moved {
        for k in 0..3 {
            let want = (draw[i].pos[k] + fire[i].pos[k]) / 2.0;
            assert!((half[i].pos[k] - want).abs() < 1e-3, "bone {i} axis {k}");
        }
    }
    let done = a.pose(&m, 0.7).unwrap();
    let fire_at = sample(&m, 1, frame_at(&m, seq(&m, "fire1"), 0.2)).unwrap();
    for &i in &moved {
        for k in 0..3 {
            assert!(
                (done[i].pos[k] - fire_at[i].pos[k]).abs() < 1e-4,
                "bone {i} axis {k}"
            );
        }
    }
}
