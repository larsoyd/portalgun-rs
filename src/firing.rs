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

pub struct FiringPlugin;

impl Plugin for FiringPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<crate::impacts::PortalImpact>()
            .init_resource::<Firing>()
            .add_systems(Startup, |mut gun: ResMut<Firing>| gun.deploy(0.0))
            .add_systems(Update, (fire, land).chain().run_if(crate::menu::playing));
    }
}

#[allow(clippy::too_many_arguments)]
fn fire(
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
) {
    let now = time.elapsed_secs();
    let pressed = [
        (Colour::Blue, std::mem::take(&mut controls.fire1)),
        (Colour::Orange, std::mem::take(&mut controls.fire2)),
    ];
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
