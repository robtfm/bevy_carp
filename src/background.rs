use bevy::prelude::*;
use rand::{thread_rng, Rng};

use crate::structs::{Permanent, ChangeBackground};

pub struct BackgroundPlugin;

impl Plugin for BackgroundPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_event::<ChangeBackground>()
            .add_system(add_background)
        ;
    }
}

#[derive(Component)]
pub struct MBackground;

fn add_background(
    mut evs: EventReader<ChangeBackground>,
    mut commands: Commands,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    old_backgrounds: Query<Entity, With<MBackground>>,
    server: Res<AssetServer>,
) {
    for _ in evs.iter() {
        for ent in old_backgrounds.iter() {
            commands.entity(ent).despawn_recursive();
        }

        let mut rng = thread_rng();
        let id = rng.gen_range(1..=3);
        let file = format!("images/lumber edit{}.png", id);
    
        let mat = StandardMaterial {
            base_color_texture: Some(server.load(&file)),
            unlit: true,
            ..Default::default()
        };
    
        commands.spawn_bundle(PbrBundle{
            mesh: meshes.add(shape::Quad::new(Vec2::new(950.0,500.0)).into()),
            material: mats.add(mat),
            transform: Transform::from_xyz(0.0, 0.0, -500.0),
            ..Default::default()
        }).insert(Permanent).insert(MBackground);    
    }
}