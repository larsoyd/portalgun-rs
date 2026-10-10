use std::path::Path;

use portal::install::{root, Install};
use portal::soundscript::{parse, Bank, Entry, Pitch};

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

fn bank() -> Bank {
    let dir = root(None, None, Path::new(&std::env::var("HOME").unwrap()));
    let install = Install::open(&dir).unwrap_or_else(|e| panic!("install at {dir:?}: {e}"));
    Bank::read(&install.search).unwrap()
}

#[test]
fn the_manifest_brings_in_the_player_portal_sounds() {
    let b = bank();
    let enter = b.get("PortalPlayer.EnterPortal").unwrap();
    assert_eq!(
        enter.waves,
        [
            "sound/player/portal_enter1.wav",
            "sound/player/portal_enter2.wav"
        ]
    );
    assert_eq!(enter.volume, (0.5, 0.5));
    assert_eq!(enter.pitch, Pitch::Range(95.0, 105.0));
    assert_eq!(enter.soundlevel.as_deref(), Some("SNDLVL_70db"));
    assert_eq!(b.get("PortalPlayer.ExitPortal").unwrap().waves.len(), 2);
    assert_eq!(
        b.get("Weapon_Portalgun.fire_blue").unwrap().volume,
        (0.7, 0.7)
    );
}

#[test]
fn every_manifest_script_is_found_in_the_paks_or_loose_hl2() {
    let b = bank();
    assert_eq!(b.scripts, 24);
    assert!(b.missing.is_empty(), "{:?}", b.missing);
    let step = b.get("Concrete.StepLeft").unwrap();
    assert_eq!(step.waves.len(), 2, "{:?}", step.waves);
}

#[test]
fn names_that_are_unknown_or_disagree_between_scripts_are_refused() {
    let b = bank();
    let unknown = b.get("Portal.NoSuchSound").unwrap_err().to_string();
    assert!(
        unknown.contains("unknown sound portal.nosuchsound"),
        "{unknown}"
    );
    let twice = b
        .get("Portal.PartyEscortAssociate")
        .unwrap_err()
        .to_string();
    assert!(twice.contains("ambiguous"), "{twice}");
}

#[test]
fn a_pick_spans_the_wave_volume_and_pitch_ranges() {
    let e = Entry {
        waves: vec!["a.wav".into(), "b.wav".into()],
        volume: (0.4, 0.6),
        pitch: Pitch::Range(95.0, 105.0),
        soundlevel: None,
    };
    let low = e.pick([0.0, 0.0, 0.0]);
    assert_eq!((low.wave, low.volume, low.pitch), (0, 0.4, 95.0));
    let high = e.pick([0.99, 0.5, 0.75]);
    assert_eq!(high.wave, 1);
    assert_eq!(e.pick([1.0, 1.0, 1.0]).wave, 1);
    assert!((high.volume - 0.5).abs() < 1e-6, "{}", high.volume);
    assert!((high.pitch - 102.5).abs() < 1e-4, "{}", high.pitch);
    let fixed = Entry {
        pitch: Pitch::Fixed(140.0),
        ..e
    };
    assert_eq!(fixed.pick([0.3, 0.3, 0.9]).pitch, 140.0);
}
