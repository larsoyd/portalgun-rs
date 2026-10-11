// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::path::Path;

use portal::install::Install;
use portal::mdl::Mdl;
use portal::particles::emitters::Emitter;
use portal::particles::forces::{Constraint, Force};
use portal::particles::initializers::Initializer;
use portal::particles::operators::Operator;
use portal::particles::renderers::Renderer;
use portal::particles::system::Definition;
use portal::pcf::Function;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::args()
        .nth(1)
        .ok_or("usage: portal_inspect PORTAL_INSTALL_DIR")?;
    let search = Install::open(Path::new(&root))?.search;
    if std::env::args().nth(2).as_deref() == Some("--effects") {
        return effects(&search);
    }
    let mdl = Mdl::parse(search.read("models/weapons/v_portalgun.mdl")?)?;
    println!(
        "[mdl] name={} version={} bones={} anims={} sequences={} textures={:?} attachments={}",
        mdl.name,
        mdl.version,
        mdl.bones.len(),
        mdl.anims.len(),
        mdl.sequences.len(),
        mdl.textures,
        mdl.attachments.len()
    );
    for s in &mdl.sequences {
        let a = &mdl.anims[s.anim];
        println!(
            "[seq] label={} activity={} looping={} frames={} fps={} events={:?}",
            s.label,
            s.activity,
            s.looping,
            a.frames,
            a.fps,
            s.events
                .iter()
                .map(|e| (e.cycle, e.event, &e.options))
                .collect::<Vec<_>>()
        );
    }
    let verts = portal::model::vertices(&search.read("models/weapons/v_portalgun.vvd")?, 0)?;
    let meshes = portal::model::meshes(
        &mdl,
        &verts,
        &search.read("models/weapons/v_portalgun.dx90.vtx")?,
    )?;
    for m in &meshes {
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for v in &m.vertices {
            for i in 0..3 {
                lo[i] = lo[i].min(v.pos[i]);
                hi[i] = hi[i].max(v.pos[i]);
            }
        }
        println!(
            "[mesh] material={} vertices={} triangles={} min={lo:?} max={hi:?}",
            mdl.textures[m.material],
            m.vertices.len(),
            m.indices.len() / 3
        );
    }
    if let Some(out) = std::env::args().nth(2) {
        for path in std::env::args().skip(3) {
            let name = path
                .rsplit('/')
                .next()
                .unwrap()
                .trim_end_matches(".vtf")
                .to_string();
            let bytes = search.read(&path)?;
            let v = portal::vtf::Vtf::parse(&bytes)?;
            std::fs::write(
                Path::new(&out).join(format!("{name}.dds")),
                portal::vtf::to_dds(&bytes)?,
            )?;
            println!(
                "[texture] name={name} size={}x{} format={} mips={}",
                v.width, v.height, v.format, v.mips
            );
        }
    }
    for pcf in [
        "particles/portalgun.pcf",
        "particles/portal_projectile.pcf",
        "particles/portals.pcf",
    ] {
        let elements = portal::dmx::parse(&search.read(pcf)?)?;
        let systems = portal::pcf::systems(&elements)?;
        println!(
            "[pcf] file={pcf} elements={} systems={}",
            elements.len(),
            systems.len()
        );
        for s in &systems {
            let kinds = [
                ("renderer", &s.renderers),
                ("operator", &s.operators),
                ("initializer", &s.initializers),
                ("emitter", &s.emitters),
                ("force", &s.forces),
                ("constraint", &s.constraints),
            ];
            for (kind, functions) in kinds {
                for f in functions.iter() {
                    println!("[pcf-fn] system={} kind={kind} function={}", s.name, f.name);
                    if ported(kind, f) {
                        println!("[ported] kind={kind} function={}", f.name);
                    }
                }
            }
            match Definition::new(s) {
                Ok(_) => println!("[pcf-def] system={} result=ok", s.name),
                Err(e) => println!("[pcf-def] system={} result=refused reason={e}", s.name),
            }
            for c in &s.children {
                println!(
                    "[pcf-child] system={} child={}",
                    s.name, systems[c.system].name
                );
            }
        }
    }
    let pose = portal::anim::sample(
        &mdl,
        mdl.sequences[mdl.sequence_for("ACT_VM_DRAW").unwrap()].anim,
        15.0,
    )?;
    println!(
        "[pose] draw frame=15 root={:?} base={:?}",
        pose[0], pose[24]
    );
    Ok(())
}

fn ported(kind: &str, f: &Function) -> bool {
    match kind {
        "operator" => Operator::from_function(f).is_some(),
        "initializer" => Initializer::from_function(f).is_some(),
        "emitter" => Emitter::from_function(f).is_some(),
        "force" => Force::from_function(f).is_some(),
        "constraint" => Constraint::from_function(f).is_some(),
        "renderer" => Renderer::from_function(f).is_some(),
        _ => false,
    }
}

fn effects(search: &portal::vpk::SearchPath) -> Result<(), Box<dyn std::error::Error>> {
    for name in ["models/portals/portal1.mdl", "models/portals/portal2.mdl"] {
        let mdl = Mdl::parse(search.read(name)?)?;
        println!(
            "[attachment-pose] {:?}",
            portal::anim::sample(&mdl, 0, 0.0)?
        );
        println!(
            "[model] {name} bones={:?} attachments={:?} sequences={:?} anims={:?}",
            mdl.bones, mdl.attachments, mdl.sequences, mdl.anims
        );
    }
    let mut materials = std::collections::BTreeSet::new();
    for file in [
        "particles/portals.pcf",
        "particles/portalgun.pcf",
        "particles/portal_projectile.pcf",
    ] {
        let systems = portal::pcf::systems(&portal::dmx::parse(&search.read(file)?)?)?;
        for s in &systems {
            println!(
                "[system] file={file} name={} material={} color={:?} children={:?}",
                s.name,
                s.material,
                s.color,
                s.children
                    .iter()
                    .map(|c| (&systems[c.system].name, c.delay))
                    .collect::<Vec<_>>()
            );
            println!("[definition] {s:?}");
            if !s.material.is_empty() {
                materials.insert(s.material.clone());
            }
        }
    }
    for material in materials {
        let path = format!("materials/{}", material.replace('\\', "/"));
        let path = if path.ends_with(".vmt") {
            path
        } else {
            format!("{path}.vmt")
        };
        println!(
            "[material] {path}\n{}",
            String::from_utf8_lossy(&search.read(&path)?)
        );
    }
    Ok(())
}
