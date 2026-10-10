use portal::keyvalues;
use portal::pair::Portal;
use portal::portal_face::Fade;
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
    /// bumped whenever a portal opens, moves or closes
    pub changes: u64,
}

impl Pair {
    pub fn get(&self, c: Colour) -> Option<&Portal> {
        self.open[c.index()].as_ref()
    }

    pub fn linked(&self) -> Option<[Portal; 2]> {
        Some([*self.get(Colour::Blue)?, *self.get(Colour::Orange)?])
    }

    pub fn place(&mut self, c: Colour, portal: Portal) {
        let linked = self.get(c.other()).is_some();
        self.open[c.index()] = Some(portal);
        let [blue, orange] = &mut self.fade;
        let (fade, partner) = match c {
            Colour::Blue => (blue, orange),
            Colour::Orange => (orange, blue),
        };
        fade.moved(linked.then_some(partner));
        self.changes += 1;
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
    let text = search.read(&path).map_err(|e| format!("{path}: {e}"))?;
    let root =
        keyvalues::parse(&String::from_utf8_lossy(&text)).map_err(|e| format!("{path}: {e}"))?;
    let shader = root.first().ok_or_else(|| format!("{path}: empty"))?;
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
