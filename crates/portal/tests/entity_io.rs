// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use portal::entity::io::{name_matches, Action, Handle, OutputEvent, Queue, Target};
use portal::vmf::Output;

fn out(target: &str, input: &str, param: &str, delay: f32, times: i32) -> Output {
    Output {
        event: "OnTrigger".into(),
        target: target.into(),
        input: input.into(),
        param: param.into(),
        delay,
        times,
    }
}

fn fired(queue: &mut Queue, now: f32) -> Vec<(String, String, Option<String>, f32)> {
    std::iter::from_fn(|| queue.pop_due(now))
        .map(|e| {
            let Target::Name(name) = e.target else {
                panic!("handle target")
            };
            (name, e.input, e.value, e.time)
        })
        .collect()
}

const ME: Handle = Handle(7);

#[test]
fn an_empty_input_becomes_use_and_zero_times_fires_forever() {
    let a = Action::new(&out("door", "", "", 0.0, 0));
    assert_eq!((a.input.as_str(), a.param, a.times), ("Use", None, -1));
    let b = Action::new(&out("door", "Open", "1", 0.5, 2));
    assert_eq!(
        (b.input.as_str(), b.param.as_deref(), b.times),
        ("Open", Some("1"), 2)
    );
}

#[test]
fn actions_fire_in_reverse_of_their_keyvalue_order() {
    let mut output =
        OutputEvent::new([out("a", "Open", "", 0.0, -1), out("b", "Open", "", 0.0, -1)].iter());
    let mut queue = Queue::default();
    output.fire(&mut queue, 1.0, None, None, Some(ME), 0.0);
    let names: Vec<String> = fired(&mut queue, 1.0).into_iter().map(|f| f.0).collect();
    assert_eq!(names, ["b", "a"]);
}

#[test]
fn equal_fire_times_queue_after_the_events_already_waiting() {
    let mut queue = Queue::default();
    for name in ["first", "second", "third"] {
        queue.add(
            Target::Name(name.into()),
            "Trigger",
            None,
            2.0,
            0.5,
            None,
            Some(ME),
        );
    }
    queue.add(
        Target::Name("early".into()),
        "Trigger",
        None,
        2.0,
        0.25,
        None,
        Some(ME),
    );
    let names: Vec<String> = fired(&mut queue, 2.5).into_iter().map(|f| f.0).collect();
    assert_eq!(names, ["early", "first", "second", "third"]);
}

#[test]
fn an_event_fires_once_curtime_reaches_its_time() {
    let mut queue = Queue::default();
    queue.add(
        Target::Name("relay".into()),
        "Trigger",
        None,
        1.0,
        0.5,
        None,
        None,
    );
    assert!(queue.pop_due(1.4999).is_none());
    assert!(queue.pop_due(1.5).is_some());
}

#[test]
fn the_fire_delay_only_adds_to_actions_without_a_parameter() {
    let mut output = OutputEvent::new(
        [
            out("plain", "Open", "", 0.25, -1),
            out("param", "Skin", "3", 0.25, -1),
        ]
        .iter(),
    );
    let mut queue = Queue::default();
    output.fire(&mut queue, 10.0, Some("v"), None, Some(ME), 1.0);
    let mut got = fired(&mut queue, 20.0);
    got.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(
        got,
        [
            ("param".into(), "Skin".into(), Some("3".into()), 10.25),
            ("plain".into(), "Open".into(), Some("v".into()), 11.25),
        ]
    );
}

#[test]
fn a_limited_action_is_dropped_after_its_last_fire() {
    let mut output = OutputEvent::new(
        [
            out("once", "Open", "", 0.0, 1),
            out("twice", "Open", "", 0.0, 2),
        ]
        .iter(),
    );
    let mut queue = Queue::default();
    let names =
        |queue: &mut Queue| -> Vec<String> { fired(queue, 0.0).into_iter().map(|f| f.0).collect() };
    output.fire(&mut queue, 0.0, None, None, Some(ME), 0.0);
    assert_eq!(names(&mut queue), ["twice", "once"]);
    output.fire(&mut queue, 0.0, None, None, Some(ME), 0.0);
    assert_eq!(names(&mut queue), ["twice"]);
    output.fire(&mut queue, 0.0, None, None, Some(ME), 0.0);
    assert!(names(&mut queue).is_empty());
}

#[test]
fn max_delay_is_the_largest_action_delay_or_zero() {
    let output = OutputEvent::new(
        [
            out("a", "x", "", 0.5, -1),
            out("b", "x", "", 2.0, -1),
            out("c", "x", "", 1.0, -1),
        ]
        .iter(),
    );
    assert_eq!(output.max_delay(), 2.0);
    assert_eq!(
        OutputEvent::new([out("a", "x", "", -1.0, -1)].iter()).max_delay(),
        0.0
    );
}

#[test]
fn cancel_drops_only_events_sent_by_that_caller() {
    let mut queue = Queue::default();
    queue.add(
        Target::Name("mine".into()),
        "Trigger",
        None,
        0.0,
        1.0,
        None,
        Some(ME),
    );
    queue.add(
        Target::Name("theirs".into()),
        "Trigger",
        None,
        0.0,
        1.0,
        Some(ME),
        Some(Handle(3)),
    );
    queue.cancel(ME);
    let names: Vec<String> = fired(&mut queue, 5.0).into_iter().map(|f| f.0).collect();
    assert_eq!(names, ["theirs"]);
}

#[test]
fn names_match_without_case_and_with_a_trailing_star() {
    assert!(name_matches(Some("Exit_Door_A00"), "exit_door_a00"));
    assert!(name_matches(Some("exit_door_a00"), "EXIT_DOOR_A00"));
    assert!(name_matches(Some("exit_door_left"), "exit_door*"));
    assert!(name_matches(Some("box"), "box*"));
    assert!(!name_matches(Some("box"), "boxes"));
    assert!(!name_matches(Some("boxes"), "box"));
    assert!(!name_matches(Some("box"), "bo"));
    assert!(name_matches(None, "*"));
    assert!(name_matches(None, ""));
    assert!(!name_matches(None, "box"));
}

#[test]
fn name_case_folding_is_a_plain_32_offset_as_in_the_original() {
    assert!(name_matches(Some("door_0"), "door_P"));
    assert!(!name_matches(Some("door_{"), "door_["));
}
