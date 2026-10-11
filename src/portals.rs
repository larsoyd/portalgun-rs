use portal::keyvalues;
use portal::pair::Portal;
use portal::portal_face::{self, Fade};
use portal::view::doubler_usable;
use portal::vpk::SearchPath;

/// PortalRefract's stage 2 materials, blue then orange
const OVERLAYS: [&str; 2] = [
    "models/portals/portalstaticoverlay_1",
    "models/portals/portalstaticoverlay_2",
];

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Colour {
    Blue,
    Orange,
}

impl Colour {
    pub const ALL: [Colour; 2] = [Colour::Blue, Colour::Orange];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn other(self) -> Colour {
        match self {
            Colour::Blue => Colour::Orange,
            Colour::Orange => Colour::Blue,
        }
    }

    /// The sound scripts call orange red.
    pub fn sound(self) -> &'static str {
        match self {
            Colour::Blue => "blue",
            Colour::Orange => "red",
        }
    }
}

/// The two portals the gun has opened.
#[derive(Default)]
pub struct Pair {
    pub open: [Option<Portal>; 2],
    pub fade: [Fade; 2],
    /// whether each portal may draw the depth doubler
    pub usable: [bool; 2],
    /// bumped whenever a portal opens, moves or closes
    pub changes: u64,
    pub revisions: [u64; 2],
}

impl Pair {
    pub fn get(&self, c: Colour) -> Option<&Portal> {
        self.open[c.index()].as_ref()
    }

    pub fn linked(&self) -> Option<[Portal; 2]> {
        Some([*self.get(Colour::Blue)?, *self.get(Colour::Orange)?])
    }

    // Translated from 0x559c30 (decompiled, client.so)
    pub fn place(&mut self, c: Colour, portal: Portal) {
        let partner = self.get(c.other()).copied();
        let linked = partner.is_some();
        self.open[c.index()] = Some(portal);
        self.usable[c.index()] = partner.is_some_and(|o| doubler_usable(&portal, &o));
        if linked {
            self.usable[c.other().index()] = true;
        }
        let [blue, orange] = &mut self.fade;
        let (fade, partner) = match c {
            Colour::Blue => (blue, orange),
            Colour::Orange => (orange, blue),
        };
        fade.moved(linked.then_some(partner));
        self.changes += 1;
        self.revisions[c.index()] += 1;
    }

    pub fn deactivate(&mut self, c: Colour) {
        if self.open[c.index()].take().is_some() {
            self.revisions[c.index()] += 1;
            self.changes += 1;
            self.usable = [false; 2];
            self.fade[c.other().index()].statics = 1.0;
        }
    }

    pub fn think(&mut self, dt: f32) {
        self.fade.iter_mut().for_each(|f| f.think(dt));
    }
}

/// What a static overlay material names for DX9, its DX8 values sit in a block of their own.
pub struct OverlayLook {
    pub noise: String,
    pub ramp: String,
    pub scale: f32,
}

pub fn overlay_look(search: &SearchPath, c: Colour) -> Result<OverlayLook, String> {
    let path = format!("materials/{}.vmt", OVERLAYS[c.index()]);
    let shader = material(search, &path)?;
    let get = |k: &str| {
        shader
            .get(k)
            .map(|v| v.trim().to_string())
            .ok_or_else(|| format!("{path}: no {k}"))
    };
    let scale = get("$PortalColorScale")?;
    Ok(OverlayLook {
        noise: get("$PortalMaskTexture")?,
        ramp: get("$PortalColorTexture")?,
        scale: scale
            .parse()
            .map_err(|_| format!("{path}: $PortalColorScale {scale}"))?,
    })
}

/// The drawn face's corners in world units and their texture coordinates, counter-clockwise seen
/// from in front.
pub fn face_quad(portal: &Portal) -> ([[f32; 3]; 4], [[f32; 2]; 4]) {
    let uvs = [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]];
    (uvs.map(|uv| portal.world(portal_face::corner(uv))), uvs)
}

/// The depth doubler's material, drawn by 0x557110 (client.so).
const DOUBLER: &str = "materials/models/portals/portal_depthdoubler.vmt";
/// the screen copy the doubler samples, made at 0x565a40 (client.so)
const DOUBLER_TARGET: &str = "_rt_DepthDoubler";

fn material(search: &SearchPath, path: &str) -> Result<keyvalues::Kv, String> {
    let text = search.read(path).map_err(|e| format!("{path}: {e}"))?;
    keyvalues::parse(&String::from_utf8_lossy(&text))
        .map_err(|e| format!("{path}: {e}"))?
        .into_iter()
        .next()
        .ok_or_else(|| format!("{path}: empty"))
}

/// The depth doubler's alpha mask. Its base texture must be the screen copy.
pub fn doubler_mask(search: &SearchPath) -> Result<String, String> {
    let shader = material(search, DOUBLER)?;
    let get = |k: &str| {
        shader
            .get(k)
            .map(|v| v.trim().to_string())
            .ok_or_else(|| format!("{DOUBLER}: no {k}"))
    };
    match get("$basetexture")? {
        t if t.eq_ignore_ascii_case(DOUBLER_TARGET) => get("$alphamasktexture"),
        t => Err(format!(
            "{DOUBLER}: $basetexture {t}, expected {DOUBLER_TARGET}"
        )),
    }
}
