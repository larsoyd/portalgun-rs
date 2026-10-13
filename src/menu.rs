// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::path::PathBuf;

use bevy::input::InputSystems;
use bevy::prelude::*;
use bevy::text::{FontSize, FontSource};
use bevy::ui::UiSystems;
use bevy::window::{CursorGrabMode, CursorOptions};

use crate::options::{Improvement, Options, Page};
use crate::player::{Config, Controls};

#[derive(Resource, Default)]
pub struct Menu {
    pub options: Options,
    pub focus: usize,
    pub error: Option<String>,
    suppress_input: bool,
    pressed: Option<Entity>,
    keyboard: Option<usize>,
}

#[derive(Resource)]
pub struct SettingsPath(pub PathBuf);

#[derive(Component)]
struct MenuRoot;

#[derive(Resource, Default)]
struct MenuFont(Handle<Font>);

#[derive(Resource, Default)]
struct MenuSounds([Handle<AudioSource>; 3]);

#[derive(Component, Clone, Copy)]
pub enum MenuAction {
    Resume,
    Options,
    Quit,
    ConfirmQuit,
    Back,
    Toggle(Improvement),
    Restore,
    Ok,
    Cancel,
    Apply,
}

#[derive(Component)]
struct MenuItem(usize);

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Menu>()
            .init_resource::<MenuFont>()
            .init_resource::<MenuSounds>()
            .add_systems(Startup, load_resources)
            .add_systems(PreUpdate, input.after(InputSystems).after(UiSystems::Focus))
            .add_systems(Update, (draw, highlight).chain());
    }
}

fn load_resources(
    root: Res<crate::hud::InstallRoot>,
    mut fonts: ResMut<Assets<Font>>,
    mut font: ResMut<MenuFont>,
    files: Res<crate::sound::Files>,
    mut audio: ResMut<Assets<AudioSource>>,
    mut sounds: ResMut<MenuSounds>,
    mut exit: MessageWriter<AppExit>,
) {
    let path = root.0.join("hl2/resource/dejavusans.ttf");
    match std::fs::read(&path) {
        Ok(bytes) => font.0 = fonts.add(Font::from_bytes(bytes)),
        Err(reason) => {
            error!("[menu] refused: font={} reason={reason}", path.display());
            exit.write(AppExit::error());
        }
    }
    for (index, name) in ["buttonrollover", "buttonclick", "buttonclickrelease"]
        .into_iter()
        .enumerate()
    {
        let path = format!("sound/ui/{name}.wav");
        match files.read(&path) {
            Ok(bytes) => {
                sounds.0[index] = audio.add(AudioSource {
                    bytes: bytes.into(),
                })
            }
            Err(reason) => {
                error!("[menu] refused: sound={path} reason={reason}");
                exit.write(AppExit::error());
            }
        }
    }
}

pub fn playing(menu: Option<Res<Menu>>) -> bool {
    menu.is_none_or(|menu| menu.options.page == Page::Closed && !menu.suppress_input)
}

fn actions(page: Page) -> Vec<MenuAction> {
    match page {
        Page::Closed => vec![],
        Page::Quit => vec![MenuAction::ConfirmQuit, MenuAction::Back],
        Page::Pause => vec![MenuAction::Resume, MenuAction::Options, MenuAction::Quit],
        Page::Options => Improvement::ALL
            .into_iter()
            .map(MenuAction::Toggle)
            .chain([
                MenuAction::Restore,
                MenuAction::Ok,
                MenuAction::Cancel,
                MenuAction::Apply,
            ])
            .collect(),
    }
}

#[allow(clippy::too_many_arguments)]
fn input(
    mut commands: Commands,
    sounds: Res<MenuSounds>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    buttons: Query<(Entity, &Interaction, &MenuAction, &MenuItem), Changed<Interaction>>,
    mut menu: ResMut<Menu>,
    mut config: ResMut<Config>,
    path: Res<SettingsPath>,
    mut controls: ResMut<Controls>,
    mut time: ResMut<Time<Virtual>>,
    mut cursors: Query<&mut CursorOptions>,
    mut exit: MessageWriter<AppExit>,
) {
    if menu.suppress_input {
        menu.suppress_input = false;
    }
    let before = menu.options.page;
    let mut action = None;
    if keys.just_pressed(KeyCode::Escape) {
        menu.options.escape();
        menu.focus = 0;
        menu.error = None;
    } else if before != Page::Closed {
        let count = actions(before).len();
        if keys.just_pressed(KeyCode::ArrowUp) {
            menu.focus = (menu.focus + count - 1) % count;
        }
        if keys.just_pressed(KeyCode::ArrowDown) || keys.just_pressed(KeyCode::Tab) {
            menu.focus = (menu.focus + 1) % count;
        }
        // Translated from 0x26a500 and 0x269100 (disassembly, GameUI.so)
        if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space) {
            if before == Page::Pause {
                action = Some(actions(before)[menu.focus]);
            } else {
                menu.keyboard = Some(menu.focus);
                ui_sound(&mut commands, &sounds, 1);
            }
        }
        if (keys.just_released(KeyCode::Enter) || keys.just_released(KeyCode::Space))
            && menu.keyboard.take() == Some(menu.focus)
        {
            action = Some(actions(before)[menu.focus]);
        }
        // Translated from 0x269fa0 and 0x26a180 (disassembly, GameUI.so)
        for (entity, interaction, command, item) in &buttons {
            if *interaction != Interaction::None {
                menu.focus = item.0;
            }
            match *interaction {
                Interaction::Pressed => {
                    menu.pressed = Some(entity);
                    ui_sound(&mut commands, &sounds, 1);
                    if before == Page::Pause || mouse.just_released(MouseButton::Left) {
                        action = Some(*command);
                    }
                }
                Interaction::Hovered
                    if menu.pressed == Some(entity) && !mouse.pressed(MouseButton::Left) =>
                {
                    menu.pressed = None;
                    action = Some(*command);
                }
                Interaction::Hovered => ui_sound(&mut commands, &sounds, 0),
                Interaction::None
                    if menu.pressed == Some(entity) && !mouse.pressed(MouseButton::Left) =>
                {
                    menu.pressed = None
                }
                Interaction::None => {}
            }
        }
    }
    if matches!(action, Some(MenuAction::Apply)) && menu.options.draft.as_ref() == Some(&config.0) {
        action = None;
    }
    if mouse.just_released(MouseButton::Left) {
        menu.pressed = None;
    }
    if let Some(action) = action {
        ui_sound(&mut commands, &sounds, 2);
        menu.pressed = None;
        menu.error = None;
        match action {
            MenuAction::Resume => menu.options.page = Page::Closed,
            MenuAction::Options => {
                menu.options.open(&config.0);
                menu.focus = 0;
            }
            MenuAction::Quit => {
                menu.options.page = Page::Quit;
                menu.focus = 1;
            }
            MenuAction::ConfirmQuit => {
                exit.write(AppExit::Success);
            }
            MenuAction::Back => {
                menu.options.page = Page::Pause;
                menu.focus = 2;
            }
            MenuAction::Toggle(improvement) => {
                if let Some(draft) = &mut menu.options.draft {
                    improvement.toggle(draft);
                }
            }
            MenuAction::Restore => menu.options.restore_original(),
            MenuAction::Cancel => {
                menu.options.cancel();
                menu.focus = 1;
            }
            MenuAction::Ok | MenuAction::Apply => {
                if menu
                    .options
                    .draft
                    .as_ref()
                    .is_some_and(|draft| *draft != config.0)
                {
                    match menu.options.apply(&mut config.0, &path.0) {
                        Ok(()) => info!(
                            "[menu] settings=saved file={} effective={:?}",
                            path.0.display(),
                            config.0
                        ),
                        Err(reason) => {
                            error!("[menu] settings=failed reason={reason}");
                            menu.error = Some(format!("Could not save settings: {reason}"));
                        }
                    }
                }
                if matches!(action, MenuAction::Ok) && menu.error.is_none() {
                    menu.options.cancel();
                    menu.focus = 1;
                }
            }
        }
    }
    let after = menu.options.page;
    if after != before {
        menu.pressed = None;
        menu.keyboard = None;
        let paused = after != Page::Closed;
        if paused {
            time.pause();
        } else {
            time.unpause();
        }
        for mut cursor in &mut cursors {
            cursor.visible = paused;
            cursor.grab_mode = if paused {
                CursorGrabMode::None
            } else {
                CursorGrabMode::Locked
            };
        }
        *controls = Controls::default();
        keys.reset_all();
        mouse.reset_all();
        menu.suppress_input = true;
        info!("[menu] page={after:?} paused={paused} cursor_free={paused}");
    }
}

fn ui_sound(commands: &mut Commands, sounds: &MenuSounds, index: usize) {
    commands.spawn((
        AudioPlayer(sounds.0[index].clone()),
        PlaybackSettings::DESPAWN,
    ));
}

const TEXT: Color = Color::srgb_u8(221, 221, 221);
const ACCENT: Color = Color::srgb_u8(255, 155, 0);

fn label(text: impl Into<String>, size: f32, font: &Handle<Font>) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font_size: FontSize::Px(size),
            font: FontSource::Handle(font.clone()),
            ..default()
        },
        TextColor(TEXT),
    )
}

#[derive(PartialEq)]
struct DrawState(
    Page,
    Option<crate::settings::Settings>,
    Option<String>,
    crate::settings::Settings,
);

fn draw(
    mut commands: Commands,
    menu: Res<Menu>,
    config: Res<Config>,
    roots: Query<Entity, With<MenuRoot>>,
    font: Res<MenuFont>,
    mut previous: Local<Option<DrawState>>,
) {
    let state = DrawState(
        menu.options.page,
        menu.options.draft.clone(),
        menu.error.clone(),
        config.0.clone(),
    );
    if previous.as_ref() == Some(&state) {
        return;
    }
    *previous = Some(state);
    for entity in &roots {
        commands.entity(entity).despawn();
    }
    if menu.options.page == Page::Closed {
        return;
    }
    let label = |text: &str, size| label(text.to_owned(), size, &font.0);
    let options = menu.options.page == Page::Options;
    let root = commands
        .spawn((
            MenuRoot,
            GlobalZIndex(100),
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                align_items: AlignItems::Center,
                justify_content: if options || menu.options.page == Page::Quit {
                    JustifyContent::Center
                } else {
                    JustifyContent::FlexStart
                },
                padding: UiRect::left(percent(8)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.65)),
        ))
        .id();
    let panel = commands
        .spawn((
            Node {
                width: percent(90),
                max_width: px(640),
                padding: UiRect::all(px(24)),
                flex_direction: FlexDirection::Column,
                row_gap: px(6),
                ..default()
            },
            BackgroundColor(if options || menu.options.page == Page::Quit {
                Color::srgba_u8(132, 132, 132, 212)
            } else {
                Color::NONE
            }),
            ChildOf(root),
        ))
        .id();
    commands.spawn((
        label(
            match menu.options.page {
                Page::Options => "OPTIONS",
                Page::Quit => "QUIT GAME",
                _ => "PORTAL",
            },
            28.0,
        ),
        ChildOf(panel),
    ));
    if options {
        let close = commands
            .spawn((
                Button,
                MenuAction::Cancel,
                MenuItem(8),
                Node {
                    position_type: PositionType::Absolute,
                    right: px(12),
                    top: px(12),
                    width: px(28),
                    height: px(28),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BackgroundColor(Color::NONE),
                ChildOf(panel),
            ))
            .id();
        commands.spawn((label("×", 20.0), ChildOf(close)));
    }
    if menu.options.page == Page::Quit {
        commands.spawn((
            label("Are you sure you want to quit?", 18.0),
            ChildOf(panel),
        ));
    }
    if options {
        commands.spawn((label("Improvements", 21.0), ChildOf(panel)));
        commands.spawn((
            label(
                "Original behavior is the default. Check to enable an improvement.",
                16.0,
            ),
            ChildOf(panel),
        ));
    }
    let mut footer = None;
    for (index, action) in actions(menu.options.page).into_iter().enumerate() {
        let text = match action {
            MenuAction::Resume => "RESUME GAME".into(),
            MenuAction::Options => "OPTIONS".into(),
            MenuAction::Quit => "QUIT".into(),
            MenuAction::ConfirmQuit => "Quit game".into(),
            MenuAction::Back => "Cancel".into(),
            MenuAction::Toggle(improvement) => {
                let checked = menu
                    .options
                    .draft
                    .as_ref()
                    .is_some_and(|s| improvement.enabled(s));
                format!(
                    "[{}]  {}",
                    if checked { "x" } else { " " },
                    improvement.label()
                )
            }
            MenuAction::Restore => "Use original behavior".into(),
            MenuAction::Ok => "OK".into(),
            MenuAction::Cancel => "Cancel".into(),
            MenuAction::Apply => "Apply".into(),
        };
        let enabled = !matches!(action, MenuAction::Apply)
            || menu
                .options
                .draft
                .as_ref()
                .is_some_and(|draft| *draft != config.0);
        let parent = if matches!(
            action,
            MenuAction::Ok | MenuAction::Cancel | MenuAction::Apply
        ) {
            *footer.get_or_insert_with(|| {
                commands
                    .spawn((
                        Node {
                            justify_content: JustifyContent::FlexEnd,
                            column_gap: px(12),
                            margin: UiRect::top(px(16)),
                            ..default()
                        },
                        ChildOf(panel),
                    ))
                    .id()
            })
        } else {
            panel
        };
        let button = commands
            .spawn((
                Button,
                action,
                MenuItem(index),
                Node {
                    min_height: px(34),
                    padding: UiRect::axes(px(10), px(4)),
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::NONE),
                ChildOf(parent),
            ))
            .id();
        commands
            .spawn((
                label(&text, if options { 18.0 } else { 24.0 }),
                ChildOf(button),
            ))
            .insert(TextColor(if enabled {
                TEXT
            } else {
                Color::srgb_u8(128, 128, 128)
            }));
    }
    if let Some(error) = &menu.error {
        commands
            .spawn((label(error, 16.0), ChildOf(panel)))
            .insert(TextColor(ACCENT));
    }
    commands.spawn((
        label(
            "Esc: resume    Arrow keys / Tab: select    Enter: activate",
            14.0,
        ),
        ChildOf(panel),
    ));
}

fn highlight(menu: Res<Menu>, mut buttons: Query<(&MenuItem, &mut BackgroundColor)>) {
    for (item, mut background) in &mut buttons {
        background.0 = if item.0 == menu.focus {
            Color::srgba(1.0, 0.61, 0.0, 0.18)
        } else {
            Color::NONE
        };
    }
}
