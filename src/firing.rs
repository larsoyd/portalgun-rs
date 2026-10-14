// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use bevy::prelude::*;
use portal::placement::Placement;

use crate::faces::Portals;
use crate::gun::Gun;
use crate::player::{Controls, Player};
use crate::portals::Colour;
use crate::scene::Level;
use crate::sound::{Files, Sounds};
use crate::view::Gun as ViewGun;

#[derive(Resource, Default, Deref, DerefMut)]
pub struct Firing(pub Gun);

/// Shot streams in flight, their particle effect and whether the head has landed.
#[derive(Resource, Default)]
pub struct Streams(Vec<(u64, crate::gun::Shot, bool)>);

pub struct FiringPlugin;

impl Plugin for FiringPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<crate::impacts::PortalImpact>()
            .init_resource::<Firing>()
            .init_resource::<Streams>()
            .add_systems(Startup, |mut gun: ResMut<Firing>| gun.deploy(0.0))
            .add_systems(
                Update,
                (fizzle.before(crate::objects::ObjectInteraction), fire, land)
                    .chain()
                    .run_if(crate::menu::playing),
            )
            .add_systems(
                Update,
                stream
                    .after(fire)
                    .before(crate::effects::ParticleSimulation)
                    .run_if(crate::menu::playing),
            );
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn fire(
    mut commands: Commands,
    time: Res<Time>,
    mut controls: ResMut<Controls>,
    mut gun: ResMut<Firing>,
    mut player: ResMut<Player>,
    level: Res<Level>,
    portals: Res<Portals>,
    mut view: Option<ResMut<ViewGun>>,
    mut body: Option<ResMut<crate::body::BodyAnimation>>,
    mut sounds: ResMut<Sounds>,
    files: Res<Files>,
    mut audio: ResMut<Assets<AudioSource>>,
    mut rng: Local<crate::random::Xorshift>,
    objects: Option<Res<crate::objects::Objects>>,
    (mut fx, mut streams): (
        Option<ResMut<crate::effects::ParticleEffects>>,
        ResMut<Streams>,
    ),
) {
    let now = time.elapsed_secs();
    let pressed = [
        (Colour::Blue, std::mem::take(&mut controls.fire1)),
        (Colour::Orange, std::mem::take(&mut controls.fire2)),
    ];
    if objects.is_some_and(|o| o.holding() || now < o.blocked_until) {
        return;
    }
    for (colour, _) in pressed.into_iter().filter(|(_, p)| *p) {
        let Some(shot) = gun.fire(colour, now, &player, &level.0, &portals) else {
            info!("[gun] wait colour={colour:?} now={now:.2}");
            continue;
        };
        info!(
            "[gun] fire colour={colour:?} from={:?} landed={:?} arrives={:.3}",
            shot.from.map(|v| v.round()),
            shot.landed.map(|v| v.round()),
            shot.arrives
        );
        if let Some(fx) = fx.as_deref_mut() {
            let frame = crate::effects::Frame::stream(shot.from);
            let id = fx.spawn(colour, crate::effects::Effect::Stream, frame, None);
            streams.0.push((id, shot.clone(), false));
        }
        sounds.play(
            &mut commands,
            &files,
            &mut audio,
            &format!("Weapon_Portalgun.fire_{}", colour.sound()),
        );
        if let Some(body) = body.as_deref_mut() {
            body.fire(now);
        }
        let kick = crate::gun::fire_punch(&mut rng);
        player.punch.kick(kick);
        info!("[view] punch fire pitch={:.2} yaw={:.2}", kick[0], kick[1]);
        if let Some(view) = view.as_deref_mut() {
            let crate::view::Gun { mdl, anim, .. } = view;
            anim.play(mdl, "ACT_VM_PRIMARYATTACK", now);
        }
    }
}

/// Moves each stream head along its path, holds it on the end for a frame, then lets it fade.
/// Translated from 0x560a00 (decompiled, client.so)
fn stream(
    time: Res<Time>,
    mut streams: ResMut<Streams>,
    fx: Option<ResMut<crate::effects::ParticleEffects>>,
) {
    let Some(mut fx) = fx else { return };
    let now = time.elapsed_secs();
    streams.0.retain_mut(|(id, shot, landed)| {
        match shot.stream_step(now, *landed) {
            crate::gun::StreamStep::Fly(at) => fx.move_point(*id, at),
            crate::gun::StreamStep::Land(at) => {
                fx.move_point(*id, at);
                *landed = true;
            }
            crate::gun::StreamStep::Done => {
                fx.stop(*id);
                return false;
            }
        }
        true
    });
}

#[allow(clippy::too_many_arguments)]
fn land(
    mut commands: Commands,
    mut impacts: MessageWriter<crate::impacts::PortalImpact>,
    time: Res<Time>,
    mut gun: ResMut<Firing>,
    mut portals: ResMut<Portals>,
    mut sounds: ResMut<Sounds>,
    files: Res<Files>,
    mut audio: ResMut<Assets<AudioSource>>,
) {
    for shot in gun.land(time.elapsed_secs()) {
        if let (Some(effect), Some(at)) = (crate::impacts::selection(&shot.placement), shot.impact)
        {
            let mut impact = crate::impacts::PortalImpact::at(shot.colour, effect, &at);
            if effect == crate::effects::Effect::Overlap {
                impact.target = portals.get(shot.colour.other()).map(|p| p.origin);
            }
            impacts.write(impact);
        }
        match shot.placement {
            Placement::Placed { portal, quality } => {
                info!(
                    "[place] ok colour={:?} origin={:?} forward={:?} quality={quality:.2}",
                    shot.colour,
                    portal.origin.map(|v| v.round()),
                    portal.forward
                );
                portals.place(shot.colour, portal);
                sounds.play(
                    &mut commands,
                    &files,
                    &mut audio,
                    &format!("Portal.open_{}", shot.colour.sound()),
                );
            }
            Placement::Failed(reason) => {
                info!(
                    "[place] refused colour={:?} reason={reason:?} at={:?}",
                    shot.colour,
                    shot.landed.map(|v| v.round())
                );
            }
        }
    }
}

fn fizzle(
    mut controls: ResMut<Controls>,
    mut gun: ResMut<Firing>,
    mut portals: ResMut<Portals>,
    mut visual: Option<ResMut<crate::gun_visual::GunVisualState>>,
) {
    if !std::mem::take(&mut controls.fizzle)
        || (portals.open.iter().all(Option::is_none) && gun.flying().is_empty())
    {
        return;
    }
    for colour in Colour::ALL {
        portals.deactivate(colour);
    }
    gun.fizzle();
    if let Some(visual) = visual.as_deref_mut() {
        visual.fizzle_revision += 1;
    }
    info!("[gun] fizzle portals=cleared pending=0");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gun_visual::GunVisualState;
    use portal::pair::Portal;

    #[test]
    fn release_consumes_early_shots_and_allows_a_new_click_at_half_a_second() {
        let home = std::env::var("HOME").unwrap_or_default();
        let dir = portal::install::root(None, None, std::path::Path::new(&home));
        let install = portal::install::Install::open(&dir).unwrap();
        let sounds = Sounds::read(&install.search).unwrap();
        let level = crate::chamber::Chamber::test_chamber(&install.search).unwrap();
        let mut objects = crate::objects::Objects::default();
        objects.carry = Some(portal::carry::Carry::new(
            portal::carry::IDENTITY,
            0.0,
            None,
        ));
        objects.release(10.0);
        let mut clock = Time::<()>::default();
        clock.advance_by(std::time::Duration::from_millis(10499));
        let mut app = App::new();
        app.insert_resource(clock)
            .insert_resource(objects)
            .insert_resource(Player(crate::walker::Walker::at([-200.0, 0.0, 0.0])))
            .insert_resource(Level(level))
            .insert_resource(Files(install.search))
            .insert_resource(sounds)
            .init_resource::<Controls>()
            .init_resource::<Firing>()
            .init_resource::<Streams>()
            .init_resource::<Portals>()
            .init_resource::<Assets<AudioSource>>()
            .add_systems(Update, fire);
        app.world_mut().resource_mut::<Controls>().fire1 = true;
        app.update();
        assert!(app.world().resource::<Firing>().flying().is_empty());
        assert!(
            !app.world().resource::<Controls>().fire1,
            "blocked click must be consumed"
        );
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(1));
        app.update();
        assert!(
            app.world().resource::<Firing>().flying().is_empty(),
            "the old click must not fire later"
        );
        app.world_mut().resource_mut::<Controls>().fire2 = true;
        app.update();
        let gun = app.world().resource::<Firing>();
        assert_eq!(gun.flying().len(), 1);
        assert_eq!(gun.flying()[0].colour, Colour::Orange);
        assert_eq!(gun.flying()[0].launched, 10.5);
    }

    #[test]
    fn clearing_an_open_portal_triggers_one_fizzle_and_empty_clears_do_nothing() {
        let mut app = App::new();
        app.init_resource::<Controls>()
            .init_resource::<Firing>()
            .init_resource::<Streams>()
            .init_resource::<Portals>()
            .init_resource::<GunVisualState>()
            .add_systems(Update, fizzle);
        app.world_mut().resource_mut::<Portals>().place(
            Colour::Blue,
            Portal::new([0.0; 3], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], (32.0, 54.0)),
        );
        app.world_mut().resource_mut::<Firing>().last_fired = 1;
        app.world_mut().resource_mut::<Controls>().fizzle = true;
        app.update();
        assert!(app
            .world()
            .resource::<Portals>()
            .open
            .iter()
            .all(Option::is_none));
        assert_eq!(app.world().resource::<Firing>().last_fired, 0);
        assert_eq!(app.world().resource::<GunVisualState>().fizzle_revision, 1);
        assert!(!app.world().resource::<Controls>().fizzle);
        app.world_mut().resource_mut::<Controls>().fizzle = true;
        app.update();
        assert_eq!(app.world().resource::<GunVisualState>().fizzle_revision, 1);
    }
}
