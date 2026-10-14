// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use portal::physics_sound::{
    collision_volume, impact_sound, scrape_sound, slip_energy, ImpactQueue, Scrape, ScrapeStep,
};
use portal::surfaces::Surface;

fn metal() -> Surface {
    Surface {
        impact_hard: "SolidMetal.ImpactHard".into(),
        impact_soft: "SolidMetal.ImpactSoft".into(),
        scrape_rough: "SolidMetal.ScrapeRough".into(),
        scrape_smooth: "SolidMetal.ScrapeSmooth".into(),
        hardness: 1.0,
        roughness: 0.1,
        scrape_rough_threshold: 0.5,
        impact_hard_threshold: 0.5,
        ..Surface::default()
    }
}

#[test]
fn impacts_sound_from_seventy_units_a_second_and_a_twentieth_apart() {
    assert_eq!(collision_volume(69.99, 1.0), None);
    assert_eq!(collision_volume(70.0, 1.0), Some(70.0 * 70.0 / 102400.0));
    assert_eq!(collision_volume(200.0, 0.0499), None);
    assert!(collision_volume(200.0, 0.05).is_some());
    assert_eq!(collision_volume(320.0, 1.0), Some(1.0));
    assert_eq!(collision_volume(400.0, 1.0), Some(1.0));
}

#[test]
fn hard_surfaces_get_the_hard_impact_and_soft_ones_the_soft() {
    let own = metal();
    let mut soft = metal();
    soft.hardness = 0.49;
    assert_eq!(
        impact_sound(&own, &metal(), 100.0),
        Some("SolidMetal.ImpactHard")
    );
    assert_eq!(
        impact_sound(&own, &soft, 100.0),
        Some("SolidMetal.ImpactSoft")
    );
    soft.hardness = 0.5;
    assert_eq!(
        impact_sound(&own, &soft, 100.0),
        Some("SolidMetal.ImpactHard")
    );
    let mut quick = metal();
    quick.hard_min_velocity = 150.0;
    assert_eq!(
        impact_sound(&quick, &metal(), 149.0),
        Some("SolidMetal.ImpactSoft")
    );
    assert_eq!(
        impact_sound(&quick, &metal(), 150.0),
        Some("SolidMetal.ImpactHard")
    );
    assert_eq!(impact_sound(&Surface::default(), &metal(), 100.0), None);
}

#[test]
fn queued_impacts_of_one_surface_merge_and_flush_after_a_twentieth() {
    let mut q = ImpactQueue::default();
    q.add(0, 1, 0.3, 100.0, [1.0, 0.0, 0.0]);
    q.add(0, 2, 0.5, 90.0, [2.0, 0.0, 0.0]);
    q.add(3, 1, 0.2, 80.0, [3.0, 0.0, 0.0]);
    assert!(q.flush(0.05).is_empty());
    let out = q.flush(0.001);
    assert_eq!(out.len(), 2);
    let merged = out.iter().find(|e| e.own == 0).unwrap();
    assert!((merged.volume - 0.8).abs() < 1e-6);
    assert_eq!(merged.origin, [2.0, 0.0, 0.0]);
    assert_eq!(merged.hit, 2);
    assert!((merged.speed - 100.0001).abs() < 1e-3);
    assert!(q.flush(1.0).is_empty());
}

#[test]
fn a_flush_caps_each_volume_at_one() {
    let mut q = ImpactQueue::default();
    q.add(0, 1, 0.9, 300.0, [0.0; 3]);
    q.add(0, 1, 0.9, 300.0, [0.0; 3]);
    assert_eq!(q.flush(0.1)[0].volume, 1.0);
}

#[test]
fn past_five_queued_surfaces_everything_merges_into_the_last() {
    let mut q = ImpactQueue::default();
    for own in 0..5 {
        q.add(own, 0, 0.1, 100.0, [0.0; 3]);
    }
    q.add(9, 0, 0.1, 100.0, [0.0; 3]);
    let out = q.flush(0.1);
    assert_eq!(out.len(), 5);
    assert!((out.iter().find(|e| e.own == 4).unwrap().volume - 0.2).abs() < 1e-6);
}

#[test]
fn slip_work_becomes_scrape_energy_past_a_twentieth() {
    assert_eq!(slip_energy(0.0, 0.025, 0.015), None);
    assert_eq!(slip_energy(0.05 * 0.015 / 0.025, 0.025, 0.015), None);
    let e = slip_energy(1.0, 0.025, 0.015).unwrap();
    assert!((e - 1.0 / 0.015 * 0.025 * 1550.0032).abs() < 1e-3, "{e}");
}

#[test]
fn scrapes_need_seventy_five_and_go_smooth_on_smooth_surfaces() {
    let own = metal();
    let mut rough = metal();
    rough.roughness = 1.0;
    assert_eq!(scrape_sound(&own, &rough, 74.9), None);
    let (name, volume) = scrape_sound(&own, &rough, 7750.0).unwrap();
    assert_eq!(name, "SolidMetal.ScrapeRough");
    assert!((volume - 0.25).abs() < 1e-6);
    assert_eq!(
        scrape_sound(&own, &metal(), 7750.0).unwrap().0,
        "SolidMetal.ScrapeSmooth"
    );
    rough.roughness = 0.5;
    assert_eq!(
        scrape_sound(&own, &rough, 7750.0).unwrap().0,
        "SolidMetal.ScrapeRough"
    );
}

#[test]
fn a_scrape_starts_loud_enough_retargets_every_half_second_and_stops_a_tenth_after() {
    let mut s = Scrape::default();
    assert_eq!(
        s.friction(0.0, Some(0.19), 0.5, (95.0, 105.0)),
        ScrapeStep::Nothing
    );
    assert_eq!(
        s.friction(0.0, Some(0.0078), 1.0, (95.0, 105.0)),
        ScrapeStep::Nothing
    );
    assert_eq!(
        s.friction(0.0, Some(0.25), 0.5, (95.0, 105.0)),
        ScrapeStep::Start { volume: 0.125 }
    );
    assert_eq!(
        s.friction(0.4, Some(0.5), 0.5, (95.0, 105.0)),
        ScrapeStep::Keep
    );
    assert_eq!(
        s.friction(0.5, Some(0.5), 0.5, (95.0, 105.0)),
        ScrapeStep::Retarget {
            volume: 0.25,
            pitch: 100.0
        }
    );
    assert_eq!(s.friction(0.55, None, 0.5, (95.0, 105.0)), ScrapeStep::Keep);
    assert!(!s.expired(0.65));
    assert!(s.expired(0.6501));
    assert_eq!(
        s.friction(0.7, Some(0.25), 0.5, (95.0, 105.0)),
        ScrapeStep::Start { volume: 0.125 }
    );
}
