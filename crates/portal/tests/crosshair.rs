// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use portal::crosshair::{
    font_tall, icon_uv, icons, paint, reticle_at, scale, GunState, Icon, Rings, Sprite,
};
use portal::keyvalues;

const SIZES: [(i32, i32); 5] = [(44, 64), (44, 64), (44, 64), (44, 64), (28, 64)];

fn both() -> GunState {
    GunState {
        can_fire: [true, true],
        placable: [1.0, 1.0],
        last_fired: 0,
        holding: false,
    }
}

fn draw(gun: &GunState, rings: &mut Rings, dt: f32) -> Vec<Sprite> {
    paint(1920, 1080, &SIZES, gun, rings, dt)
}

#[test]
fn the_hud_scale_steps_every_1600_lines() {
    assert_eq!(scale(1080), 1.0);
    assert_eq!(scale(1599), 1.0);
    assert_eq!(scale(1600), 2.0);
    assert_eq!(scale(3199), 2.0);
    assert_eq!(scale(3200), 3.0);
}

#[test]
fn brackets_sit_around_the_centre_at_1080p() {
    let s = draw(&both(), &mut Rings::default(), 0.0);
    assert_eq!(s.len(), 4);
    assert_eq!(s[0].icon, Icon::LeftValid);
    assert_eq!(s[0].rect, [931, 497, 44, 64]);
    assert_eq!(s[1].icon, Icon::RightValid);
    assert_eq!(s[1].rect, [944, 518, 44, 64]);
    assert_eq!(s[2].icon, Icon::LastPlaced);
    assert_eq!(s[2].rect, [908, 508, 28, 64]);
    assert_eq!(s[3].rect, [981, 508, 28, 64]);
    let odd = paint(1921, 1081, &SIZES, &both(), &mut Rings::default(), 0.0);
    assert_eq!(
        odd[0].rect,
        [931, 497, 44, 64],
        "half the width is whole pixels"
    );
}

#[test]
fn brackets_grow_with_the_scale() {
    let s = paint(3840, 2160, &SIZES, &both(), &mut Rings::default(), 0.0);
    assert_eq!(s[0].rect, [1863, 994, 88, 128]);
    assert_eq!(s[1].rect, [1889, 1037, 88, 128]);
    assert_eq!(s[2].rect, [1816, 1016, 56, 128]);
    assert_eq!(s[3].rect, [1962, 1016, 56, 128]);
    let tall = paint(3200, 4800, &SIZES, &both(), &mut Rings::default(), 0.0);
    assert_eq!(
        tall[3].rect[0], 1684,
        "28 by 4 by 0.75 lands on a whole pixel"
    );
}

#[test]
fn a_bracket_fills_only_above_half_placable() {
    let mut gun = both();
    gun.placable = [0.5, 0.51];
    let s = draw(&gun, &mut Rings::default(), 0.0);
    assert_eq!(s[0].icon, Icon::LeftInvalid);
    assert_eq!(s[1].icon, Icon::RightValid);
}

#[test]
fn brackets_wear_the_portal_colours() {
    let s = draw(&both(), &mut Rings::default(), 0.0);
    assert_eq!(s[0].colour, [64, 160, 255, 150]);
    assert_eq!(s[1].colour, [255, 160, 32, 150]);
    assert_eq!(&s[2].colour[..3], &[64, 160, 255]);
    assert_eq!(&s[3].colour[..3], &[255, 160, 32]);
}

#[test]
fn with_one_colour_both_brackets_copy_it_and_the_rings_hide() {
    let mut gun = both();
    gun.can_fire = [true, false];
    gun.placable = [1.0, 0.0];
    gun.last_fired = 1;
    let s = draw(&gun, &mut Rings::default(), 0.1);
    assert_eq!(s[1].colour, [64, 160, 255, 150]);
    assert_eq!(s[1].icon, Icon::RightValid);
    assert_eq!(s[2].colour[3], 0);
    assert_eq!(s[3].colour[3], 0);
    let mut orange = both();
    orange.can_fire = [false, true];
    orange.placable = [1.0, 0.0];
    let s = draw(&orange, &mut Rings::default(), 0.1);
    assert_eq!(s[0].colour, [255, 160, 32, 150]);
    assert_eq!(s[0].icon, Icon::LeftInvalid);
}

#[test]
fn rings_start_at_80_counting_up() {
    let r = Rings::default();
    assert_eq!(r.alpha, [80.0, 80.0]);
    assert_eq!(r.up, [true, true]);
}

#[test]
fn the_last_fired_ring_rises_then_reflects_off_255() {
    let mut gun = both();
    gun.last_fired = 2;
    let mut r = Rings::default();
    draw(&gun, &mut r, 0.25);
    assert_eq!(r.alpha[1], 230.0);
    assert!(r.up[1]);
    draw(&gun, &mut r, 0.05);
    assert_eq!(r.alpha[1], 250.0);
    assert!(!r.up[1], "past 255 it turns");
    let mut at = Rings {
        alpha: [0.0, 255.0],
        up: [true, true],
    };
    draw(&gun, &mut at, 0.0);
    assert!(at.up[1], "255 itself keeps rising");
}

#[test]
fn a_falling_ring_floors_at_128() {
    let mut gun = both();
    gun.last_fired = 1;
    let mut r = Rings {
        alpha: [200.0, 0.0],
        up: [false, true],
    };
    let s = draw(&gun, &mut r, 0.1);
    assert_eq!(r.alpha[0], 170.0);
    assert_eq!(s[2].colour[3], 170);
    draw(&gun, &mut r, 1.0);
    assert_eq!(r.alpha[0], 128.0);
}

#[test]
fn the_other_ring_fades_to_nothing_and_rearms() {
    let mut gun = both();
    gun.last_fired = 1;
    let mut r = Rings {
        alpha: [100.0, 100.0],
        up: [true, false],
    };
    draw(&gun, &mut r, 0.1);
    assert_eq!(r.alpha[1], 70.0);
    assert!(r.up[1]);
    draw(&gun, &mut r, 1.0);
    assert_eq!(r.alpha[1], 0.0);
}

#[test]
fn before_any_shot_the_rings_are_invisible_but_fade() {
    let mut r = Rings::default();
    let s = draw(&both(), &mut r, 0.1);
    assert_eq!(s[2].colour[3], 0);
    assert_eq!(s[3].colour[3], 0);
    assert_eq!(r.alpha, [140.0, 50.0]);
}

#[test]
fn holding_something_shows_both_brackets_hollow_and_tan() {
    let mut gun = both();
    gun.holding = true;
    let s = draw(&gun, &mut Rings::default(), 0.0);
    assert_eq!(s[0].icon, Icon::LeftInvalid);
    assert_eq!(s[1].icon, Icon::RightInvalid);
    assert_eq!(s[0].colour, [242, 202, 167, 255]);
    assert_eq!(s[1].colour, [242, 202, 167, 255]);
}

#[test]
fn icon_coordinates_are_inset_half_a_texel() {
    assert_eq!(
        icon_uv([98, 0, 44, 64], (256, 64)),
        [98.5 / 256.0, 0.5 / 64.0, 141.5 / 256.0, 63.5 / 64.0]
    );
}

#[test]
fn icons_come_from_the_texture_data_by_name() {
    let text = r#""sprites/640_hud"
{
    TextureData
    {
        "portal_crosshair_left_invalid" { "file" "sprites/hud/portal_crosshairs" "x" "2" "y" "0" "width" "44" "height" "64" }
        "portal_crosshair_right_invalid" { "file" "sprites/hud/portal_crosshairs" "x" "50" "y" "0" "width" "44" "height" "64" }
        "portal_crosshair_left_valid" { "file" "sprites/hud/portal_crosshairs" "x" "98" "y" "0" "width" "44" "height" "64" }
        "portal_crosshair_right_valid" { "file" "sprites/hud/portal_crosshairs" "x" "146" "y" "0" "width" "44" "height" "64" }
        "portal_crosshair_last_placed" { "file" "sprites/hud/portal_crosshairs" "x" "194" "y" "0" "width" "28" "height" "64" }
    }
}"#;
    let root = keyvalues::parse(text).unwrap();
    let (file, rects) = icons(&root).unwrap();
    assert_eq!(file, "sprites/hud/portal_crosshairs");
    assert_eq!(rects[Icon::LeftValid as usize], [98, 0, 44, 64]);
    assert_eq!(rects[Icon::LastPlaced as usize], [194, 0, 28, 64]);
    let missing = keyvalues::parse(r#""x" { TextureData { } }"#).unwrap();
    assert!(icons(&missing)
        .unwrap_err()
        .contains("portal_crosshair_left_invalid"));
}

#[test]
fn the_reticle_font_size_follows_the_screen_height() {
    let text = r#"Scheme { Fonts { Crosshairs {
        "1" { "name" "HalfLife2" "tall" "40" [!$OSX] "tall" "41" [$OSX] "yres" "1 1599" [!$DECK] "yres" "1 1439" [$DECK] }
        "2" { "name" "HalfLife2" "tall" "80" "yres" "1600 3199" [!$DECK] "yres" "1440 3199" [$DECK] }
        "3" { "name" "HalfLife2" "tall" "120" "yres" "3200 4799" }
        "4" { "name" "HalfLife2" "tall" "17" }
    } } }"#;
    let root = keyvalues::parse(text).unwrap();
    assert_eq!(font_tall(&root, 1080), Ok(40.0));
    assert_eq!(font_tall(&root, 1599), Ok(40.0));
    assert_eq!(font_tall(&root, 1600), Ok(80.0));
    assert_eq!(font_tall(&root, 4000), Ok(120.0));
    assert!(font_tall(&root, 5000).is_err());
}

#[test]
fn the_reticle_cell_is_centred_with_integer_halves() {
    assert_eq!(reticle_at(1920, 1080, 25, 40), [948, 520]);
    assert_eq!(reticle_at(1921, 1081, 25, 41), [949, 521]);
}
