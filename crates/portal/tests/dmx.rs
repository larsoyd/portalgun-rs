// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use portal::dmx::{parse, Value};

/// A binary DMX version 2 file: a string table, then elements and their attributes.
struct Dmx {
    strings: Vec<&'static str>,
    elements: Vec<(&'static str, &'static str)>,
    attributes: Vec<Vec<(&'static str, u8, Vec<u8>)>>,
}

impl Dmx {
    fn index(&mut self, s: &'static str) -> u16 {
        if let Some(i) = self.strings.iter().position(|x| *x == s) {
            return i as u16;
        }
        self.strings.push(s);
        (self.strings.len() - 1) as u16
    }

    fn bytes(mut self) -> Vec<u8> {
        let elements = self.elements.clone();
        let types: Vec<u16> = elements.iter().map(|(t, _)| self.index(t)).collect();
        let attributes = std::mem::take(&mut self.attributes);
        let names: Vec<Vec<u16>> = attributes
            .iter()
            .map(|a| a.iter().map(|(n, _, _)| self.index(n)).collect())
            .collect();
        let mut out = b"<!-- dmx encoding binary 2 format pcf 1 -->\n\0".to_vec();
        out.extend((self.strings.len() as u16).to_le_bytes());
        for s in &self.strings {
            out.extend(s.as_bytes());
            out.push(0);
        }
        out.extend((elements.len() as i32).to_le_bytes());
        for ((_, name), t) in elements.iter().zip(&types) {
            out.extend(t.to_le_bytes());
            out.extend(name.as_bytes());
            out.push(0);
            out.extend([7u8; 16]);
        }
        for (attrs, names) in attributes.iter().zip(&names) {
            out.extend((attrs.len() as i32).to_le_bytes());
            for ((_, kind, value), n) in attrs.iter().zip(names) {
                out.extend(n.to_le_bytes());
                out.push(*kind);
                out.extend(value);
            }
        }
        out
    }
}

fn f32s(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

fn sample() -> Vec<u8> {
    sample_linking(-1)
}

fn sample_linking(link: i32) -> Vec<u8> {
    let mut strings_array = 2i32.to_le_bytes().to_vec();
    strings_array.extend(b"a\0bc\0");
    let mut refs = 1i32.to_le_bytes().to_vec();
    refs.extend(1i32.to_le_bytes());
    Dmx {
        strings: vec![],
        elements: vec![
            ("DmeParticleSystemDefinition", "portal_1_edge"),
            ("DmeParticleOperator", "op"),
        ],
        attributes: vec![
            vec![
                ("max_particles", 2, 1000i32.to_le_bytes().to_vec()),
                ("material", 5, b"effects\\portal_1_particle.vmt\0".to_vec()),
                ("radius", 3, 5.0f32.to_le_bytes().to_vec()),
                ("color", 8, vec![255, 198, 0, 255]),
                ("bounding_box_max", 10, f32s(&[60.0, 40.0, 50.0])),
                ("flag", 4, vec![1]),
                ("operators", 15, refs),
                ("names", 19, strings_array),
                ("nothing", 1, link.to_le_bytes().to_vec()),
                ("delay", 7, 25_000i32.to_le_bytes().to_vec()),
            ],
            vec![
                ("functionName", 5, b"Movement Basic\0".to_vec()),
                ("drag", 3, 0.3f32.to_le_bytes().to_vec()),
                ("gravity", 10, f32s(&[0.0, 0.0, -1.0])),
            ],
        ],
    }
    .bytes()
}

#[test]
fn reads_elements_and_typed_attributes() {
    let e = parse(&sample()).unwrap();
    assert_eq!(e.len(), 2);
    assert_eq!(
        (e[0].kind.as_str(), e[0].name.as_str()),
        ("DmeParticleSystemDefinition", "portal_1_edge")
    );
    assert_eq!(e[0].get("max_particles"), Some(&Value::Int(1000)));
    assert_eq!(
        e[0].get("material"),
        Some(&Value::String("effects\\portal_1_particle.vmt".into()))
    );
    assert_eq!(e[0].get("radius"), Some(&Value::Float(5.0)));
    assert_eq!(e[0].get("color"), Some(&Value::Color([255, 198, 0, 255])));
    assert_eq!(
        e[0].get("bounding_box_max"),
        Some(&Value::Vec3([60.0, 40.0, 50.0]))
    );
    assert_eq!(e[0].get("flag"), Some(&Value::Bool(true)));
    assert_eq!(e[0].get("nothing"), Some(&Value::Element(None)));
    assert_eq!(e[0].get("delay"), Some(&Value::Time(2.5)));
    assert_eq!(
        e[0].get("operators"),
        Some(&Value::Array(vec![Value::Element(Some(1))]))
    );
    assert_eq!(
        e[0].get("names"),
        Some(&Value::Array(vec![
            Value::String("a".into()),
            Value::String("bc".into())
        ]))
    );
    assert_eq!(
        e[1].get("functionName"),
        Some(&Value::String("Movement Basic".into()))
    );
    assert_eq!(e[1].float("drag"), Some(0.3));
    assert_eq!(e[1].vec3("gravity"), Some([0.0, 0.0, -1.0]));
    assert_eq!(e[1].get("missing"), None);
}

#[test]
fn refuses_other_encodings_and_truncation() {
    let mut text = sample();
    text[17..23].copy_from_slice(b"keyval");
    assert!(parse(&text).is_err());
    let whole = sample();
    assert!(parse(&whole[..whole.len() - 3]).is_err());
    assert!(parse(&sample_linking(1)).is_ok());
    assert!(
        parse(&sample_linking(2)).is_err(),
        "an element index past the list"
    );
}
