use portal::vpk::{Pak, SearchPath};

struct File<'a> {
    ext: &'a str,
    dir: &'a str,
    name: &'a str,
    preload: &'a [u8],
    archive: u16,
    offset: u32,
    len: u32,
}

fn dir_tree(files: &[File]) -> Vec<u8> {
    let mut t = Vec::new();
    for f in files {
        for s in [f.ext, f.dir, f.name] {
            t.extend_from_slice(s.as_bytes());
            t.push(0);
        }
        t.extend_from_slice(&0u32.to_le_bytes());
        t.extend_from_slice(&(f.preload.len() as u16).to_le_bytes());
        t.extend_from_slice(&f.archive.to_le_bytes());
        t.extend_from_slice(&f.offset.to_le_bytes());
        t.extend_from_slice(&f.len.to_le_bytes());
        t.extend_from_slice(&0xffffu16.to_le_bytes());
        t.extend_from_slice(f.preload);
        t.extend_from_slice(&[0, 0]);
    }
    t.push(0);
    t
}

fn dir_file(version: u32, files: &[File], embedded: &[u8]) -> Vec<u8> {
    let tree = dir_tree(files);
    let mut d = Vec::new();
    d.extend_from_slice(&0x55aa_1234u32.to_le_bytes());
    d.extend_from_slice(&version.to_le_bytes());
    d.extend_from_slice(&(tree.len() as u32).to_le_bytes());
    if version == 2 {
        d.extend_from_slice(&[0; 16]);
    }
    d.extend_from_slice(&tree);
    d.extend_from_slice(embedded);
    d
}

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("portal-vpk-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn reads_preload_embedded_and_archived_data() {
    let dir = scratch("v2");
    let files = [
        File {
            ext: "mdl",
            dir: "models/weapons",
            name: "v_portalgun",
            preload: b"ID",
            archive: 0,
            offset: 3,
            len: 2,
        },
        File {
            ext: "vmt",
            dir: "materials/x",
            name: "a",
            preload: b"",
            archive: 0x7fff,
            offset: 2,
            len: 3,
        },
        File {
            ext: "txt",
            dir: " ",
            name: "root",
            preload: b"top",
            archive: 0x7fff,
            offset: 0,
            len: 0,
        },
    ];
    std::fs::write(dir.join("pak_dir.vpk"), dir_file(2, &files, b"..xyz")).unwrap();
    std::fs::write(dir.join("pak_000.vpk"), b"___STxxxx").unwrap();
    let pak = Pak::open(&dir.join("pak_dir.vpk")).unwrap();
    assert_eq!(pak.len(), 3);
    assert_eq!(pak.read("models/weapons/v_portalgun.mdl").unwrap(), b"IDST");
    assert_eq!(pak.read("Materials\\X\\A.vmt").unwrap(), b"xyz");
    assert_eq!(pak.read("root.txt").unwrap(), b"top");
    assert!(pak.read("models/missing.mdl").is_err());
}

#[test]
fn version_one_has_a_short_header() {
    let dir = scratch("v1");
    let files = [File {
        ext: "wav",
        dir: "sound",
        name: "s",
        preload: b"",
        archive: 0x7fff,
        offset: 1,
        len: 2,
    }];
    std::fs::write(dir.join("one_dir.vpk"), dir_file(1, &files, b"0ab")).unwrap();
    let pak = Pak::open(&dir.join("one_dir.vpk")).unwrap();
    assert_eq!(pak.read("sound/s.wav").unwrap(), b"ab");
}

#[test]
fn search_path_takes_the_first_pak_that_has_the_file() {
    let dir = scratch("search");
    let a = [File {
        ext: "txt",
        dir: "d",
        name: "both",
        preload: b"first",
        archive: 0x7fff,
        offset: 0,
        len: 0,
    }];
    let b = [
        File {
            ext: "txt",
            dir: "d",
            name: "both",
            preload: b"second",
            archive: 0x7fff,
            offset: 0,
            len: 0,
        },
        File {
            ext: "txt",
            dir: "d",
            name: "only_b",
            preload: b"b",
            archive: 0x7fff,
            offset: 0,
            len: 0,
        },
    ];
    std::fs::write(dir.join("a_dir.vpk"), dir_file(2, &a, b"")).unwrap();
    std::fs::write(dir.join("b_dir.vpk"), dir_file(2, &b, b"")).unwrap();
    let search = SearchPath::new(vec![
        Pak::open(&dir.join("a_dir.vpk")).unwrap(),
        Pak::open(&dir.join("b_dir.vpk")).unwrap(),
    ]);
    assert_eq!(search.read("d/both.txt").unwrap(), b"first");
    assert_eq!(search.read("d/only_b.txt").unwrap(), b"b");
    assert!(search.read("d/none.txt").is_err());
}

#[test]
fn rejects_a_file_without_the_signature() {
    let dir = scratch("bad");
    std::fs::write(dir.join("bad_dir.vpk"), [0u8; 32]).unwrap();
    assert!(Pak::open(&dir.join("bad_dir.vpk")).is_err());
}
