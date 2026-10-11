// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

#![allow(dead_code)]

pub struct Blob(pub Vec<u8>);

impl Blob {
    pub fn new(len: usize) -> Self {
        Blob(vec![0; len])
    }
    pub fn at(&self) -> usize {
        self.0.len()
    }
    pub fn i32(&mut self, at: usize, v: i32) {
        self.0[at..at + 4].copy_from_slice(&v.to_le_bytes());
    }
    pub fn f32(&mut self, at: usize, v: f32) {
        self.0[at..at + 4].copy_from_slice(&v.to_le_bytes());
    }
    pub fn f32s(&mut self, at: usize, v: &[f32]) {
        for (i, x) in v.iter().enumerate() {
            self.f32(at + 4 * i, *x);
        }
    }
    pub fn i16(&mut self, at: usize, v: i16) {
        self.0[at..at + 2].copy_from_slice(&v.to_le_bytes());
    }
    pub fn grow(&mut self, len: usize) -> usize {
        let at = self.at();
        self.0.resize(at + len, 0);
        at
    }
    pub fn bytes(&mut self, b: &[u8]) -> usize {
        let at = self.at();
        self.0.extend_from_slice(b);
        at
    }
    pub fn cstr(&mut self, s: &str) -> usize {
        let at = self.bytes(s.as_bytes());
        self.0.push(0);
        at
    }
}

pub struct BoneDef {
    pub name: &'static str,
    pub parent: i32,
    pub pos: [f32; 3],
    pub rot: [f32; 3],
    pub pos_scale: [f32; 3],
    pub rot_scale: [f32; 3],
}

pub fn bone(name: &'static str, parent: i32, pos: [f32; 3]) -> BoneDef {
    BoneDef {
        name,
        parent,
        pos,
        rot: [0.0; 3],
        pos_scale: [1.0; 3],
        rot_scale: [1.0; 3],
    }
}

pub struct AnimDef {
    pub name: &'static str,
    pub fps: f32,
    pub flags: i32,
    pub frames: i32,
    /// raw mstudioanim_t chain, offsets relative to its start
    pub data: Vec<u8>,
}

pub struct SeqDef {
    pub label: &'static str,
    pub activity: &'static str,
    pub flags: i32,
    pub anim: i16,
    pub fade: [f32; 2],
    pub events: Vec<(f32, i32, &'static str)>,
}

pub struct MdlDef {
    pub bones: Vec<BoneDef>,
    pub anims: Vec<AnimDef>,
    pub seqs: Vec<SeqDef>,
    pub textures: Vec<&'static str>,
    pub cd: Vec<&'static str>,
    pub meshes: Vec<(i32, i32, i32)>,
    pub attachments: Vec<(&'static str, i32, [f32; 3])>,
}

pub const HEADER: usize = 408;

pub fn mdl(def: &MdlDef) -> Vec<u8> {
    let mut b = Blob::new(HEADER);
    b.bytes_at(0, b"IDST");
    b.i32(4, 46);
    b.bytes_at(12, b"weapons/v_test.mdl");
    b.f32s(80, &[0.0, 0.0, 64.0]);

    let bones = b.grow(216 * def.bones.len());
    b.i32(156, def.bones.len() as i32);
    b.i32(160, bones as i32);
    for (i, d) in def.bones.iter().enumerate() {
        let o = bones + 216 * i;
        let name = b.cstr(d.name);
        b.i32(o, (name - o) as i32);
        b.i32(o + 4, d.parent);
        b.f32s(o + 32, &d.pos);
        b.f32s(o + 44, &angle_quat(d.rot));
        b.f32s(o + 60, &d.rot);
        b.f32s(o + 72, &d.pos_scale);
        b.f32s(o + 84, &d.rot_scale);
        b.f32s(
            o + 96,
            &[
                1.0, 0.0, 0.0, -d.pos[0], 0.0, 1.0, 0.0, -d.pos[1], 0.0, 0.0, 1.0, -d.pos[2],
            ],
        );
    }

    let anims = b.grow(100 * def.anims.len());
    b.i32(180, def.anims.len() as i32);
    b.i32(184, anims as i32);
    for (i, a) in def.anims.iter().enumerate() {
        let o = anims + 100 * i;
        b.i32(o, -(o as i32));
        let name = b.cstr(a.name);
        b.i32(o + 4, (name - o) as i32);
        b.f32(o + 8, a.fps);
        b.i32(o + 12, a.flags);
        b.i32(o + 16, a.frames);
        let data = b.bytes(&a.data);
        b.i32(o + 56, (data - o) as i32);
    }

    let seqs = b.grow(212 * def.seqs.len());
    b.i32(188, def.seqs.len() as i32);
    b.i32(192, seqs as i32);
    for (i, s) in def.seqs.iter().enumerate() {
        let o = seqs + 212 * i;
        b.i32(o, -(o as i32));
        let label = b.cstr(s.label);
        b.i32(o + 4, (label - o) as i32);
        let act = b.cstr(s.activity);
        b.i32(o + 8, (act - o) as i32);
        b.i32(o + 12, s.flags);
        b.i32(o + 56, 1);
        let idx = b.grow(2);
        b.i16(idx, s.anim);
        b.i32(o + 60, (idx - o) as i32);
        b.i32(o + 68, 1);
        b.i32(o + 72, 1);
        b.f32(o + 104, s.fade[0]);
        b.f32(o + 108, s.fade[1]);
        let events = b.grow(80 * s.events.len());
        b.i32(o + 24, s.events.len() as i32);
        b.i32(o + 28, (events - o) as i32);
        for (k, (cycle, event, options)) in s.events.iter().enumerate() {
            let e = events + 80 * k;
            b.f32(e, *cycle);
            b.i32(e + 4, *event);
            b.bytes_at(e + 12, options.as_bytes());
        }
    }

    let tex = b.grow(64 * def.textures.len());
    b.i32(204, def.textures.len() as i32);
    b.i32(208, tex as i32);
    for (i, t) in def.textures.iter().enumerate() {
        let o = tex + 64 * i;
        let name = b.cstr(t);
        b.i32(o, (name - o) as i32);
    }
    let cd = b.grow(4 * def.cd.len());
    b.i32(212, def.cd.len() as i32);
    b.i32(216, cd as i32);
    for (i, t) in def.cd.iter().enumerate() {
        let name = b.cstr(t);
        b.i32(cd + 4 * i, name as i32);
    }
    let skins = b.grow(2 * def.textures.len());
    b.i32(220, def.textures.len() as i32);
    b.i32(224, 1);
    b.i32(228, skins as i32);
    for i in 0..def.textures.len() {
        b.i16(skins + 2 * i, i as i16);
    }

    let part = b.grow(16);
    b.i32(232, 1);
    b.i32(236, part as i32);
    let pname = b.cstr("studio");
    b.i32(part, (pname - part) as i32);
    b.i32(part + 4, 1);
    let model = b.grow(148);
    b.i32(part + 12, (model - part) as i32);
    b.bytes_at(model, b"ref.smd");
    let meshes = b.grow(116 * def.meshes.len());
    b.i32(model + 72, def.meshes.len() as i32);
    b.i32(model + 76, (meshes - model) as i32);
    b.i32(model + 80, def.meshes.iter().map(|m| m.1).sum());
    for (i, (mat, count, offset)) in def.meshes.iter().enumerate() {
        let o = meshes + 116 * i;
        b.i32(o, *mat);
        b.i32(o + 8, *count);
        b.i32(o + 12, *offset);
    }

    let att = b.grow(92 * def.attachments.len());
    b.i32(240, def.attachments.len() as i32);
    b.i32(244, att as i32);
    for (i, (name, bone, origin)) in def.attachments.iter().enumerate() {
        let o = att + 92 * i;
        let n = b.cstr(name);
        b.i32(o, (n - o) as i32);
        b.i32(o + 8, *bone);
        b.f32s(
            o + 12,
            &[
                1.0, 0.0, 0.0, origin[0], 0.0, 1.0, 0.0, origin[1], 0.0, 0.0, 1.0, origin[2],
            ],
        );
    }
    let len = b.at() as i32;
    b.i32(76, len);
    b.0
}

impl Blob {
    pub fn bytes_at(&mut self, at: usize, v: &[u8]) {
        self.0[at..at + v.len()].copy_from_slice(v);
    }
}

pub fn angle_quat(a: [f32; 3]) -> [f32; 4] {
    let (sy, cy) = (a[2] * 0.5).sin_cos();
    let (sp, cp) = (a[1] * 0.5).sin_cos();
    let (sr, cr) = (a[0] * 0.5).sin_cos();
    [
        sr * cp * cy - cr * sp * sy,
        cr * sp * cy + sr * cp * sy,
        cr * cp * sy - sr * sp * cy,
        cr * cp * cy + sr * sp * sy,
    ]
}

/// One mstudioanim_t record: bone, flags, then payload, with nextoffset filled in by chain.
pub fn anim_chain(records: &[(u8, u8, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    for (i, (bone, flags, payload)) in records.iter().enumerate() {
        let next = if i + 1 == records.len() {
            0
        } else {
            4 + payload.len() as i16
        };
        out.push(*bone);
        out.push(*flags);
        out.extend_from_slice(&next.to_le_bytes());
        out.extend_from_slice(payload);
    }
    out
}

pub type Track = [(u8, u8, Vec<i16>)];

/// Adjacent value pointers (rotation then position) followed by their RLE tracks.
pub fn value_tracks(sets: &[[&Track; 3]]) -> Vec<u8> {
    let mut ptrs = vec![0i16; 3 * sets.len()];
    let mut body = Vec::new();
    for (set, tracks) in sets.iter().enumerate() {
        for (axis, track) in tracks.iter().enumerate() {
            if track.is_empty() {
                continue;
            }
            ptrs[3 * set + axis] = (6 * (sets.len() - set) + body.len()) as i16;
            for (valid, total, values) in track.iter() {
                body.push(*valid);
                body.push(*total);
                for v in values {
                    body.extend_from_slice(&v.to_le_bytes());
                }
            }
        }
    }
    let mut out: Vec<u8> = ptrs.iter().flat_map(|p| p.to_le_bytes()).collect();
    out.extend(body);
    out
}

pub struct VertexDef {
    pub pos: [f32; 3],
    pub bones: [u8; 3],
    pub weights: [f32; 3],
    pub count: u8,
}

pub fn vertex(x: f32, bone: u8) -> VertexDef {
    VertexDef {
        pos: [x, 0.0, 0.0],
        bones: [bone, 0, 0],
        weights: [1.0, 0.0, 0.0],
        count: 1,
    }
}

/// fixups are (lod, source, count)
pub fn vvd(verts: &[VertexDef], fixups: &[(i32, i32, i32)]) -> Vec<u8> {
    let mut b = Blob::new(64);
    b.bytes_at(0, b"IDSV");
    b.i32(4, 4);
    b.i32(12, 1);
    b.i32(16, verts.len() as i32);
    b.i32(48, fixups.len() as i32);
    let fix = b.grow(12 * fixups.len());
    b.i32(52, fix as i32);
    for (i, (lod, src, n)) in fixups.iter().enumerate() {
        b.i32(fix + 12 * i, *lod);
        b.i32(fix + 12 * i + 4, *src);
        b.i32(fix + 12 * i + 8, *n);
    }
    let data = b.grow(48 * verts.len());
    b.i32(56, data as i32);
    for (i, v) in verts.iter().enumerate() {
        let o = data + 48 * i;
        b.f32s(o, &v.weights);
        b.bytes_at(o + 12, &v.bones);
        b.0[o + 15] = v.count;
        b.f32s(o + 16, &v.pos);
        b.f32s(o + 28, &[0.0, 0.0, 1.0]);
        b.f32s(o + 40, &[v.pos[0] / 10.0, 0.5]);
    }
    let tan = b.grow(16 * verts.len());
    b.i32(60, tan as i32);
    for i in 0..verts.len() {
        b.f32s(tan + 16 * i, &[1.0, i as f32, 0.0, -1.0]);
    }
    b.0
}

/// mesh-local vertex ids, strips of triangle indices into those, strip flags
pub type StripGroup = (Vec<u16>, Vec<Vec<u16>>, u8);

/// one body part, one model and one LOD holding these meshes
pub fn vtx(meshes: &[Vec<StripGroup>]) -> Vec<u8> {
    let mut b = Blob::new(36);
    b.i32(0, 7);
    b.i32(20, 1);
    b.i32(28, 1);
    let part = b.grow(8);
    b.i32(32, part as i32);
    b.i32(part, 1);
    let model = b.grow(8);
    b.i32(part + 4, (model - part) as i32);
    b.i32(model, 1);
    let lod = b.grow(12);
    b.i32(model + 4, (lod - model) as i32);
    b.i32(lod, meshes.len() as i32);
    let mesh0 = b.grow(9 * meshes.len());
    b.i32(lod + 4, (mesh0 - lod) as i32);
    for (m, groups) in meshes.iter().enumerate() {
        let mesh = mesh0 + 9 * m;
        let g0 = b.grow(25 * groups.len());
        b.i32(mesh, groups.len() as i32);
        b.i32(mesh + 4, (g0 - mesh) as i32);
        for (g, (ids, strips, flags)) in groups.iter().enumerate() {
            let group = g0 + 25 * g;
            let verts = b.grow(9 * ids.len());
            for (i, id) in ids.iter().enumerate() {
                b.0[verts + 9 * i + 3] = 1;
                b.0[verts + 9 * i + 4..verts + 9 * i + 6].copy_from_slice(&id.to_le_bytes());
            }
            let indices: Vec<u16> = strips.concat();
            let idx = b.grow(2 * indices.len());
            for (i, v) in indices.iter().enumerate() {
                b.i16(idx + 2 * i, *v as i16);
            }
            let strip = b.grow(27 * strips.len());
            let mut first = 0;
            for (k, s) in strips.iter().enumerate() {
                b.i32(strip + 27 * k, s.len() as i32);
                b.i32(strip + 27 * k + 4, first);
                b.i32(strip + 27 * k + 8, ids.len() as i32);
                b.0[strip + 27 * k + 18] = *flags;
                first += s.len() as i32;
            }
            b.i32(group, ids.len() as i32);
            b.i32(group + 4, (verts - group) as i32);
            b.i32(group + 8, indices.len() as i32);
            b.i32(group + 12, (idx - group) as i32);
            b.i32(group + 16, strips.len() as i32);
            b.i32(group + 20, (strip - group) as i32);
        }
    }
    b.0
}
