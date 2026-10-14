// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use bevy::prelude::*;
use portal::pair::Portal;
use portal::placement::{Failure, Placement};

use crate::effects::{Effect, Frame, ParticleEffects, ParticleSimulation};
use crate::faces::{PortalViewsUpdated, Portals};
use crate::portals::Colour;
use crate::sound::{Files, Sounds};

pub fn selection(placement: &Placement) -> Option<Effect> {
    match placement {
        Placement::Placed { .. } => Some(Effect::Success),
        Placement::Failed(Failure::CantFit) => Some(Effect::NoFit),
        Placement::Failed(Failure::BadSurface) => Some(Effect::BadSurface),
        Placement::Failed(Failure::Overlap) => Some(Effect::Overlap),
        Placement::Failed(Failure::Missed | Failure::PassThrough) => None,
    }
}

#[derive(Message, Clone, Copy)]
pub struct PortalImpact {
    pub colour: Colour,
    pub effect: Effect,
    pub frame: Frame,
    pub target: Option<[f32; 3]>,
}
impl PortalImpact {
    pub fn at(colour: Colour, effect: Effect, portal: &Portal) -> Self {
        Self {
            colour,
            effect,
            frame: Frame::impact(portal, if effect == Effect::Near { 16.0 } else { 1.0 }),
            target: None,
        }
    }
}

#[derive(Resource, Default)]
struct Previous {
    open: [Option<Portal>; 2],
    revisions: [u64; 2],
}

pub struct PortalImpactsPlugin;
impl Plugin for PortalImpactsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PortalImpact>()
            .init_resource::<Previous>()
            .add_systems(
                PostUpdate,
                (closures, dispatch)
                    .chain()
                    .after(PortalViewsUpdated)
                    .before(ParticleSimulation),
            );
    }
}
fn closures(
    pair: Res<Portals>,
    mut previous: ResMut<Previous>,
    mut impacts: MessageWriter<PortalImpact>,
) {
    for c in Colour::ALL {
        let i = c.index();
        if pair.open[i] != previous.open[i] || pair.revisions[i] != previous.revisions[i] {
            if let Some(portal) = previous.open[i] {
                impacts.write(PortalImpact::at(c, Effect::Close, &portal));
            }
            previous.open[i] = pair.open[i];
            previous.revisions[i] = pair.revisions[i];
        }
    }
}
#[allow(clippy::too_many_arguments)]
fn dispatch(
    mut commands: Commands,
    mut messages: MessageReader<PortalImpact>,
    mut fx: ResMut<ParticleEffects>,
    mut sounds: ResMut<Sounds>,
    files: Res<Files>,
    mut audio: ResMut<Assets<AudioSource>>,
) {
    for impact in messages.read() {
        let id = fx.spawn(impact.colour, impact.effect, impact.frame, impact.target);
        info!(
            "[fx] impact effect={} id={id} live={}",
            impact.effect.name(impact.colour),
            fx.instance_count()
        );
        let sound = match impact.effect {
            Effect::Close => Some(format!("Portal.close_{}", impact.colour.sound())),
            Effect::Near | Effect::Overlap | Effect::BadVolume | Effect::BadSurface => {
                Some("Portal.fizzle_invalid_surface".into())
            }
            _ => None,
        };
        if let Some(sound) = sound {
            sounds.play(&mut commands, &files, &mut audio, &sound);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_dispatch_tracks_same_spot_replacement_and_deactivation_once() {
        let mut app = App::new();
        app.add_message::<PortalImpact>()
            .init_resource::<Portals>()
            .init_resource::<Previous>()
            .add_systems(Update, closures);
        let p = Portal::new(
            [100.0, 0.0, 64.0],
            [-1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            (32.0, 54.0),
        );
        app.world_mut()
            .resource_mut::<Portals>()
            .place(Colour::Blue, p);
        app.update();
        assert_eq!(
            app.world_mut()
                .resource_mut::<Messages<PortalImpact>>()
                .drain()
                .count(),
            0
        );
        app.world_mut()
            .resource_mut::<Portals>()
            .place(Colour::Blue, p);
        app.update();
        let impacts: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<PortalImpact>>()
            .drain()
            .collect();
        assert_eq!(impacts.len(), 1);
        assert_eq!(impacts[0].effect, Effect::Close);
        assert_eq!(impacts[0].colour, Colour::Blue);
        assert_eq!(impacts[0].frame, Frame::impact(&p, 1.0));
        app.world_mut()
            .resource_mut::<Portals>()
            .deactivate(Colour::Blue);
        app.update();
        assert_eq!(
            app.world_mut()
                .resource_mut::<Messages<PortalImpact>>()
                .drain()
                .count(),
            1
        );
        app.world_mut()
            .resource_mut::<Portals>()
            .deactivate(Colour::Blue);
        app.update();
        assert_eq!(
            app.world_mut()
                .resource_mut::<Messages<PortalImpact>>()
                .drain()
                .count(),
            0
        );
    }
}
