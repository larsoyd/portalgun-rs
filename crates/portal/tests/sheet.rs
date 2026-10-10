use portal::particles::sheet::Sheet;

/// sequence number, clamp, total time and its frames as (duration, rect)
type Seq = (i32, bool, f32, Vec<(f32, [f32; 4])>);

fn blob(version: i32, sequences: &[Seq]) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend(version.to_le_bytes());
    b.extend((sequences.len() as i32).to_le_bytes());
    for (seq, clamp, total, frames) in sequences {
        b.extend(seq.to_le_bytes());
        b.extend((*clamp as i32).to_le_bytes());
        b.extend((frames.len() as i32).to_le_bytes());
        b.extend(total.to_le_bytes());
        for (duration, rect) in frames {
            b.extend(duration.to_le_bytes());
            let rects = if version == 0 { 1 } else { 4 };
            for _ in 0..rects {
                for v in rect {
                    b.extend(v.to_le_bytes());
                }
            }
        }
    }
    b
}

const A: [f32; 4] = [0.0, 0.0, 0.25, 1.0];
const B: [f32; 4] = [0.25, 0.0, 0.5, 1.0];
const C: [f32; 4] = [0.5, 0.0, 0.75, 1.0];

#[test]
fn a_single_frame_sequence_has_one_sample() {
    let s = Sheet::parse(&blob(1, &[(3, true, 1.0, vec![(1.0, A)])])).unwrap();
    let sample = s.sample(3, 500);
    assert_eq!((sample.a, sample.b, sample.blend), (A, A, 1.0));
    assert_eq!(s.sample(0, 0).a, A);
    let two = Sheet::parse(&blob(
        1,
        &[
            (5, true, 1.0, vec![(1.0, A)]),
            (63, true, 1.0, vec![(1.0, B)]),
        ],
    ))
    .unwrap();
    assert_eq!(
        (two.sample(0, 0).a, two.sample(63, 0).a),
        (A, B),
        "filled from the first loaded"
    );
}

#[test]
fn frames_spread_over_1024_samples_by_duration() {
    let frames = vec![(1.0, A), (1.0, B), (2.0, C)];
    let s = Sheet::parse(&blob(0, &[(0, true, 4.0, frames)])).unwrap();
    assert_eq!(s.span(0), 4.0);
    let at = |i| {
        let x = s.sample(0, i);
        (x.a, x.b, x.blend)
    };
    assert_eq!(at(0), (A, A, 1.0), "before the first knot clamps");
    assert_eq!(at(128), (A, B, 0.5), "knots at 0, 256 and 512");
    assert_eq!(at(384), (B, C, 0.5));
    assert_eq!(at(768), (C, C, 1.0), "past the last knot clamps");
    let wraps = Sheet::parse(&blob(
        0,
        &[(0, false, 4.0, vec![(1.0, A), (1.0, B), (2.0, C)])],
    ))
    .unwrap();
    let x = wraps.sample(0, 768);
    assert_eq!(
        (x.a, x.b, x.blend),
        (C, A, 0.5),
        "wrapping runs from the last back to the first"
    );
    let x = wraps.sample(0, 0);
    assert_eq!(
        (x.a, x.b, x.blend),
        (C, A, 1.0),
        "the first knot is the end of the wrap gap"
    );
}

#[test]
fn bad_sheets_are_refused() {
    assert!(
        Sheet::parse(&blob(1, &[(64, true, 1.0, vec![(1.0, A)])])).is_err(),
        "sequence 64"
    );
    let mut cut = blob(1, &[(0, true, 1.0, vec![(1.0, A)])]);
    cut.truncate(cut.len() - 4);
    assert!(Sheet::parse(&cut).is_err());
    assert!(Sheet::parse(&blob(1, &[])).is_err(), "no sequences");
}
