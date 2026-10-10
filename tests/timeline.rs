use portalgun::run::Timeline;

#[test]
fn each_shot_fires_once_on_the_first_tick_at_or_past_its_time() {
    let mut t = Timeline::new(vec![2.0, 0.5], None);
    assert!(t.shots_due(0.49).is_empty());
    assert_eq!(t.shots_due(0.5), vec![0.5]);
    assert!(t.shots_due(1.0).is_empty());
    assert_eq!(t.shots_due(5.0), vec![2.0]);
    assert!(t.shots_due(9.0).is_empty());
}

#[test]
fn quits_from_the_quit_time_on_and_never_without_one() {
    let t = Timeline::new(vec![], Some(4.0));
    assert!(!t.should_quit(3.99));
    assert!(t.should_quit(4.0));
    assert!(!Timeline::new(vec![], None).should_quit(1e9));
}
