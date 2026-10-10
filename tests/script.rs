use portalgun::script::{Action, Script};

#[test]
fn actions_come_due_once_in_time_order() {
    let mut s =
        Script::parse("# warm up\n1.0 fire1\n0.5 look 90 -10\n2 move 1 0\n2 jump\n3 fire2\n")
            .unwrap();
    assert!(s.due(0.4).is_empty());
    assert_eq!(s.due(0.5), vec![Action::Look(90.0, -10.0)]);
    assert_eq!(
        s.due(2.5),
        vec![Action::Fire1, Action::Move(1.0, 0.0), Action::Jump]
    );
    assert!(s.due(2.9).is_empty());
    assert_eq!(s.due(10.0), vec![Action::Fire2]);
    assert!(s.due(20.0).is_empty());
}

#[test]
fn a_bad_line_is_refused_with_its_number() {
    for (text, want) in [
        ("1 fire1\n2 dance", "line 2"),
        ("x fire1", "line 1"),
        ("1 look 90", "line 1"),
        ("1 move a 0", "line 1"),
        ("-1 fire1", "line 1"),
    ] {
        let e = Script::parse(text).unwrap_err();
        assert!(e.contains(want), "{text:?} gave {e}");
    }
}
