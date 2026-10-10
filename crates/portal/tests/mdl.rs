mod common;

use common::*;
use portal::mdl::Mdl;

fn gun() -> MdlDef {
    MdlDef {
        bones: vec![
            bone("Root", -1, [0.0, 0.0, 38.5]),
            bone("Base", 0, [1.0, 2.0, 3.0]),
        ],
        anims: vec![
            AnimDef {
                name: "@idle",
                fps: 30.0,
                flags: 1,
                frames: 1,
                data: anim_chain(&[]),
            },
            AnimDef {
                name: "@fire1",
                fps: 24.0,
                flags: 0,
                frames: 16,
                data: anim_chain(&[]),
            },
        ],
        seqs: vec![
            SeqDef {
                label: "idle",
                activity: "ACT_VM_IDLE",
                flags: 1,
                anim: 0,
                fade: [0.2, 0.2],
                events: vec![],
            },
            SeqDef {
                label: "fire1",
                activity: "ACT_VM_PRIMARYATTACK",
                flags: 2,
                anim: 1,
                fade: [0.1, 0.3],
                events: vec![(0.25, 5001, "1")],
            },
        ],
        textures: vec!["v_hands", "v_portalgun"],
        cd: vec!["models\\Weapons\\v_models\\v_portalgun\\"],
        meshes: vec![(0, 3, 0), (1, 4, 3)],
        attachments: vec![("muzzle", 1, [0.5, 0.0, 22.0])],
    }
}

#[test]
fn reads_the_skeleton() {
    let m = Mdl::parse(mdl(&gun())).unwrap();
    assert_eq!(m.version, 46);
    assert_eq!(m.bones.len(), 2);
    assert_eq!(m.bones[1].name, "Base");
    assert_eq!(m.bones[1].parent, Some(0));
    assert_eq!(m.bones[0].parent, None);
    assert_eq!(m.bones[1].pos, [1.0, 2.0, 3.0]);
    assert_eq!(m.bones[0].quat, [0.0, 0.0, 0.0, 1.0]);
    assert_eq!(m.bones[1].pose_to_bone[1], [0.0, 1.0, 0.0, -2.0]);
    assert_eq!(m.eye_position, [0.0, 0.0, 64.0]);
}

#[test]
fn reads_sequences_and_their_animations() {
    let m = Mdl::parse(mdl(&gun())).unwrap();
    let fire = m.sequence_for("ACT_VM_PRIMARYATTACK").unwrap();
    let s = &m.sequences[fire];
    assert_eq!(s.label, "fire1");
    assert!(!s.looping);
    assert_eq!((s.fade_in, s.fade_out), (0.1, 0.3));
    assert_eq!(s.anim, 1);
    assert_eq!(s.events.len(), 1);
    assert_eq!((s.events[0].cycle, s.events[0].event), (0.25, 5001));
    assert_eq!(s.events[0].options, "1");
    assert!(m.sequences[m.sequence_for("ACT_VM_IDLE").unwrap()].looping);
    let a = &m.anims[1];
    assert_eq!((a.name.as_str(), a.fps, a.frames), ("@fire1", 24.0, 16));
    assert!(m.sequence_for("ACT_VM_RELOAD").is_none());
    assert_eq!(m.sequence_named("fire1"), Some(fire));
}

#[test]
fn reads_materials_meshes_and_attachments() {
    let m = Mdl::parse(mdl(&gun())).unwrap();
    assert_eq!(m.textures, ["v_hands", "v_portalgun"]);
    assert_eq!(m.cd_textures, ["models\\Weapons\\v_models\\v_portalgun\\"]);
    assert_eq!(m.skins, vec![vec![0, 1]]);
    let model = &m.body_parts[0][0];
    assert_eq!(model.vertex_count, 7);
    let meshes: Vec<_> = model
        .meshes
        .iter()
        .map(|x| (x.material, x.vertex_count, x.vertex_offset))
        .collect();
    assert_eq!(meshes, [(0, 3, 0), (1, 4, 3)]);
    let a = &m.attachments[0];
    assert_eq!((a.name.as_str(), a.bone), ("muzzle", 1));
    assert_eq!(a.local[2][3], 22.0);
    assert_eq!(m.attachment("muzzle"), Some(0));
}

#[test]
fn refuses_other_files_and_versions() {
    let mut bytes = mdl(&gun());
    assert!(Mdl::parse(bytes[..100].to_vec()).is_err());
    bytes[4] = 49;
    assert!(Mdl::parse(bytes.clone()).is_err());
    bytes[4] = 46;
    bytes[0] = b'X';
    assert!(Mdl::parse(bytes).is_err());
}

#[test]
fn refuses_offsets_past_the_end() {
    let mut bytes = mdl(&gun());
    bytes[160..164].copy_from_slice(&(1_000_000i32).to_le_bytes());
    assert!(Mdl::parse(bytes).is_err());
}
