use portal::soundscript::{parse, Pitch};

const SCRIPT: &str = r#"
"Weapon_Portalgun.fire_blue"
{
	"channel"		"CHAN_WEAPON"
	"volume"		"0.7"
	"soundlevel"	"SNDLVL_GUNFIRE"
	"pitch"			"95,105"
	"wave"			")weapons/portalgun/portalgun_shoot_blue1.wav"
}
"Portal.open_red"
{
	"channel"	"CHAN_STATIC"
	"volume"	"VOL_NORM"
	"pitch"		"PITCH_NORM"
	"soundlevel"	"SNDLVL_80dB"
	"rndwave"
	{
		"wave"	"weapons/portalgun/portal_open2.wav"
		"wave"	"*weapons/portalgun/portal_open3.wav"
	}
}
"Portal.ambient_loop"
{
	"volume"	"0.15, 0.25"
	"pitch"		"110"
	"wave"		"^weapons\portalgun\portal_ambient_loop1.wav"
}
"#;

#[test]
fn entries_keep_their_waves_volume_and_pitch() {
    let s = parse(SCRIPT).unwrap();
    let fire = &s["weapon_portalgun.fire_blue"];
    assert_eq!(
        fire.waves,
        ["sound/weapons/portalgun/portalgun_shoot_blue1.wav"]
    );
    assert_eq!(fire.volume, (0.7, 0.7));
    assert_eq!(fire.pitch, Pitch::Range(95.0, 105.0));
    let open = &s["portal.open_red"];
    assert_eq!(
        open.waves,
        [
            "sound/weapons/portalgun/portal_open2.wav",
            "sound/weapons/portalgun/portal_open3.wav"
        ]
    );
    assert_eq!(open.volume, (1.0, 1.0));
    assert_eq!(open.pitch, Pitch::Fixed(100.0));
    let loop_ = &s["portal.ambient_loop"];
    assert_eq!(loop_.pitch, Pitch::Fixed(110.0));
    assert_eq!(loop_.volume, (0.15, 0.25));
    assert_eq!(
        loop_.waves,
        ["sound/weapons/portalgun/portal_ambient_loop1.wav"]
    );
}

#[test]
fn an_entry_without_a_wave_is_refused() {
    assert!(parse("\"A\" { \"volume\" \"1\" }").is_err());
    assert!(parse("\"A\" { \"wave\" \"a.wav\" \"volume\" \"loud\" }").is_err());
}
