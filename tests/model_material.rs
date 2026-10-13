// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use portalgun::model_material::{exponent, fresnel, Phong};

#[test]
fn phong_uses_the_texture_exponent_and_two_fresnel_intervals() {
    assert_eq!(exponent(0.0), 1.0);
    assert_eq!(exponent(1.0), 150.0);
    assert_eq!(exponent(0.5), 75.5);
    assert_eq!(fresnel(1.0, [5.0, 1.0, 2.0]), 5.0);
    assert_eq!(fresnel(0.0, [5.0, 1.0, 2.0]), 2.0);
    assert!((fresnel(1.0 - 0.5f32.sqrt(), [5.0, 1.0, 2.0]) - 1.0).abs() < 1e-6);
    assert_eq!(fresnel(-1.0, [5.0, 1.0, 2.0]), 2.0);
}

#[test]
fn gun_material_keeps_normal_exponent_warp_and_fresnel_parameters() {
    let vmt = portal::keyvalues::parse(
        r#"VertexLitGeneric {
        "$bumpmap" "models\gun_normal"
        "$phongexponenttexture" "models/gun_exponent"
        "$lightwarptexture" "models/gun_warp"
        "$phong" "1" "$phongboost" "3" "$phongfresnelranges" "[5 1 2]"
        "$selfillum" "1"
    }"#,
    )
    .unwrap();
    let p = Phong::parse(&vmt[0]).unwrap();
    assert!(p.enabled);
    assert_eq!(p.normal.as_deref(), Some("models/gun_normal"));
    assert_eq!(p.exponent.as_deref(), Some("models/gun_exponent"));
    assert_eq!(p.warp.as_deref(), Some("models/gun_warp"));
    assert_eq!(p.boost, 3.0);
    assert_eq!(p.fresnel, [5.0, 1.0, 2.0]);
    assert!(p.self_illum);
}
