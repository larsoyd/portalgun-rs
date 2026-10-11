// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use portal::keyvalues::{parse, Kv};

const VMT: &str = r#"// leading comment
"VertexLitGeneric"
{
	"$baseTexture" "models\weapons\v_models\v_portalgun/v_portalgun"
//	"$translucent" "1"
	$phong 1
	"$phongfresnelranges"	"[5 1 2]"
	"<DX90"
	{
		"$PortalColorScale" "1.4"
	}
	"Proxies" { "CurrentTime" { "resultVar" "$time" } }
}
"#;

#[test]
fn reads_nested_blocks_quoted_and_bare_tokens() {
    let root = parse(VMT).unwrap();
    assert_eq!(root.len(), 1);
    let shader = &root[0];
    assert_eq!(shader.key, "VertexLitGeneric");
    assert_eq!(
        shader.get("$basetexture"),
        Some("models\\weapons\\v_models\\v_portalgun/v_portalgun")
    );
    assert_eq!(shader.get("$phong"), Some("1"));
    assert_eq!(shader.get("$translucent"), None);
    assert_eq!(shader.get("$phongfresnelranges"), Some("[5 1 2]"));
    assert_eq!(
        shader
            .block("<dx90")
            .and_then(|b| b.get("$portalcolorscale")),
        Some("1.4")
    );
    let proxy = shader
        .block("proxies")
        .and_then(|p| p.block("currenttime"))
        .unwrap();
    assert_eq!(proxy.get("resultvar"), Some("$time"));
}

#[test]
fn platform_conditions_after_values_are_skipped() {
    let root = parse("\"A\" { \"wave\" \"x.wav\" [$X360] \"vol\" \"0.5\" }").unwrap();
    assert_eq!(root[0].get("wave"), Some("x.wav"));
    assert_eq!(root[0].get("vol"), Some("0.5"));
}

#[test]
fn repeated_keys_are_all_kept() {
    let root = parse("\"S\" { \"rndwave\" { \"wave\" \"a.wav\" \"wave\" \"b.wav\" } }").unwrap();
    let waves: Vec<&str> = root[0].block("rndwave").unwrap().all("wave").collect();
    assert_eq!(waves, ["a.wav", "b.wav"]);
}

#[test]
fn unbalanced_braces_are_refused() {
    assert!(parse("\"A\" { \"b\" \"c\"").is_err());
    assert!(parse("\"A\" }").is_err());
    assert!(parse("\"A\" \"b\" }").is_err());
    assert!(parse("\"A\" \"unterminated").is_err());
    let _: Option<&Kv> = None;
}
