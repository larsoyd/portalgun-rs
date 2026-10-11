// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::text::{FontSize, FontSource, LineHeight};
use bevy::window::PrimaryWindow;
use portal::crosshair::{font_tall, icons, paint, reticle_at, GunState, Rings};
use portal::keyvalues::{self, Kv};

use crate::faces::Portals;
use crate::firing::Firing;
use crate::gun::test_shot;
use crate::player::Player;
use crate::portals::Colour;
use crate::scene::Level;
use crate::sound::Files;
use crate::texture;

/// the server's placability think runs ten times a second
const TEST_EVERY: f32 = 0.1;
const TEXTURE_DATA: &str = "scripts/mod_textures.txt";
const SCHEME: &str = "resource/clientscheme.res";
/// CustomFontFiles names it in resource, the install keeps it loose under hl2
const RETICLE_FONT: &str = "hl2/resource/halflife2.ttf";
/// the crosshair character the gun's weapon script names
const RETICLE: char = 'Q';

#[derive(Resource)]
pub struct InstallRoot(pub PathBuf);

#[derive(Resource)]
struct Crosshair {
    sizes: [(i32, i32); 5],
    rects: [[i32; 4]; 5],
    sprites: [Entity; 4],
    reticle: Entity,
    scheme: Vec<Kv>,
    /// the reticle's advance over the font's em
    advance: f32,
    rings: Rings,
    placable: [f32; 2],
    next_test: f32,
    screen: UVec2,
}

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_hud)
            .add_systems(Update, (test_shots, draw_hud).chain());
    }
}

fn read_kv(files: &Files, path: &str) -> Result<Vec<Kv>, String> {
    let text = files.read(path).map_err(|e| format!("{path}: {e}"))?;
    keyvalues::parse(&String::from_utf8_lossy(&text)).map_err(|e| format!("{path}: {e}"))
}

fn reticle_advance(bytes: &[u8]) -> Result<f32, String> {
    let face = ttf_parser::Face::parse(bytes, 0).map_err(|e| format!("{RETICLE_FONT}: {e}"))?;
    let glyph = face
        .glyph_index(RETICLE)
        .ok_or_else(|| format!("{RETICLE_FONT}: no {RETICLE}"))?;
    let advance = face
        .glyph_hor_advance(glyph)
        .ok_or_else(|| format!("{RETICLE_FONT}: no advance for {RETICLE}"))?;
    Ok(advance as f32 / face.units_per_em() as f32)
}

fn spawn_hud(
    mut commands: Commands,
    files: Res<Files>,
    root: Res<InstallRoot>,
    mut images: ResMut<Assets<Image>>,
    mut fonts: ResMut<Assets<Font>>,
    mut exit: MessageWriter<AppExit>,
) {
    let read = || -> Result<_, String> {
        let (sheet, rects) = icons(&read_kv(&files, TEXTURE_DATA)?)?;
        let image =
            texture::base_texture(&files, &sheet).and_then(|t| texture::load(&files, &t))?;
        let font_path = root.0.join(RETICLE_FONT);
        let bytes =
            std::fs::read(&font_path).map_err(|e| format!("{}: {e}", font_path.display()))?;
        let advance = reticle_advance(&bytes)?;
        let font = Font::from_bytes(bytes);
        Ok((sheet, rects, image, font, advance, read_kv(&files, SCHEME)?))
    };
    let (sheet, rects, image, font, advance, scheme) = match read() {
        Ok(r) => r,
        Err(e) => {
            error!("[hud] refused: reason={e}");
            exit.write(AppExit::error());
            return;
        }
    };
    let size = image.size();
    let image = images.add(image);
    let sprite = |commands: &mut Commands| {
        commands
            .spawn((
                ImageNode::new(image.clone()),
                Node {
                    position_type: PositionType::Absolute,
                    ..default()
                },
            ))
            .id()
    };
    let sprites = [(); 4].map(|_| sprite(&mut commands));
    let reticle = commands
        .spawn((
            Text::new(RETICLE.to_string()),
            TextFont {
                font: FontSource::Handle(fonts.add(font)),
                ..default()
            },
            TextColor(Color::WHITE),
            Node {
                position_type: PositionType::Absolute,
                ..default()
            },
        ))
        .id();
    info!(
        "[hud] sheet={sheet} size={}x{} reticle_advance={advance:.3}",
        size.x, size.y
    );
    commands.insert_resource(Crosshair {
        sizes: rects.map(|r| (r[2], r[3])),
        rects,
        sprites,
        reticle,
        scheme,
        advance,
        rings: Rings::default(),
        placable: [1.0; 2],
        next_test: 0.0,
        screen: UVec2::ZERO,
    });
}

fn test_shots(
    time: Res<Time>,
    hud: Option<ResMut<Crosshair>>,
    player: Res<Player>,
    level: Res<Level>,
    portals: Res<Portals>,
) {
    let Some(mut hud) = hud else { return };
    let now = time.elapsed_secs();
    if now < hud.next_test {
        return;
    }
    hud.next_test = now + TEST_EVERY;
    let placable = Colour::ALL.map(|c| test_shot(c, &player, &level.0, &portals));
    if placable != hud.placable {
        info!(
            "[hud] placable at={now:.2} blue={:.3} orange={:.3}",
            placable[0], placable[1]
        );
    }
    hud.placable = placable;
}

#[allow(clippy::too_many_arguments)]
fn draw_hud(
    time: Res<Time>,
    hud: Option<ResMut<Crosshair>>,
    gun: Res<Firing>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut nodes: Query<&mut Node>,
    mut sprites: Query<&mut ImageNode>,
    mut text: Query<(&mut TextFont, &mut LineHeight)>,
) {
    let Some(hud) = hud else { return };
    let hud = hud.into_inner();
    let (sw, sh) = (
        window.physical_width() as i32,
        window.physical_height() as i32,
    );
    let k = window.scale_factor();
    let state = GunState {
        can_fire: [true, true],
        placable: hud.placable,
        last_fired: gun.last_fired,
        holding: false,
    };
    let painted = paint(
        sw,
        sh,
        &hud.sizes,
        &state,
        &mut hud.rings,
        time.delta_secs(),
    );
    let place = |node: &mut Node, [x, y, w, h]: [i32; 4]| {
        node.left = Val::Px(x as f32 / k);
        node.top = Val::Px(y as f32 / k);
        node.width = Val::Px(w as f32 / k);
        node.height = Val::Px(h as f32 / k);
    };
    for (entity, s) in hud.sprites.iter().zip(&painted) {
        if let Ok(mut node) = nodes.get_mut(*entity) {
            place(&mut node, s.rect);
        }
        if let Ok(mut image) = sprites.get_mut(*entity) {
            let [x, y, w, h] = hud.rects[s.icon as usize].map(|v| v as f32);
            image.rect = Some(Rect::new(x + 0.5, y + 0.5, x + w - 0.5, y + h - 0.5));
            let [r, g, b, a] = s.colour;
            image.color = Color::srgba_u8(r, g, b, a);
        }
    }
    let screen = UVec2::new(sw as u32, sh as u32);
    if screen == hud.screen {
        return;
    }
    hud.screen = screen;
    let tall = match font_tall(&hud.scheme, sh) {
        Ok(t) => t,
        Err(e) => {
            warn!("[hud] reticle hidden reason={e}");
            return;
        }
    };
    let w = (hud.advance * tall).round() as i32;
    let [x, y] = reticle_at(sw, sh, w, tall as i32);
    if let Ok((mut font, mut line)) = text.get_mut(hud.reticle) {
        font.font_size = FontSize::Px(tall / k);
        *line = LineHeight::Px(tall / k);
    }
    if let Ok(mut node) = nodes.get_mut(hud.reticle) {
        place(&mut node, [x, y, w, tall as i32]);
    }
    info!(
        "[hud] screen={sw}x{sh} scale={} reticle_tall={tall} reticle_at={x},{y}",
        portal::crosshair::scale(sh)
    );
}
