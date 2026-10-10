use portal::gun_fx::{GunFx, Interp, State, SPRITE_SCALE};

fn sprite<'a>(
    s: &'a [portal::gun_fx::Sprite],
    attachment: &str,
    material: &str,
) -> Option<&'a portal::gun_fx::Sprite> {
    s.iter()
        .find(|x| x.attachment == attachment && x.material == material)
}

/// a fixed sequence standing in for Source's random stream
fn rng() -> impl FnMut() -> f32 {
    let mut i = 0u32;
    move || {
        i += 1;
        (i % 4) as f32 / 4.0
    }
}

#[test]
fn interpolated_values_ramp_linearly_and_hold() {
    let mut v = Interp::default();
    v.set_absolute(10.0, 1.0);
    assert_eq!(v.value(5.0), 10.0);
    v.init_from_current(30.0, 0.2, 2.0);
    assert_eq!(v.value(2.0), 10.0);
    assert!((v.value(2.1) - 20.0).abs() < 1e-4);
    assert_eq!(v.value(3.0), 30.0);
    v.init_from_current(5.0, 0.0, 3.0);
    assert_eq!(v.value(3.0), 5.0, "no time jumps straight to the end");
}

#[test]
fn a_holstered_gun_shows_nothing() {
    let mut fx = GunFx::new(0.0);
    fx.set_state(State::Holding, Some(1), 0.0);
    fx.set_state(State::None, Some(1), 0.0);
    fx.think(Some(1), 0.016, 0.016, &mut rng());
    assert!(fx.sprites(0.016).is_empty());
}

#[test]
fn ready_shows_arm_glows_muzzle_glow_and_tube_sprites_but_no_end_caps() {
    let mut fx = GunFx::new(0.0);
    fx.set_state(State::Ready, None, 0.0);
    fx.think(None, 0.016, 0.016, &mut rng());
    let s = fx.sprites(0.5);
    for a in [
        "Arm1_attach1",
        "Arm1_attach2",
        "Arm2_attach1",
        "Arm2_attach2",
        "Arm3_attach1",
        "Arm3_attach2",
    ] {
        let g = sprite(&s, a, "sprites/glow04_noz").unwrap_or_else(|| panic!("{a}"));
        assert_eq!(g.color, [255.0, 128.0, 0.0]);
        assert!(g.alpha >= 10.0 && g.alpha <= 24.0, "{}", g.alpha);
    }
    for a in ["Beam_point1", "Beam_point5"] {
        assert!(sprite(&s, a, "sprites/portalgun_effects").is_some());
    }
    assert!(sprite(&s, "Inside_effects", "sprites/portalgun_effects").is_some());
    assert!(sprite(&s, "Arm1_attach3", "sprites/grav_flare").is_none());
    assert!(
        sprite(&s, "Body_light", "sprites/bluelight").is_none(),
        "nothing fired yet"
    );
}

#[test]
fn the_body_light_and_inner_glows_take_the_last_fired_colour() {
    let mut fx = GunFx::new(0.0);
    fx.set_state(State::Ready, None, 0.0);
    fx.think(Some(2), 0.016, 0.016, &mut rng());
    let s = fx.sprites(0.016);
    assert!(sprite(&s, "Body_light", "sprites/orangelight").is_some());
    assert!(sprite(&s, "Body_light", "sprites/bluelight").is_none());
    let light = sprite(&s, "Body_light", "sprites/orangelight").unwrap();
    assert_eq!((light.size, light.alpha), (0.018 * SPRITE_SCALE, 128.0));
    assert_eq!(
        sprite(&s, "Inside_effects", "sprites/portalgun_effects")
            .unwrap()
            .color,
        [255.0, 160.0, 32.0]
    );
    assert_eq!(
        sprite(&s, "Beam_point3", "sprites/portalgun_effects")
            .unwrap()
            .color,
        [255.0, 160.0, 32.0]
    );
    fx.think(None, 0.016, 0.032, &mut rng());
    assert_eq!(
        sprite(
            &fx.sprites(0.032),
            "Inside_effects",
            "sprites/portalgun_effects"
        )
        .unwrap()
        .color,
        [128.0; 3]
    );
}

#[test]
fn the_inner_glow_pulses_between_its_sizes_once_a_second() {
    let mut fx = GunFx::new(0.0);
    fx.set_state(State::Ready, Some(1), 0.0);
    let mut r = rng();
    let mut sizes = Vec::new();
    for i in 1..=200 {
        let now = i as f32 * 0.01;
        fx.think(Some(1), 0.01, now, &mut r);
        let glow = *sprite(
            &fx.sprites(now),
            "Inside_effects",
            "sprites/portalgun_effects",
        )
        .unwrap();
        sizes.push((glow.size, glow.alpha));
    }
    let (min, max) = sizes.iter().fold((f32::MAX, f32::MIN), |(a, b), (s, _)| {
        (a.min(*s), b.max(*s))
    });
    assert!(
        (min - 3.0).abs() < 0.05 && (max - 4.0).abs() < 0.05,
        "{min} {max}"
    );
    assert!(
        (sizes[99].0 - 4.0).abs() < 0.05,
        "full at one second {:?}",
        sizes[99]
    );
    // alpha is cast to an integer as Source does, so 127.7 shows as 127
    assert!(
        (sizes[99].1 - 127.0).abs() < 1.0 && (sizes[199].1 - 96.0).abs() < 1.0,
        "{:?} {:?}",
        sizes[99],
        sizes[199]
    );
}

#[test]
fn every_view_model_effect_has_a_slot() {
    let fx = GunFx::new(0.0);
    assert_eq!(
        fx.capacity(),
        3 + 6 + 3 + 1 + 5,
        "three lights, six glows, three caps, the inner glow, five tube sprites"
    );
}

#[test]
fn holding_adds_the_end_caps() {
    let mut fx = GunFx::new(0.0);
    fx.set_state(State::Holding, Some(1), 0.0);
    fx.think(Some(1), 0.016, 0.016, &mut rng());
    let s = fx.sprites(0.016);
    for a in ["Arm1_attach3", "Arm2_attach3", "Arm3_attach3"] {
        let cap = sprite(&s, a, "sprites/grav_flare").unwrap();
        assert!(
            cap.size >= 1.0 && cap.size <= 3.0 && cap.alpha >= 96.0 && cap.alpha <= 128.0,
            "{cap:?}"
        );
    }
}

#[test]
fn glow_flicker_spans_source_random_ranges() {
    for (draw, alpha, cap_alpha) in [(0.0, 10.0, 96.0), (0.9999, 24.0, 128.0)] {
        let mut fx = GunFx::new(0.0);
        fx.set_state(State::Holding, None, 0.0);
        fx.think(None, 0.016, 0.016, &mut || draw);
        let s = fx.sprites(0.016);
        assert_eq!(
            sprite(&s, "Arm2_attach1", "sprites/glow04_noz")
                .unwrap()
                .alpha,
            alpha
        );
        assert_eq!(
            sprite(&s, "Arm2_attach3", "sprites/grav_flare")
                .unwrap()
                .alpha,
            cap_alpha
        );
    }
}
