// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use portal::vmf::parse;

const TEXT: &str = r#"versioninfo
{
	"editorversion" "400"
}
world
{
	"id" "1"
	"classname" "worldspawn"
	solid
	{
		"id" "2"
		side
		{
			"id" "1"
			"plane" "(-64 64 64) (64 64 64) (64 -64 64)"
			"material" "CONCRETE/CONCRETE_MODULAR_FLOOR001A"
			"uaxis" "[1 0 0 16] 0.25"
			"vaxis" "[0 -1 0 -8] 0.5"
			"rotation" "0"
			"lightmapscale" "16"
			"smoothing_groups" "0"
		}
		side
		{
			"id" "2"
			"plane" "(-64 -64 0) (64 -64 0) (64 64 0)"
			"material" "concrete/concrete_modular_floor001a"
			"uaxis" "[1 0 0 0] 0.25"
			"vaxis" "[0 -1 0 0] 0.25"
		}
		editor
		{
			"color" "0 175 108"
		}
	}
}
entity
{
	"id" "10"
	"classname" "trigger_multiple"
	"targetname" "button_trigger"
	"origin" "-512 -928 152"
	"wait" "1"
	connections
	{
		"OnTrigger" "exit_door,Open,,0,-1"
		"OnEndTouchAll" "exit_door,Close,,0.5,1"
	}
	solid
	{
		"id" "11"
		side
		{
			"id" "3"
			"plane" "(0 0 8) (8 0 8) (8 -8 8)"
			"material" "TOOLS/TOOLSTRIGGER"
			"uaxis" "[1 0 0 0] 0.25"
			"vaxis" "[0 -1 0 0] 0.25"
		}
	}
}
entity
{
	"id" "12"
	"classname" "logic_relay"
	"targetname" "relay"
	connections
	{
		"OnTrigger" "lights^TurnOn^^0.25^-1"
	}
}
"#;

/// Hammer 4.x writes the escape character between output fields
fn map() -> String {
    TEXT.replace('^', "\u{1b}")
}

#[test]
fn reads_world_sides_with_outward_planes_and_texture_axes() {
    let vmf = parse(&map()).unwrap();
    assert_eq!(vmf.world.len(), 1);
    let sides = &vmf.world[0].sides;
    assert_eq!(sides.len(), 2);
    assert_eq!(sides[0].plane.normal, [0.0, 0.0, 1.0]);
    assert_eq!(sides[0].plane.dist, 64.0);
    assert_eq!(sides[1].plane.normal, [0.0, 0.0, -1.0]);
    assert_eq!(sides[1].plane.dist, 0.0);
    assert_eq!(sides[0].material, "concrete/concrete_modular_floor001a");
    assert_eq!(sides[0].u.axis, [1.0, 0.0, 0.0]);
    assert_eq!((sides[0].u.shift, sides[0].u.scale), (16.0, 0.25));
    assert_eq!(sides[0].v.axis, [0.0, -1.0, 0.0]);
    assert_eq!((sides[0].v.shift, sides[0].v.scale), (-8.0, 0.5));
}

#[test]
fn reads_entities_keys_brushes_and_outputs() {
    let vmf = parse(&map()).unwrap();
    assert_eq!(vmf.entities.len(), 2);
    let trigger = &vmf.entities[0];
    assert_eq!(trigger.classname, "trigger_multiple");
    assert_eq!(trigger.get("targetname"), Some("button_trigger"));
    assert_eq!(trigger.get("WAIT"), Some("1"));
    assert_eq!(trigger.origin(), Some([-512.0, -928.0, 152.0]));
    assert_eq!(trigger.solids.len(), 1);
    assert_eq!(trigger.solids[0].sides[0].material, "tools/toolstrigger");
    let close = &trigger.outputs[1];
    assert_eq!(close.event, "OnEndTouchAll");
    assert_eq!(close.target, "exit_door");
    assert_eq!(close.input, "Close");
    assert_eq!(close.param, "");
    assert_eq!((close.delay, close.times), (0.5, 1));
    assert_eq!(trigger.outputs[0].times, -1);
    let relay = &vmf.entities[1];
    assert_eq!(relay.origin(), None);
    let on = &relay.outputs[0];
    assert_eq!(
        (on.target.as_str(), on.input.as_str()),
        ("lights", "TurnOn")
    );
    assert_eq!((on.delay, on.times), (0.25, -1));
}

#[test]
fn refuses_a_side_whose_points_make_no_plane() {
    let bad = map().replace(
        "(-64 64 64) (64 64 64) (64 -64 64)",
        "(0 0 0) (1 1 1) (2 2 2)",
    );
    let err = parse(&bad).unwrap_err().to_string();
    assert!(err.contains("plane"), "{err}");
}

#[test]
fn refuses_a_side_without_material() {
    let bad = map().replace("\"material\" \"concrete/concrete_modular_floor001a\"", "");
    let err = parse(&bad).unwrap_err().to_string();
    assert!(err.contains("material"), "{err}");
}

#[test]
fn refuses_an_output_with_missing_fields() {
    let bad = map().replace("exit_door,Close,,0.5,1", "exit_door,Close");
    let err = parse(&bad).unwrap_err().to_string();
    assert!(err.contains("OnEndTouchAll"), "{err}");
}
