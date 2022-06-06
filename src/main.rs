//done
// clean up input, make configurable. check leafwing
// show controls in context
// sfx
// image generation and shader should take offset and orientation
// store state when things change, impl undo (make sure locals are empty & emptied)
// levels

//tbd
// splash
// music
// options (inc keys)
// quotes (in and out)
// background
// proper title page
// improve cutter

#![feature(let_else)]

use input::{Controller, InputPlugin};
use menus::{spawn_in_level_menu, spawn_main_menu, spawn_play_menu, spawn_popup_menu};
use rand::{
    prelude::{SliceRandom, StdRng},
    thread_rng, Rng, SeedableRng,
};

use bevy::{
    app::AppExit,
    prelude::{shape::UVSphere, *},
    render::{
        camera::Camera3d,
        render_resource::{Extent3d, TextureDimension},
    },
    utils::{HashMap, HashSet},
    window::WindowResized,
};

use bevy_egui::{
    egui::{self},
    EguiContext, EguiPlugin, EguiSettings,
};
use bevy_kira_audio::{AudioApp, AudioChannel, AudioPlugin};

mod bl_quad;
mod input;
mod menus;
mod model;
mod shader;
mod structs;
mod wood_material;

use bl_quad::BLQuad;
use model::*;
use shader::SimpleTextureMaterial;
use structs::{
    ActionEvent, GrabDropChannel, HammerChannel, LevelDef, MenuChannel, PopupMenuEvent, PositionZ,
    SpawnLevelEvent,
};
use wood_material::{WoodMaterial, WoodMaterialPlugin, WoodMaterialSpec};

use crate::structs::{PopupMenu, Position};

fn main() {
    // stage conventions to avoid adding components to despawned entities:
    // pre - ensure working state
    // update - add / spawn, don't despawn (unless it's locally used entities)
    // post - spawn / despawn, don't add

    let mut app = App::new();
    app.add_plugins(DefaultPlugins)
        .add_plugin(WoodMaterialPlugin)
        .add_plugin(EguiPlugin)
        .add_plugin(AudioPlugin)
        .add_plugin(InputPlugin)
        .add_audio_channel::<MenuChannel>()
        .add_audio_channel::<GrabDropChannel>()
        .add_audio_channel::<HammerChannel>()
        .init_resource::<Level>()
        .init_resource::<DonePlanks>()
        .init_resource::<LevelDef>()
        .init_resource::<LevelBase>()
        .init_resource::<LevelSet>()
        .init_resource::<UndoBuffer>()
        .insert_resource(AmbientLight {
            color: Color::rgba(0.8, 0.8, 1.0, 1.0),
            brightness: 0.1,
        })
        .insert_resource(ClearColor(Color::rgb(0.05, 0.05, 0.3)))
        .add_event::<SpawnLevelEvent>()
        .add_event::<PopupMenuEvent>()
        .add_event::<CutEvent>()
        .add_event::<ResetEvent>()
        .add_event::<SnapUndo>()
        .add_event::<SpawnNail>()
        .add_event::<SpawnPlank>()
        // egui
        .add_startup_system(egui_setup)
        .add_system(handle_window_resize)
        // menus
        .add_startup_system(setup_main_menu)
        .add_system(spawn_main_menu)
        .add_system(spawn_play_menu)
        .add_system(spawn_in_level_menu)
        .add_system(spawn_popup_menu)
        // setup level
        .add_system(setup_level) // generate the level from the def
        .add_system_to_stage(CoreStage::PreUpdate, create_level) // (re)spawn a level. should have its own stage really
        // level mechanics
        .add_system(target.before(grab_or_drop).before(hammer_home))
        .add_system(grab_or_drop)
        .add_system(rotate_plank)
        .add_system_to_stage(CoreStage::PostUpdate, cut_plank) // despawns -> postupdate
        .add_system(extend_cut.before(update_transforms))
        .add_system(draw_cuts.before(extend_cut)) // despawns but only things it is the only user of
        .add_system(hammer_home)
        .add_system(ensure_focus)
        .add_system(update_transforms)
        // visuals
        .add_system(update_materials)
        .add_system(spawn_planks)
        .add_system(spawn_nails)
        // system events
        .add_system_to_stage(CoreStage::PostUpdate, system_events)
        // undo/redo
        // records planks before and after, after requires commands completed, so cut_plank -> record gets prior state -> spawn_planks -> *cmds exec* -> record gets new state
        .add_system_to_stage(CoreStage::PostUpdate, record_state.after(cut_plank))
        .add_system_to_stage(CoreStage::PostUpdate, change_state)
        // camera management
        .add_system_to_stage(CoreStage::PostUpdate, camera_focus)
        
        .run();
}

#[derive(Default)]
struct ResetEvent {
    cursor_pos: Option<Position>,
    cursor_trans: Option<Transform>,
    camera_pos: Option<(Position, PositionZ)>,
    camera_trans: Option<Transform>,
}

#[derive(Component)]
struct Permanent;

fn setup_main_menu(mut evs: EventWriter<ActionEvent>) {
    evs.send(ActionEvent {
        sender: Entity::from_raw(0),
        label: "main menu",
        target: None,
    });
}

fn egui_setup(mut egui_ctx: ResMut<EguiContext>) {
    let widget_visuals = egui::style::WidgetVisuals {
        bg_fill: egui::Rgba::from_rgba_premultiplied(0.0, 0.0, 0.0, 0.8).into(),
        bg_stroke: egui::Stroke::new(1.0, egui::Rgba::from_rgba_premultiplied(0.0, 0.0, 0.0, 0.8)),
        rounding: egui::Rounding::same(5.0),
        fg_stroke: egui::Stroke::new(1.0, egui::Rgba::from_rgba_premultiplied(1.0, 1.0, 1.0, 1.0)),
        expansion: 0.0,
    };

    let widgets = egui::style::Widgets {
        noninteractive: widget_visuals.clone(),
        inactive: widget_visuals.clone(),
        active: widget_visuals.clone(),
        ..Default::default()
    };

    egui_ctx.ctx_mut().set_visuals(egui::Visuals {
        window_rounding: 0.0.into(),
        widgets,
        ..Default::default()
    });

    let mut fonts = egui::FontDefinitions::default();
    // Install my own font (maybe supporting non-latin characters).
    // .ttf and .otf files supported.
    fonts.font_data.insert(
        "my_font".to_owned(),
        egui::FontData::from_static(include_bytes!("../assets/fonts/CLIFF.ttf")),
    );

    // Put my font first (highest priority) for proportional text:
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "my_font".to_owned());

    // Tell egui to use these fonts:
    egui_ctx.ctx_mut().set_fonts(fonts);
}

fn handle_window_resize(
    mut events: EventReader<WindowResized>,
    windows: Res<Windows>,
    mut egui_settings: ResMut<EguiSettings>,
) {
    for _ in events.iter() {
        if let Some(window) = windows.get_primary() {
            egui_settings.scale_factor = f64::max(
                1.0,
                f32::min(window.height() / 720.0, window.width() / 1280.0) as f64,
            );
        }
    }
}

#[derive(Default, Clone)]
pub struct LevelSet([Option<LevelDef>; 30], usize);

fn spawn_random(
    spawn_evs: &mut EventWriter<SpawnLevelEvent>,
    levelset: &mut LevelSet,
    total: usize,
    skip: usize,
) {
    let mut rng = thread_rng();

    let mut defs = (0..total)
        .map(|_| {
            let seed = rng.gen();
            let num_holes = rng.gen_range(2..15);
            let total_blocks = rng.gen_range(0..7) + num_holes * rng.gen_range(3..9);
            Some(LevelDef {
                num_holes,
                total_blocks,
                seed,
            })
        })
        .collect::<Vec<_>>();

    defs.sort_by_key(|def| {
        let def = def.as_ref().unwrap();
        let mut rng = StdRng::seed_from_u64(def.seed);
        let holes = gen_holes(def.num_holes, def.total_blocks, &mut rng);
        let plank = Plank::from_holes(&holes, &mut rng);
        let level = Level {
            holes,
            planks: vec![(plank, Position::default())],
            ..Default::default()
        };
        (level.difficulty() * 100000.0) as i32
    });

    defs = defs.into_iter().skip(skip).take(30).collect();

    let defs: [Option<LevelDef>; 30] = match defs.try_into() {
        Ok(defs) => defs,
        Err(_) => panic!(),
    };

    *levelset = LevelSet(defs, 0);

    spawn_evs.send(SpawnLevelEvent {
        def: levelset.0[0].as_ref().unwrap().clone(),
    });
}

fn setup_level(
    mut spawn_evs: EventReader<SpawnLevelEvent>,
    mut base: ResMut<LevelBase>,
    mut def: ResMut<LevelDef>,
    mut action_evs: EventWriter<ActionEvent>,
    mut commands: Commands,
) {
    for ev in spawn_evs.iter() {
        let mut rng = StdRng::seed_from_u64(ev.def.seed);
        let mut holes = gen_holes(ev.def.num_holes, ev.def.total_blocks, &mut rng);
        holes
            .holes
            .sort_by(|a, b| a.size().y.cmp(&b.size().y).reverse());
        let mut plank = Plank::from_holes(&holes, &mut rng);
        plank.shift(IVec2::ONE);

        // arrange
        let count = holes.holes.len();
        let grid_y = (count as f32 / 2.0).sqrt().floor() as usize;
        let grid_x = (count as f32 / grid_y as f32).ceil() as usize;

        debug!("setup_level: count: {}, grid: {},{}", count, grid_x, grid_y);

        let mut extents = IVec2::ZERO;
        let mut grid_col = 0;
        let mut x_off = 1;
        let mut y_off = 1;
        let mut max_y_row = 0;

        for hole in holes.holes.iter_mut() {
            hole.shift(IVec2::new(x_off, y_off));
            let hole_extents = hole.extents();
            max_y_row = max_y_row.max(hole_extents.1 .1);
            x_off = hole_extents.0 .1 + 2;
            extents =
                extents.max(IVec2::new(hole_extents.0 .1, hole_extents.1 .1) + IVec2::ONE * 2);
            grid_col += 1;
            if grid_col == grid_x {
                grid_col = 0;
                x_off = 1;
                y_off = max_y_row + 2;
                max_y_row = 0;
            }
        }

        let uber_hole = Hole::merge(holes.holes.iter());
        debug!("uber hole: [{:?}] \n{}", uber_hole.extents(), uber_hole);
        debug!("plank: [{:?}]\n{}", plank.extents(), plank);

        let size = plank.size() + 2;
        let pos = IVec2::new(-size.x / 2, -size.y - 1);

        *base = LevelBase(Level {
            extents,
            holes,
            planks: vec![(plank, Position(pos))],
            setup: true,
        });
        *def = ev.def.clone();
        action_evs.send(ActionEvent {
            sender: Entity::from_raw(0),
            label: "restart",
            target: None,
        });
        commands.insert_resource(UndoBuffer::new(base.0.clone()));
        commands.insert_resource(DonePlanks::default());
    }
}

fn create_coordset_image<'a>(images: &mut Assets<Image>, coords: &CoordSet) -> Handle<Image> {
    let size = IVec2::new(coords.extents().0 .1 + 1, coords.extents().1 .1 + 1);
    debug!("dims: {}", size);
    let mut data = Vec::from_iter(std::iter::repeat(0u8).take((size.x * size.y) as usize));

    for coord in coords.coords.iter() {
        data[(coord.x + coord.y * size.x) as usize] = 1;
    }

    let image = Image::new(
        Extent3d {
            width: size.x as u32,
            height: size.y as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        bevy::render::render_resource::TextureFormat::R8Uint,
    );

    images.add(image)
}

fn create_level(
    mut evs: EventReader<ResetEvent>,
    old: Query<Entity, Without<Permanent>>,
    mut commands: Commands,
    level: Res<Level>,
    done_planks: Res<DonePlanks>,
    mut spawn_nails: EventWriter<SpawnNail>,
    mut spawn_planks: EventWriter<SpawnPlank>,
    (mut meshes, mut std_mats): (ResMut<Assets<Mesh>>, ResMut<Assets<StandardMaterial>>),
    mut focus: EventWriter<ActionEvent>,
) {
    for ev in evs.iter() {
        for ent in old.iter() {
            commands.entity(ent).despawn_recursive();
        }

        commands.spawn().insert(Controller {
            action: vec![
                ("pause", ("menu", true), true),
                ("undo", ("third action", true), true),
                ("redo", ("fourth action", true), true),
            ],
            enabled: true,
            ..Default::default()
        });

        let size = level.extents;
        let pos = IVec2::new(-size.x / 2, 1);
        let merger = CoordSet::merge(level.holes.holes.iter());
        spawn_planks.send(SpawnPlank(
            merger,
            Position(pos),
            false,
            true,
            Some(level.extents),
        ));

        for plank in level.planks.iter() {
            spawn_planks.send(SpawnPlank(plank.0.clone(), plank.1, true, true, None));
        }

        for (plank, pos, nails) in done_planks.0.iter() {
            spawn_planks.send(SpawnPlank(plank.clone(), *pos, true, false, None));
            for coord in nails.iter() {
                spawn_nails.send(SpawnNail(*coord));
            }
        }

        let (cam_pos, cam_z) = ev
            .camera_pos
            .unwrap_or((Position::default(), PositionZ(20)));
        let cam_trans = ev
            .camera_trans
            .unwrap_or(Transform::from_xyz(0.0, 0.0, 20.0).looking_at(Vec3::ZERO, Vec3::Y));
        let cam_id = commands
            .spawn_bundle(PerspectiveCameraBundle {
                perspective_projection: PerspectiveProjection {
                    fov: std::f32::consts::PI / 4.0,
                    ..Default::default()
                },
                transform: cam_trans,
                ..default()
            })
            .insert(cam_pos)
            .insert(cam_z)
            .insert(Controller {
                display_directions: Some("Pan"),
                enabled: true,
                forward: ("zoom in", false),
                back: ("zoom out", false),
                left: ("pan left", false),
                right: ("pan right", false),
                up: ("pan up", false),
                down: ("pan down", false),
                action: vec![("focus", ("select all", true), true)],
                ..Default::default()
            })
            .id();

        commands
            .spawn_bundle((
                ev.cursor_trans
                    .unwrap_or(Transform::from_xyz(0.0, 0.0, 0.3)),
                GlobalTransform::default(),
            ))
            .insert(ev.cursor_pos.unwrap_or_default())
            .insert(ExtentItem(IVec2::ONE, IVec2::ONE))
            .insert(Cursor)
            .insert(Controller {
                display_directions: Some("Move"),
                enabled: true,
                left: ("move left", false),
                right: ("move right", false),
                up: ("move up", false),
                down: ("move down", false),
                action: vec![
                    ("grab", ("main action", true), true),
                    ("cut", ("second action", true), true),
                ],
                ..Default::default()
            })
            .with_children(|p| {
                p.spawn_bundle(PbrBundle {
                    mesh: meshes.add(
                        UVSphere {
                            radius: 0.5,
                            ..Default::default()
                        }
                        .into(),
                    ),
                    material: std_mats.add(Color::WHITE.into()),
                    transform: Transform::from_xyz(0.5, 0.5, 0.0),
                    ..Default::default()
                });
                p.spawn_bundle(PointLightBundle {
                    transform: Transform::from_xyz(0.0, 0.0, 8.0),
                    point_light: PointLight {
                        color: Color::rgba(1.0, 1.0, 0.8, 1.0),
                        intensity: 1000.0,
                        range: 50.0,
                        ..Default::default()
                    },
                    ..Default::default()
                });
            });

        focus.send(ActionEvent {
            sender: cam_id,
            label: "focus",
            target: None,
        });
    }
}

#[derive(Component)]
struct ExtentItem(IVec2, IVec2);

#[derive(Component)]
struct MoveSpeed(f32);

#[derive(Component)]
struct PrevPosition(pub IVec2);

fn update_transforms(
    time: Res<Time>,
    mut q: Query<(
        &mut Transform,
        &Position,
        Option<&PositionZ>,
        Option<&MoveSpeed>,
    )>,
) {
    for (mut transform, position, maybe_posz, maybe_speed) in q.iter_mut() {
        let speed = maybe_speed.unwrap_or(&MoveSpeed(15.0)).0;
        if transform.translation.x < position.0.x as f32 {
            transform.translation.x = f32::min(
                position.0.x as f32,
                transform.translation.x
                    + time.delta_seconds()
                        * f32::max(1.0, position.0.x as f32 - transform.translation.x)
                        * speed,
            );
        } else {
            transform.translation.x = f32::max(
                position.0.x as f32,
                transform.translation.x
                    - time.delta_seconds()
                        * f32::max(1.0, transform.translation.x - position.0.x as f32)
                        * speed,
            );
        }
        if transform.translation.y < position.0.y as f32 {
            transform.translation.y = f32::min(
                position.0.y as f32,
                transform.translation.y
                    + time.delta_seconds()
                        * f32::max(1.0, position.0.y as f32 - transform.translation.y)
                        * speed,
            );
        } else {
            transform.translation.y = f32::max(
                position.0.y as f32,
                transform.translation.y
                    - time.delta_seconds()
                        * f32::max(1.0, transform.translation.y - position.0.y as f32)
                        * speed,
            );
        }

        if let Some(&PositionZ(posz)) = maybe_posz {
            if transform.translation.z < posz as f32 {
                transform.translation.z = f32::min(
                    posz as f32,
                    transform.translation.z
                        + time.delta_seconds()
                            * f32::max(1.0, posz as f32 - transform.translation.z)
                            * speed
                            * 2.0,
                );
            } else {
                transform.translation.z = f32::max(
                    posz as f32,
                    transform.translation.z
                        - time.delta_seconds()
                            * f32::max(1.0, transform.translation.z - posz as f32)
                            * speed
                            * 2.0,
                );
            }
        }
    }
}

#[derive(Component)]
struct Selected;

#[derive(Component)]
struct Targeted;

#[derive(Component)]
struct Cursor;

#[derive(Component)]
struct PlankComponent(Plank, Handle<WoodMaterial>);

#[derive(Default, Clone)]
struct DonePlanks(Vec<(Plank, Position, Vec<IVec2>)>);

#[derive(Component)]
struct MHoles;

#[derive(Component)]
struct RotateAround(IVec2);

fn update_materials(
    time: Res<Time>,
    mut mats: ResMut<Assets<WoodMaterial>>,
    selected: Query<&PlankComponent, With<Selected>>,
    targeted: Query<&PlankComponent, With<Targeted>>,
    neither: Query<&PlankComponent, (Without<Selected>, Without<Targeted>)>,
) {
    let mult = (((time.seconds_since_startup() * 6.0).sin() + 1.0) / 4.0) as f32 + 0.5;
    for plank in selected.iter() {
        if let Some(mat) = mats.get_mut(plank.1.clone_weak()) {
            mat.0.hilight_color = Color::rgba(0.2 * mult, 0.2 * mult, mult, 1.0);
        }
    }
    for plank in targeted.iter() {
        if let Some(mat) = mats.get_mut(plank.1.clone_weak()) {
            mat.0.hilight_color = Color::rgba(0.5 * mult, 0.5 * mult, 0.5 * mult, 1.0);
        }
    }
    for plank in neither.iter() {
        if let Some(mat) = mats.get_mut(plank.1.clone_weak()) {
            mat.0.hilight_color = Color::BLACK;
        }
    }
}

fn camera_focus(
    mut evs: EventReader<ActionEvent>,
    mut cam: Query<(&mut Position, &mut PositionZ, &PerspectiveProjection), Without<ExtentItem>>,
    all: Query<(Entity, &Position, &ExtentItem)>,
) {
    for ev in evs.iter() {
        if let Ok((mut pos, mut z, cam)) = cam.get_mut(ev.sender) {
            let mut min_x = i32::MAX;
            let mut max_x = i32::MIN;
            let mut min_y = i32::MAX;
            let mut max_y = i32::MIN;
            let mut count = 0;

            for (_, pos, extent) in all
                .iter()
                .filter(|(e, ..)| ev.target.is_none() || ev.target.as_ref().unwrap() == e)
            {
                min_x = i32::min(min_x, pos.0.x + extent.0.x);
                max_x = i32::max(max_x, pos.0.x + extent.1.x);
                min_y = i32::min(min_y, pos.0.y + extent.0.y);
                max_y = i32::max(max_y, pos.0.y + extent.1.y);
                count += 1;
            }

            if count > 0 {
                min_x -= 1;
                max_x += 2;
                min_y -= 1;
                max_y += 2;

                let x_scale = 1.0 * cam.aspect_ratio;
                let y_scale = 1.0;
                let z_scale = 0.4;

                let target_z = (f32::max(
                    (max_x - min_x) as f32 / x_scale,
                    (max_y - min_y) as f32 * y_scale,
                ) / (2.0 * z_scale))
                    .ceil() as i32;

                if target_z > z.0 {
                    z.0 = target_z;
                }

                let x_range = (z.0 as f32 * x_scale * z_scale) as i32;
                let y_range = (z.0 as f32 * y_scale * z_scale) as i32;

                if min_x < pos.0.x - x_range {
                    pos.0.x = min_x + x_range;
                }
                if max_x > pos.0.x + x_range {
                    pos.0.x = max_x - x_range;
                }
                if min_y < pos.0.y - y_range {
                    pos.0.y = min_y + y_range;
                }
                if max_y > pos.0.y + y_range {
                    pos.0.y = max_y - y_range;
                }
            }
        }
    }
}

fn ensure_focus(
    cam: Query<Entity, With<Camera3d>>,
    cursor: Query<Entity, With<Cursor>>,
    selected: Query<Entity, With<Selected>>,
    mut action: EventWriter<ActionEvent>,
) {
    if let Ok(cam) = cam.get_single() {
        if let Ok(cursor) = cursor.get_single() {
            action.send(ActionEvent {
                sender: cam,
                label: "focus",
                target: Some(cursor),
            });
        }
        for ent in selected.iter() {
            action.send(ActionEvent {
                sender: cam,
                label: "focus",
                target: Some(ent),
            });
        }
    }
}

fn grab_or_drop(
    mut commands: Commands,
    mut ev: EventReader<ActionEvent>,
    mut to_grab: Query<(Entity, &mut Transform), (With<Targeted>, Without<Selected>)>,
    mut to_drop: Query<(Entity, &mut Transform), With<Selected>>,
    asset_server: Res<AssetServer>,
    audio: Res<AudioChannel<GrabDropChannel>>,
) {
    for ev in ev.iter() {
        if ev.label == "grab" {
            if let Ok((grab, mut trans)) = to_grab.get_single_mut() {
                debug!("grab");
                commands
                    .entity(grab)
                    .remove::<Targeted>()
                    .insert(Selected)
                    .insert(Controller {
                        enabled: true,
                        left: ("move left", false),
                        right: ("move right", false),
                        up: ("move up", false),
                        down: ("move down", false),
                        action: vec![
                            ("rot_left", ("turn left", true), true),
                            ("rot_right", ("turn right", true), true),
                        ],
                        ..Default::default()
                    });
                trans.translation.z = 0.3;
                audio.set_playback_rate(1.1);
                audio.play(
                    asset_server.load("audio/zapsplat_multimedia_pop_up_tone_short_010_78862.mp3"),
                );
            }

            if let Ok((droppee, mut trans)) = to_drop.get_single_mut() {
                debug!("drop");
                commands
                    .entity(droppee)
                    .remove::<Selected>()
                    .remove::<Controller>();
                trans.translation.z = 0.3;
                audio.set_playback_rate(1.1);
                audio.play(
                    asset_server.load("audio/zapsplat_multimedia_pop_up_tone_short_011_78863.mp3"),
                );
            }
        }
    }
}

fn rotate_plank(
    mut commands: Commands,
    mut ev: EventReader<ActionEvent>,
    cursor: Query<(&Position, &Transform), (With<Cursor>, Without<PlankComponent>)>,
    mut grabbed: Query<
        (
            Entity,
            &mut Transform,
            &mut PlankComponent,
            &mut Position,
            &Children,
        ),
        With<PlankComponent>,
    >,
    mut material_nodes: Query<&mut Transform, (Without<PlankComponent>, Without<Cursor>)>,
) {
    for ev in ev.iter() {
        let dir = match ev.label {
            "rot_left" => 1,
            "rot_right" => 3,
            _ => continue,
        };

        let Ok((cur_pos, cur_trans)) = cursor.get_single() else {
            continue;
        };

        if let Ok((ent, mut transform, mut plank, mut plank_pos, children)) =
            grabbed.get_mut(ev.sender)
        {
            debug!("rot {}", dir);

            for _ in 0..dir {
                plank.0.rotate();

                debug!("extents: {:?}", plank.0.extents());

                let offset = cur_pos.0 - plank_pos.0;
                let rotated = IVec2::new(-offset.y, offset.x);
                debug!(
                    "cur: {}, plank: {}, offset: {}",
                    cur_pos.0, plank_pos.0, offset
                );
                plank_pos.0 = plank_pos.0 + offset - rotated;
                debug!("rotated: {}, new pos: {}", rotated, plank_pos.0);

                if let Some(child) = children.get(0) {
                    if let Ok(mut trans) = material_nodes.get_mut(*child) {
                        let mat_rot = trans.rotation;
                        trans.translation += mat_rot * Vec3::new(1.0, 0.0, 0.0);
                        trans.rotation *= Quat::from_rotation_z(std::f32::consts::PI * 0.5);
                    }
                }
            }
            transform.translation.x =
                plank_pos.0.x as f32 + cur_trans.translation.x - cur_pos.0.x as f32;
            transform.translation.y =
                plank_pos.0.y as f32 + cur_trans.translation.y - cur_pos.0.y as f32;

            let extents = plank.0.extents();
            let extentitem = ExtentItem(
                IVec2::new(extents.0 .0 - 1, extents.1 .0 - 1),
                IVec2::new(extents.0 .1 + 1, extents.1 .1 + 1),
            );
            commands
                .entity(ent)
                .insert(RotateAround(cur_pos.0 - plank_pos.0))
                .insert(extentitem);
        }
    }
}

#[derive(Component, Default)]
struct Cut {
    visited: HashSet<IVec2>,
    separated: HashSet<(IVec2, IVec2)>,
    finished: bool,
}

impl Cut {
    fn split(&self, plank: &Plank) -> Option<[Plank; 2]> {
        if self.separated.is_empty() {
            return None;
        }

        let first = self.separated.iter().next().unwrap().0;
        let mut connected = HashSet::new();
        connected.insert(first);

        let mut to_check = Vec::new();
        to_check.push(first);

        while !to_check.is_empty() {
            let cur = to_check.pop().unwrap();
            for n in [IVec2::X, IVec2::Y, -IVec2::X, -IVec2::Y].iter() {
                let n = *n + cur;
                if plank.contains(n)
                    && !self.separated.contains(&(n.min(cur), n.max(cur)))
                    && !connected.contains(&n)
                {
                    connected.insert(n);
                    to_check.push(n);
                }
            }
        }

        if connected.len() != plank.count() {
            let mut second = HashSet::new();
            for item in plank.coords.iter() {
                if !connected.contains(item) {
                    second.insert(*item);
                }
            }
            return Some([
                Plank {
                    coords: connected,
                    turns: plank.turns,
                    texture_offset: plank.texture_offset,
                },
                Plank {
                    coords: second,
                    turns: plank.turns,
                    texture_offset: plank.texture_offset,
                },
            ]);
        }

        return None;
    }

    fn is_finished(&self, plank: &Plank) -> bool {
        self.split(plank).is_some()
    }
}

fn cut_plank(
    mut commands: Commands,
    mut ev: EventReader<ActionEvent>,
    mut end_cut: EventWriter<CutEvent>,
    cursor_pos: Query<&Position, With<Cursor>>,
    mut cursor: Query<
        (&mut Controller, &Children, Option<&Cursor>),
        Or<(With<Cursor>, With<Selected>)>,
    >,
    mut vis: Query<&mut Visibility>,
    targeted: Query<(Entity, &Position, &PlankComponent), With<Targeted>>,
    cut: Query<(Entity, &Cut)>,
    (mut meshes, mut std_mats): (ResMut<Assets<Mesh>>, ResMut<Assets<StandardMaterial>>),
    mut spawn_plank: EventWriter<SpawnPlank>,
    mut snap: EventWriter<SnapUndo>,
    asset_server: Res<AssetServer>,
    audio: Res<AudioChannel<GrabDropChannel>>,
) {
    for ev in ev.iter() {
        if ev.label == "cancel" {
            if let Ok((cutter, _cut)) = cut.get(ev.sender) {
                // currently cutting - cancel
                debug!("cancel cut");
                for (mut controller, children, is_cursor) in cursor.iter_mut() {
                    controller.enabled = true;
                    if is_cursor.is_some() {
                        for child in children.iter() {
                            if let Ok(mut vis) = vis.get_mut(*child) {
                                vis.is_visible = true;
                            }
                        }
                    }
                }
                commands.entity(cutter).despawn_recursive();
                end_cut.send(CutEvent::CancelCut);
                audio.play(asset_server.load("audio/industrial_tools_hand_saw_hang_on_hook.mp3"));

                continue;
            }
        }

        if ev.label == "cut" {
            if let Ok((_ent, plank_pos, plank)) = targeted.get_single() {
                if let Ok(pos) = cursor_pos.get(ev.sender) {
                    debug!("begin cut");
                    // not cutting - begin

                    // spawn cutter
                    let offsets = [IVec2::ZERO, IVec2::X, IVec2::Y, IVec2::ONE];
                    let valid = offsets.iter().find(|&&offset| {
                        let base = pos.0 + offset - plank_pos.0;
                        let count = offsets
                            .iter()
                            .filter(|&&n| plank.0.contains(base + n - IVec2::ONE))
                            .count();
                        count > 1 && count < 4
                    });

                    let Some(&valid) = valid else {
                        continue;
                    };

                    for (mut controller, children, is_cursor) in cursor.iter_mut() {
                        controller.enabled = false;
                        if is_cursor.is_some() {
                            for child in children.iter() {
                                if let Ok(mut vis) = vis.get_mut(*child) {
                                    vis.is_visible = false;
                                }
                            }
                        }
                    }

                    commands
                        .spawn_bundle((
                            Transform::from_xyz(pos.0.x as f32 + 0.5, pos.0.y as f32 + 0.5, 0.35),
                            GlobalTransform::default(),
                        ))
                        .insert(Position(pos.0 + valid))
                        .insert(PrevPosition(pos.0 + valid))
                        .insert(MoveSpeed(5.0))
                        .insert(ExtentItem(IVec2::ONE, IVec2::ONE))
                        .insert(Cut::default())
                        .insert(Controller {
                            enabled: true,
                            left: ("move left", false),
                            right: ("move right", false),
                            up: ("move up", false),
                            down: ("move down", false),
                            action: vec![
                                ("finish cut", ("main action", true), true),
                                ("cancel", ("second action", true), true),
                            ],
                            ..Default::default()
                        })
                        .with_children(|p| {
                            p.spawn_bundle(PbrBundle {
                                mesh: meshes.add(
                                    UVSphere {
                                        radius: 0.25,
                                        ..Default::default()
                                    }
                                    .into(),
                                ),
                                material: std_mats.add(Color::BLUE.into()),
                                transform: Transform::from_xyz(0.0, 0.0, 0.0),
                                ..Default::default()
                            });
                        });

                    audio.play(
                        asset_server.load("audio/industrial_tools_hand_saw_remove_from_hook.mp3"),
                    );
                }
            }
        }

        if ev.label == "finish cut" {
            if let Ok((cutter, cut)) = cut.get(ev.sender) {
                if let Ok((selected_ent, pos, base_plank)) = targeted.get_single() {
                    if cut.finished {
                        debug!("base");
                        debug_plank_mats(&base_plank.0);

                        // let mut rng = thread_rng();
                        let planks = cut.split(&base_plank.0).unwrap();
                        commands.entity(selected_ent).despawn_recursive();

                        for mut plank in planks.into_iter() {
                            let shift =
                                IVec2::new(-plank.extents().0 .0 + 1, -plank.extents().1 .0 + 1);
                            plank.shift(shift);
                            let pos = pos.0 - shift;

                            debug!(
                                "shift: {}, base offset: {}, new offset: {}",
                                shift, base_plank.0.texture_offset, plank.texture_offset
                            );

                            spawn_plank.send(SpawnPlank(plank, Position(pos), true, true, None));
                        }

                        for (mut controller, children, is_cursor) in cursor.iter_mut() {
                            controller.enabled = true;
                            if is_cursor.is_some() {
                                for child in children.iter() {
                                    if let Ok(mut vis) = vis.get_mut(*child) {
                                        vis.is_visible = true;
                                    }
                                }
                            }
                        }
                        commands.entity(cutter).despawn_recursive();
                        end_cut.send(CutEvent::CancelCut);

                        snap.send_default();
                        snap.send(SnapUndo { is_action: true });

                        audio.play(asset_server.load("audio/zapsplat_industrial_hand_saw_sawing_wood_hollow_fast_pace_short_71000-[AudioTrimmer.com].mp3"));
                    }
                }
            }
        }
    }
}

fn debug_plank_mats(plank: &Plank) {
    debug!("base texture offset: {}", plank.texture_offset);
    debug!("turns: {}", plank.turns);
    for coord in plank.coords.iter() {
        let mut turned = coord.clone();
        for _ in 0..plank.turns {
            turned = IVec2::new(turned.y, -turned.x);
        }
        let offset = turned + plank.texture_offset;

        debug!("coord: {}. turned: {}. offset: {}", coord, turned, offset);
    }
}

#[derive(Default)]
struct SnapUndo {
    is_action: bool,
}

#[derive(Default)]
struct RestoreUndo;

fn record_state(
    mut undo: ResMut<UndoBuffer>,
    done_planks: Res<DonePlanks>,
    planks: Query<(&Position, &PlankComponent)>,
    cursor: Query<&Position, With<Cursor>>,
    camera: Query<(&Position, &PositionZ), With<Camera>>,
    level: Res<Level>,
    mut evs: EventReader<SnapUndo>,
) {
    let Ok(&cursor_pos) = cursor.get_single() else {
        return;
    };
    let Ok((&cam_pos, &cam_pos_z)) = camera.get_single() else {
        return;
    };

    if let Some(ev) = evs.iter().next() {
        let mut level = level.clone();
        level.planks = planks
            .iter()
            .map(|(pos, plank)| (plank.0.clone(), *pos))
            .collect();

        println!("snap {} planks", level.planks.len());

        undo.push_state(
            ev.is_action,
            level,
            done_planks.0.clone(),
            cursor_pos,
            (cam_pos, cam_pos_z),
        );
        println!("pushed state");
        println!("forward: {}, back: {}", undo.has_forward(), undo.has_back());
    }

    // undo.update_cursor_and_camera(cursor_pos, (cam_pos, cam_pos_z));
}

fn change_state(
    mut evs: EventReader<ActionEvent>,
    mut undo: ResMut<UndoBuffer>,
    mut level: ResMut<Level>,
    mut done_planks: ResMut<DonePlanks>,
    mut cursor: Query<(&Transform, &mut Position), (With<Cursor>, Without<Camera>)>,
    mut camera: Query<(&Transform, &mut Position, &mut PositionZ), With<Camera>>,
    mut reset: EventWriter<ResetEvent>,
) {
    let Ok((&cursor_trans, mut cursor_pos)) = cursor.get_single_mut() else { return };
    let Ok((&camera_trans, mut camera_pos, mut camera_pos_z)) = camera.get_single_mut() else { return };

    for ev in evs.iter() {
        match ev.label {
            "undo" => {
                println!(
                    "wants back, forward: {}, back: {}",
                    undo.has_forward(),
                    undo.has_back()
                );

                let current_is_action = undo.current_state().is_action;

                if let Some(state) = undo.prev() {
                    if current_is_action && *cursor_pos != state.cursor {
                        println!("repos");
                        *cursor_pos = state.cursor;
                        *camera_pos = state.camera.0;
                        *camera_pos_z = state.camera.1;
                    } else {
                        println!("act");
                        *level = state.level.clone();
                        done_planks.0 = state.done_planks.clone();
                        reset.send(ResetEvent {
                            cursor_pos: Some(state.cursor),
                            camera_pos: Some(state.camera),
                            cursor_trans: Some(cursor_trans),
                            camera_trans: Some(camera_trans),
                        });
                        undo.move_back();
                    }
                }
            }
            "redo" => {
                println!(
                    "wants forward, forward: {}, back: {}",
                    undo.has_forward(),
                    undo.has_back()
                );

                if let Some(state) = undo.next() {
                    if state.is_action && *cursor_pos != state.cursor {
                        println!("repos");
                        *cursor_pos = state.cursor;
                        *camera_pos = state.camera.0;
                        *camera_pos_z = state.camera.1;
                    } else {
                        println!("{} planks in forward", state.level.planks.len());
                        *level = state.level.clone();
                        done_planks.0 = state.done_planks.clone();
                        reset.send(ResetEvent {
                            cursor_pos: Some(state.cursor),
                            camera_pos: Some(state.camera),
                            cursor_trans: Some(cursor_trans),
                            camera_trans: Some(camera_trans),
                        });
                        undo.move_forward();
                    }
                }
            }
            _ => (),
        }
    }
}

enum CutEvent {
    NewCut { from: IVec2, to: IVec2 },
    UnCut { from: IVec2, to: IVec2 },
    CancelCut,
    FinishCut,
    UnfinishCut,
}

fn extend_cut(
    mut cutter: Query<
        (&mut Cut, &mut Position, &mut PrevPosition),
        (Without<Targeted>, Changed<Position>),
    >,
    selected: Query<(&PlankComponent, &Position), With<Targeted>>,
    mut cuts: EventWriter<CutEvent>,
) {
    for (mut cut, mut position, mut prev) in cutter.iter_mut() {
        if position.0 == prev.0 {
            continue;
        }

        if let Ok((plank, plank_pos)) = selected.get_single() {
            let dir = position.0 - prev.0;
            let affected = match (dir.x, dir.y) {
                (1, 0) => (prev.0 - IVec2::Y, prev.0),
                (-1, 0) => (prev.0 - IVec2::ONE, prev.0 - IVec2::X),
                (0, -1) => (prev.0 - IVec2::ONE, prev.0 - IVec2::Y),
                (0, 1) => (prev.0 - IVec2::X, prev.0),
                _ => {
                    debug!("weird move, abort");
                    position.0 = prev.0;
                    continue;
                }
            };

            let affected = (affected.0 - plank_pos.0, affected.1 - plank_pos.0);

            if !plank.0.contains(affected.0) && !plank.0.contains(affected.1) {
                debug!("air block");
                position.0 = prev.0;
                continue;
            }

            if cut.separated.contains(&affected) {
                debug!("unchop");
                cut.separated.remove(&affected);
                cut.visited.remove(&prev.0);
                cuts.send(CutEvent::UnCut {
                    from: prev.0,
                    to: position.0,
                });
                prev.0 = position.0;
                if cut.finished {
                    cuts.send(CutEvent::UnfinishCut);
                    cut.finished = false;
                }
                continue;
            }

            if cut.finished {
                debug!("finished block");
                position.0 = prev.0;
                continue;
            }

            if plank.0.contains(affected.0) && plank.0.contains(affected.1) {
                debug!("chop");
                cut.visited.insert(prev.0);
                cut.visited.insert(position.0);
                cut.separated.insert(affected);
                cuts.send(CutEvent::NewCut {
                    from: prev.0,
                    to: position.0,
                });

                if cut.is_finished(&plank.0) {
                    debug!("finished!");
                    cut.finished = true;
                    cuts.send(CutEvent::FinishCut);
                }
            }

            prev.0 = position.0;
        }
    }
}

fn draw_cuts(
    mut commands: Commands,
    mut cuts: Local<HashMap<(IVec2, IVec2), Entity>>,
    mut cut_evs: EventReader<CutEvent>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut mats: Local<Option<(Handle<StandardMaterial>, Handle<StandardMaterial>)>>,
    mut reset_events: EventReader<ResetEvent>,
) {
    let (working, done) = match mats.as_ref() {
        Some(data) => data,
        None => {
            let working = materials.add(Color::GRAY.into());
            let done = materials.add(Color::WHITE.into());
            *mats = Some((working, done));
            mats.as_ref().unwrap()
        }
    };

    for _ in reset_events.iter() {
        // make sure teardown doesn't leave us with dangling entities
        cuts.clear();
    }

    for ev in cut_evs.iter() {
        match ev {
            CutEvent::NewCut { from, to } => {
                let id = commands
                    .spawn_bundle(PbrBundle {
                        mesh: meshes.add(
                            BLQuad::new((*from - *to).abs().as_vec2() + 0.2, Vec2::ZERO).into(),
                        ),
                        material: working.clone(),
                        transform: Transform::from_translation(
                            (from.min(*to).as_vec2() - 0.1).extend(0.26),
                        ),
                        ..Default::default()
                    })
                    .id();

                cuts.insert((from.min(*to), from.max(*to)), id);
            }
            CutEvent::UnCut { from, to } => {
                if let Some(existing) = cuts.remove(&(from.min(*to), from.max(*to))) {
                    commands.entity(existing).despawn_recursive();
                }
            }
            CutEvent::CancelCut => {
                for (_, ent) in cuts.drain() {
                    commands.entity(ent).despawn_recursive();
                }
            }
            CutEvent::FinishCut => {
                for ent in cuts.values() {
                    commands.entity(*ent).insert(done.clone());
                }
            }
            CutEvent::UnfinishCut => {
                for ent in cuts.values() {
                    commands.entity(*ent).insert(working.clone());
                }
            }
        }
    }
}

fn target(
    mut commands: Commands,
    cursor: Query<(Entity, &Position), (Without<PlankComponent>, With<Cursor>)>,
    current_target: Query<(Entity, &Position, &PlankComponent), With<Targeted>>,
    mut targets: Query<
        (
            Entity,
            &Position,
            &ExtentItem,
            &PlankComponent,
            &mut Transform,
        ),
        Without<Selected>,
    >,
) {
    let mut found = None;
    if let Ok((_, cursor_pos)) = cursor.get_single() {
        if let Ok((ent, pos, plank)) = current_target.get_single() {
            if plank.0.contains(cursor_pos.0 - pos.0) {
                // keep current selection for stability
                found = Some(ent);
            }
        }

        for (ent, pos, _ext, plank, mut transform) in targets.iter_mut() {
            if plank.0.contains(cursor_pos.0 - pos.0) && (found.is_none() || found == Some(ent)) {
                commands.entity(ent).insert(Targeted);
                transform.translation.z = 0.25;
                found = Some(ent);
                continue;
            }

            transform.translation.z = 0.2;
            commands.entity(ent).remove::<Targeted>();
        }
    }
}

struct SpawnPlank(Plank, Position, bool, bool, Option<IVec2>);

fn spawn_planks(
    mut evs: EventReader<SpawnPlank>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<WoodMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    for ev in evs.iter() {
        let mut plank = ev.0.clone();
        let mut pos = ev.1;

        let fixed_extents = ev.4;

        // fix up
        if fixed_extents.is_none() {
            let shift = IVec2::new(-plank.extents().0 .0 + 1, -plank.extents().1 .0 + 1);
            plank.shift(shift);
            pos = Position(pos.0 - shift);
        }

        let size = match fixed_extents {
            Some(extents) => extents + 2,
            None => plank.size() + 2,
        };

        let quad = BLQuad::new(size.as_vec2(), Vec2::ZERO);

        let colors = match (ev.2, ev.3) {
            (true, true) => (1.5, 1.2, 1.0),
            (true, false) => (1.5, 1.2, 0.0),
            (false, _) => (1.0, 1.0, 0.0),
        };

        let plank_spec = WoodMaterialSpec {
            texture_offset: plank.texture_offset,
            turns: plank.turns,
            primary_color: Color::rgba(0.462, 0.272, 0.136, 1.0) * colors.0,
            secondary_color: Color::rgba(0.284, 0.13, 0.118, 1.0) * colors.1,
            hilight_color: Color::rgba(0.2, 0.2, 1.0, 1.0) * colors.2,
            size: size.as_uvec2(),
            is_plank: ev.2,
            base_color_texture: create_coordset_image(&mut images, &plank),
        };

        debug!("plank offset: {}", plank.texture_offset);

        let mat_handle = mats.add(SimpleTextureMaterial(plank_spec));
        let cloned_mat = mat_handle.clone_weak();
        let mut cmds = commands.spawn();

        let z = match ev.2 {
            true => 0.25,
            false => 0.0,
        };
        cmds.insert(Transform::from_translation(pos.0.as_vec2().extend(z)))
            .insert(GlobalTransform::default())
            .insert(ExtentItem(IVec2::ZERO, size))
            .insert(pos)
            .with_children(|p| {
                p.spawn_bundle(MaterialMeshBundle {
                    mesh: meshes.add(quad.into()),
                    material: mat_handle,
                    ..Default::default()
                });
            });

        if ev.2 {
            // is_plank
            if ev.3 {
                // interactable
                cmds.insert(PlankComponent(plank.clone(), cloned_mat));
            }
        } else {
            cmds.insert(MHoles);
        }

        debug_plank_mats(&plank);
    }
}

struct SpawnNail(IVec2);

fn spawn_nails(
    mut evs: EventReader<SpawnNail>,
    mut commands: Commands,
    mut data: Local<Option<(Handle<Mesh>, Handle<StandardMaterial>)>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    let (mesh, mat) = data.get_or_insert_with(|| {
        (
            meshes.add(
                shape::UVSphere {
                    radius: 0.15,
                    sectors: 10,
                    stacks: 10,
                }
                .into(),
            ),
            mats.add(StandardMaterial {
                base_color: Color::GRAY.into(),
                metallic: 1.0,
                perceptual_roughness: 0.9,
                ..Default::default()
            }),
        )
    });

    for ev in evs.iter() {
        commands.spawn_bundle(PbrBundle {
            mesh: mesh.clone(),
            material: mat.clone(),
            transform: Transform::from_translation(
                ev.0.as_vec2().extend(0.2) + Vec3::new(0.5, 0.5, 0.0),
            ),
            ..Default::default()
        });
    }
}

fn hammer_home(
    mut commands: Commands,
    mut level: ResMut<Level>,
    mut done_planks: ResMut<DonePlanks>,
    holes: Query<&Position, With<MHoles>>,
    target: Query<(Entity, &PlankComponent, &Position), Without<Selected>>,
    mut menu: EventWriter<PopupMenuEvent>,
    levelset: Res<LevelSet>,
    asset_server: Res<AssetServer>,
    audio: Res<AudioChannel<HammerChannel>>,
    mut spawn_nails: EventWriter<SpawnNail>,
    mut snap: EventWriter<SnapUndo>,
) {
    let Ok(hole_pos) = holes.get_single() else {
        return;
    };

    let mut rng = thread_rng();

    for (plank_ent, plank, pos) in target.iter() {
        let mut shifted = plank.0.clone();
        shifted.shift(pos.0 - hole_pos.0);
        for (i, hole) in level.holes.holes.iter().enumerate() {
            if shifted.equals(&hole) {
                debug!("hammer!");
                commands
                    .entity(plank_ent)
                    .remove::<PlankComponent>()
                    .remove::<Targeted>()
                    .remove::<Selected>()
                    .remove::<Controller>();
                level.holes.holes.remove(i);

                let max = 1.max(shifted.count() / 2);
                shifted.shift(hole_pos.0);

                let mut coords = shifted.coords.iter().collect::<Vec<_>>();
                coords.shuffle(&mut rng);

                audio.set_playback_rate(rng.gen_range(1.0..1.5));
                audio.play(asset_server.load("audio/aaj_0404_HamrNail4Hits.mp3"));

                let mut nails = Vec::new();
                for coord in coords.into_iter().take(rng.gen_range(1..=max)) {
                    spawn_nails.send(SpawnNail(*coord));
                    nails.push(*coord);
                }

                done_planks.0.push((plank.0.clone(), *pos, nails));

                if level.holes.holes.is_empty() {
                    debug!("you win!");
                    let mut items = vec![
                        ("Restart Level".into(), "restart"),
                        ("Main Menu".into(), "main menu"),
                        ("Quit to Desktop".into(), "quit"),
                    ];

                    let next = levelset.1 + 1;

                    if next < 30 && levelset.0[next].is_some() {
                        items.insert(
                            0,
                            (format!("Next Level ({}/{})", next + 1, 30), "next level"),
                        );
                    }

                    if levelset.0[0].is_none() {
                        items.insert(0, ("Another Level".into(), "next level"));
                    }

                    menu.send(PopupMenuEvent {
                        sender: Entity::from_raw(0),
                        menu: PopupMenu {
                            heading: format!("Nice one!\n {}/{} completed!", next, 30),
                            items,
                            cancel_action: None,
                        },
                    });
                } else {
                    snap.send_default();
                }

                return;
            }
        }
    }
}

fn system_events(
    mut spawn_event: EventWriter<SpawnLevelEvent>,
    base: Res<LevelBase>,
    mut level: ResMut<Level>,
    mut ev: EventReader<ActionEvent>,
    mut quit: EventWriter<AppExit>,
    mut levelset: ResMut<LevelSet>,
    mut reset_events: EventWriter<ResetEvent>,
) {
    for ev in ev.iter() {
        match ev.label {
            "next level" => {
                levelset.1 += 1;
                if let Some(def) = levelset.0[levelset.1].as_ref() {
                    spawn_event.send(SpawnLevelEvent { def: def.clone() });
                } else {
                    spawn_random(&mut spawn_event, &mut levelset, 30, 0);
                }
            }
            "restart" => {
                *level = base.0.clone();
                level.setup = true;

                reset_events.send_default();
            }
            "quit" => {
                quit.send_default();
            }
            _ => (),
        }
    }
}
