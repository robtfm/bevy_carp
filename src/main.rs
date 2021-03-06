//done
// clean up input, make configurable. check leafwing
// show controls in context
// sfx
// image generation and shader should take offset and orientation
// store state when things change, impl undo (make sure locals are empty & emptied)
// levels
// proper title page
// background
// improve cutter
// more sfx
// credits
// music
// options (ex keys)

//tbd
// options (inc keys)
// quotes (in and out)
// merge pbr and use lighting

#![feature(let_else)]
#![feature(bool_to_option)]

const HOLE_Z: f32 = 0.0;
const PLANK_Z: f32 = 0.5;
const PLANK_Z_SELECTED: f32 = 1.0;
const PLANK_Z_HILIGHTED: f32 = 0.75;
const PLANK_Z_DONE: f32 = 0.25;

use bevy_pkv::PkvStore;
use input::{Action, ActionType, Controller, DisplayDirections, DisplayMode, InputPlugin};
use menus::{
    spawn_controls, spawn_in_level_menu, spawn_main_menu, spawn_play_menu, spawn_popup_menu,
    PopupMenuEvent,
};
use rand::{prelude::SliceRandom, thread_rng, Rng, SeedableRng};

use bevy::{
    app::AppExit,
    ecs::event::{Events, ManualEventReader},
    log::LogSettings,
    math::Vec3Swizzles,
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

mod background;
mod bl_quad;
mod input;
mod menus;
mod model;
mod shader;
mod structs;
mod window;
mod wood_material;

use bl_quad::BLQuad;
use model::*;
use rand_pcg::Pcg32;
use shader::SimpleTextureMaterial;
use structs::{
    ActionEvent, ActionLabel, ChangeBackground, ControlHelp, GrabDropChannel, HammerChannel,
    LevelDef, LevelSet, MenuChannel, MusicChannel, Permanent, PositionZ, SpawnLevelEvent,
    UndoChannel,
};
use window::{descriptor_from_settings, WindowModeSerial};
use wood_material::{WoodMaterial, WoodMaterialPlugin, WoodMaterialSpec};

use crate::{
    background::BackgroundPlugin,
    menus::{spawn_credits, spawn_options_menu, PopupMenu},
    structs::{CutChannel, Position, SwooshChannel, QUIT_TO_DESKTOP},
};

fn main() {
    // stage conventions to avoid adding components to despawned entities:
    // pre - ensure working state
    // update - add / spawn, don't despawn (unless it's locally used entities)
    // post - spawn / despawn, don't add

    let settings = PkvStore::new("robtfm", "measure once");

    let window_descriptor = descriptor_from_settings(&settings);
    let control_help = ControlHelp(settings.get("control help").unwrap_or(true));
    let cursor_speed = CursorSpeed(settings.get("cursor speed").unwrap_or(15.0));
    let cut_speed = CutSpeed(settings.get("cut speed").unwrap_or(5.0));
    let music_volume = MusicVolume(settings.get("music volume").unwrap_or(0.5));
    let sfx_volume = SfxVolume(settings.get("sfx volume").unwrap_or(0.5));

    let mut app = App::new();
    app.insert_resource(window_descriptor)
        .insert_resource(LogSettings {
            level: bevy::log::Level::INFO,
            filter: "wgpu=error,symphonia=error".to_string(),
        })
        .add_plugins(DefaultPlugins)
        .add_plugin(WoodMaterialPlugin)
        .add_plugin(EguiPlugin)
        .add_plugin(AudioPlugin)
        .add_plugin(InputPlugin)
        .add_plugin(BackgroundPlugin)
        .add_audio_channel::<MusicChannel>()
        .add_audio_channel::<MenuChannel>()
        .add_audio_channel::<GrabDropChannel>()
        .add_audio_channel::<SwooshChannel>()
        .add_audio_channel::<HammerChannel>()
        .add_audio_channel::<CutChannel>()
        .add_audio_channel::<UndoChannel>()
        .init_resource::<Level>()
        .init_resource::<DonePlanks>()
        .init_resource::<LevelDef>()
        .init_resource::<LevelBase>()
        .init_resource::<LevelSet>()
        .init_resource::<UndoBuffer>()
        .insert_resource(settings)
        .insert_resource(AmbientLight {
            color: Color::rgba(0.8, 0.8, 1.0, 1.0),
            brightness: 0.1,
        })
        .insert_resource(ClearColor(Color::rgb(0.05, 0.05, 0.3)))
        .insert_resource(control_help)
        .insert_resource(cursor_speed)
        .insert_resource(cut_speed)
        .insert_resource(music_volume)
        .insert_resource(sfx_volume)
        .add_event::<SpawnLevelEvent>()
        .add_event::<PopupMenuEvent>()
        .add_event::<CutEvent>()
        .add_event::<ResetEvent>()
        .add_event::<SnapUndo>()
        .add_event::<SpawnNail>()
        .add_event::<SpawnPlank>()
        // egui
        .add_startup_system(warm_assets)
        .add_startup_system(egui_setup)
        .add_system(handle_window_resize)
        // menus
        .add_startup_system(splash)
        .add_system_to_stage(CoreStage::PostUpdate, spawn_main_menu.after(camera_focus)) // despawns, must run after cam focus so the entities are spawned to focus on
        .add_system(spawn_play_menu)
        .add_system(spawn_in_level_menu)
        .add_system(spawn_credits)
        .add_system(spawn_popup_menu)
        .add_system(spawn_options_menu)
        .add_system(spawn_controls)
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
        .add_system(update_positions.before(update_transforms))
        .add_system(update_transforms)
        .add_system(check_cut_actions)
        // fx
        .add_system(update_materials)
        .add_system(spawn_planks)
        .add_system(spawn_nails)
        .add_system(animate_cuts)
        .add_system(animate_sparks)
        .add_system(update_volumes)
        .add_system(update_speed_settings)
        // system events
        .add_system_to_stage(CoreStage::PostUpdate, system_events)
        // undo/redo
        // records planks before and after, after requires commands completed, so cut_plank -> record gets prior state -> spawn_planks -> *cmds exec* -> record gets new state
        .add_system_to_stage(CoreStage::PostUpdate, record_state.after(cut_plank))
        .add_system_to_stage(CoreStage::PostUpdate, change_state)
        // camera management
        .add_system_to_stage(CoreStage::PostUpdate, camera_focus)
        // .add_system(debug_actions)
        .run();
}

#[derive(Default)]
struct ResetEvent {
    cursor_pos: Option<Position>,
    cursor_trans: Option<Transform>,
    camera_pos: Option<(Position, PositionZ)>,
    camera_trans: Option<Transform>,
}

fn splash(
    mut evs: EventWriter<ActionEvent>,
    audio: Res<AudioChannel<MusicChannel>>,
    server: Res<AssetServer>,
) {
    evs.send(ActionEvent {
        sender: Entity::from_raw(99),
        label: ActionLabel("main menu"),
        target: None,
    });

    audio.play_looped(server.load("audio/Banjos,+Unite!+-+320bit.mp3"));
}

pub struct MusicVolume(pub f32);
pub struct SfxVolume(pub f32);

fn update_volumes(
    music_channel: Res<AudioChannel<MusicChannel>>,
    menu_channel: Res<AudioChannel<MenuChannel>>,
    grab_channel: Res<AudioChannel<GrabDropChannel>>,
    swoosh_channel: Res<AudioChannel<SwooshChannel>>,
    hammer_channel: Res<AudioChannel<HammerChannel>>,
    cut_channel: Res<AudioChannel<CutChannel>>,
    undo_channel: Res<AudioChannel<UndoChannel>>,
    music: Res<MusicVolume>,
    sfx: Res<SfxVolume>,
    mut last_music: Local<Option<f32>>,
    mut last_sfx: Local<Option<f32>>,
    mut settings: ResMut<PkvStore>,
    asset_server: Res<AssetServer>,
) {
    if Some(music.0) != *last_music {
        music_channel.set_volume(music.0);
        *last_music = Some(music.0);
        settings.set("music volume", &music.0).unwrap();
    }
    if Some(sfx.0) != *last_sfx {
        menu_channel.set_volume(sfx.0);
        grab_channel.set_volume(sfx.0);
        swoosh_channel.set_volume(sfx.0);
        hammer_channel.set_volume(sfx.0);
        cut_channel.set_volume(sfx.0 * 0.7);
        undo_channel.set_volume(sfx.0);

        if last_sfx.is_some() {
            cut_channel.stop();
            cut_channel.play(asset_server.load("audio/zapsplat_industrial_hand_saw_sawing_wood_hollow_fast_pace_short_71000-[AudioTrimmer.com].mp3"));
            settings.set("sfx volume", &sfx.0).unwrap();
        }

        *last_sfx = Some(sfx.0);
    }
}

fn update_speed_settings(
    cursor: Res<CursorSpeed>,
    cutter: Res<CutSpeed>,
    mut last_cursor: Local<f32>,
    mut last_cutter: Local<f32>,
    mut settings: ResMut<PkvStore>,
) {
    if cursor.0 != *last_cursor {
        settings.set("cursor speed", &cursor.0).unwrap();
        *last_cursor = cursor.0;
    }
    if cutter.0 != *last_cutter {
        settings.set("cutter speed", &cutter.0).unwrap();
        *last_cutter = cutter.0;
    }
}

fn warm_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut handles: Local<Vec<HandleUntyped>>,
) {
    // spawn an entity so that 0v0 is taken
    let id = commands.spawn().id();
    commands.entity(id).despawn();

    *handles = vec![
        asset_server.load::<AudioSource, _>("audio/aaj_0404_HamrNail4Hits.mp3").clone_untyped(),
        asset_server.load::<AudioSource, _>("audio/industrial_tools_hand_saw_hang_on_hook.mp3").clone_untyped(),
        asset_server.load::<AudioSource, _>("audio/industrial_tools_hand_saw_remove_from_hook.mp3").clone_untyped(),
        asset_server.load::<AudioSource, _>("audio/zapsplat_foley_wood_bambo_swoosh_through_air_001-[AudioTrimmer.com](1).mp3").clone_untyped(),
        asset_server.load::<AudioSource, _>("audio/zapsplat_industrial_hand_saw_sawing_wood_hollow_fast_pace_short_71000-[AudioTrimmer.com].mp3").clone_untyped(),
        asset_server.load::<AudioSource, _>("audio/zapsplat_multimedia_alert_mallet_hit_short_single_generic_003_79278.mp3").clone_untyped(),
        asset_server.load::<AudioSource, _>("audio/zapsplat_multimedia_game_sound_game_show_correct_tone_bright_positive_006_80747.mp3").clone_untyped(),
        asset_server.load::<AudioSource, _>("audio/zapsplat_multimedia_pop_up_tone_short_010_78862.mp3").clone_untyped(),
        asset_server.load::<AudioSource, _>("audio/zapsplat_multimedia_pop_up_tone_short_011_78863.mp3").clone_untyped(),
        asset_server.load::<AudioSource, _>("audio/zapsplat_office_compass_pencil_draw_circle_on_paper_003_22758-[AudioTrimmer.com].mp3").clone_untyped(),
        asset_server.load::<AudioSource, _>("audio/zapsplat_sport_surfboard_leash_velcro_strap_undo_003.mp3").clone_untyped(),
        asset_server.load::<AudioSource, _>("audio/zapsplat_sport_surfboard_leash_velcro_strap_undo_004.mp3").clone_untyped(),
        asset_server.load::<Image, _>("images/lumber edit1.png").clone_untyped(),
        asset_server.load::<Image, _>("images/lumber edit2.png").clone_untyped(),
        asset_server.load::<Image, _>("images/lumber edit3.png").clone_untyped(),
        asset_server.load::<Image, _>("images/title.png").clone_untyped(),
    ];
}

fn egui_setup(mut egui_ctx: ResMut<EguiContext>) {
    let widget_visuals = egui::style::WidgetVisuals {
        bg_fill: egui::Rgba::from_rgba_premultiplied(0.0, 0.0, 0.0, 0.8).into(),
        bg_stroke: egui::Stroke::new(1.0, egui::Rgba::from_rgba_premultiplied(1.0, 1.0, 1.0, 1.0)),
        rounding: egui::Rounding::same(5.0),
        fg_stroke: egui::Stroke::new(1.0, egui::Rgba::from_rgba_premultiplied(0.8, 0.8, 1.0, 1.0)),
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
    mut events_1: EventReader<WindowResized>,
    mut events_2: EventReader<WindowMoved>,
    windows: Res<Windows>,
    mut egui_settings: ResMut<EguiSettings>,
    mut settings: ResMut<PkvStore>,
) {
    if events_1.is_empty() && events_2.is_empty() {
        return;
    }

    if let Some(window) = windows.get_primary() {
        let scale = f64::max(
            0.1,
            f32::min(window.height() / 720.0, window.width() / 1280.0) as f64,
        );
        egui_settings.scale_factor = scale;

        if settings.get::<WindowModeSerial>("window mode").unwrap() != WindowModeSerial::Fullscreen
        {
            settings
                .set("window size", &(window.width(), window.height()))
                .unwrap();
            if let Some(pos) = window.position() {
                settings.set("window pos", &pos).unwrap();
            }
            debug!("store: {}/{}", window.width(), window.height());
        }
    }

    events_1.iter().count();
    events_2.iter().count();
}

fn spawn_random(
    total: usize,
    skip: usize,
    title: String,
    seed: u64,
    key: &'static str,
) -> LevelSet {
    let mut rng = Pcg32::seed_from_u64(seed);

    let mut defs = (0..total)
        .map(|_| {
            let numbers = (
                rng.gen::<u64>(),
                rng.gen_range::<u64, _>(2..15),
                rng.gen_range::<u64, _>(0..7),
                rng.gen_range::<u64, _>(3..9),
            );

            let seed = numbers.0;
            let num_holes = numbers.1;
            let total_blocks = numbers.2 + num_holes * numbers.3;
            LevelDef {
                num_holes: num_holes as usize,
                total_blocks: total_blocks as usize,
                seed,
            }
        })
        .collect::<Vec<_>>();

    defs.sort_by_key(|def| {
        let mut rng = Pcg32::seed_from_u64(def.seed);
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

    let defs: [LevelDef; 30] = match defs.try_into() {
        Ok(defs) => defs,
        Err(_) => panic!(),
    };

    LevelSet {
        title,
        levels: defs,
        current_level: 0,
        settings_key: key,
    }
}

fn setup_level(
    mut spawn_evs: EventReader<SpawnLevelEvent>,
    mut base: ResMut<LevelBase>,
    mut level: ResMut<Level>,
    mut def: ResMut<LevelDef>,
    mut reset: EventWriter<ResetEvent>,
    mut commands: Commands,
    mut bg: EventWriter<ChangeBackground>,
) {
    for ev in spawn_evs.iter() {
        let mut rng = Pcg32::seed_from_u64(ev.def.seed);
        let mut holes = gen_holes(ev.def.num_holes, ev.def.total_blocks, &mut rng);
        holes
            .holes
            .sort_by(|a, b| a.size().y.cmp(&b.size().y).reverse());
        let mut plank = Plank::from_holes(&holes, &mut rng);
        if plank.size().x < plank.size().y {
            plank.rotate();
            plank = plank.normalize();
        }
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
            extents = extents.max(IVec2::new(hole_extents.0 .1, hole_extents.1 .1));
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

        let size = plank.size() + 1;
        let pos = IVec2::new(-size.x / 2, -size.y - 1);

        *base = LevelBase(Level {
            extents,
            holes,
            planks: vec![(plank, Position(pos))],
            setup: true,
        });
        *level = base.0.clone();
        *def = ev.def.clone();
        commands.insert_resource(UndoBuffer::new(base.0.clone()));
        commands.insert_resource(DonePlanks::default());

        bg.send_default();
        reset.send_default();
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

pub struct CursorSpeed(pub f32);
pub struct CutSpeed(pub f32);

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
    cursor_speed: Res<CursorSpeed>,
) {
    for ev in evs.iter() {
        for ent in old.iter() {
            commands.entity(ent).despawn_recursive();
        }

        commands
            .spawn()
            .insert(Controller {
                display_order: 0,
                actions: vec![
                    (
                        ActionType::Menu,
                        Action {
                            label: ActionLabel("pause"),
                            sticky: true,
                            display: DisplayMode::Active,
                            display_text: None,
                        },
                    ),
                    (
                        ActionType::ThirdAction,
                        Action {
                            label: ActionLabel("undo"),
                            sticky: true,
                            display: DisplayMode::Active,
                            display_text: None,
                        },
                    ),
                    (
                        ActionType::FourthAction,
                        Action {
                            label: ActionLabel("redo"),
                            sticky: true,
                            display: DisplayMode::Active,
                            display_text: None,
                        },
                    ),
                ],
                enabled: true,
                ..Default::default()
            })
            .insert(SystemController);

        let size = level.extents;
        let pos = IVec2::new(-size.x / 2, 1);
        let merger = CoordSet::merge(level.holes.holes.iter());
        spawn_planks.send(SpawnPlank {
            plank: merger,
            position: Position(pos),
            is_plank: false,
            is_interactive: true,
            manual_extents: Some(level.extents),
        });

        for plank in level.planks.iter() {
            spawn_planks.send(SpawnPlank {
                plank: plank.0.clone(),
                position: plank.1,
                is_plank: true,
                is_interactive: true,
                manual_extents: None,
            });
        }

        for (plank, pos, nails) in done_planks.0.iter() {
            spawn_planks.send(SpawnPlank {
                plank: plank.clone(),
                position: *pos,
                is_plank: true,
                is_interactive: false,
                manual_extents: None,
            });
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
            .insert(PositionOffset(Vec2::new(0.01, 0.01)))
            .insert(Controller {
                display_order: 1,
                display_directions: Some(DisplayDirections {
                    label: "pan".into(),
                    up: ActionType::PanUp,
                    down: ActionType::PanDown,
                    left: ActionType::PanLeft,
                    right: ActionType::PanRight,
                }),
                enabled: true,
                actions: vec![
                    (
                        ActionType::PanUp,
                        Action {
                            label: ActionLabel("up"),
                            sticky: false,
                            display: DisplayMode::Off,
                            display_text: None,
                        },
                    ),
                    (
                        ActionType::PanDown,
                        Action {
                            label: ActionLabel("down"),
                            sticky: false,
                            display: DisplayMode::Off,
                            display_text: None,
                        },
                    ),
                    (
                        ActionType::PanLeft,
                        Action {
                            label: ActionLabel("left"),
                            sticky: false,
                            display: DisplayMode::Off,
                            display_text: None,
                        },
                    ),
                    (
                        ActionType::PanRight,
                        Action {
                            label: ActionLabel("right"),
                            sticky: false,
                            display: DisplayMode::Off,
                            display_text: None,
                        },
                    ),
                    (
                        ActionType::ZoomIn,
                        Action {
                            label: ActionLabel("forward"),
                            sticky: false,
                            display: DisplayMode::Active,
                            display_text: None,
                        },
                    ),
                    (
                        ActionType::ZoomOut,
                        Action {
                            label: ActionLabel("backward"),
                            sticky: false,
                            display: DisplayMode::Active,
                            display_text: None,
                        },
                    ),
                    (
                        ActionType::PanFocus,
                        Action {
                            label: ActionLabel("focus"),
                            sticky: true,
                            display: DisplayMode::Active,
                            display_text: None,
                        },
                    ),
                ],
                ..Default::default()
            })
            .id();

        commands
            .spawn_bundle((
                ev.cursor_trans
                    .unwrap_or(Transform::from_xyz(0.0, 0.0, PLANK_Z_SELECTED + 0.5)),
                GlobalTransform::default(),
            ))
            .insert(ev.cursor_pos.unwrap_or_default())
            .insert(ExtentItem(IVec2::ONE, IVec2::ONE))
            .insert(Cursor)
            .insert(MoveSpeed(cursor_speed.0))
            .insert(Controller {
                display_order: 2,
                display_directions: Some(DisplayDirections {
                    label: "move".into(),
                    up: ActionType::MoveUp,
                    down: ActionType::MoveDown,
                    left: ActionType::MoveLeft,
                    right: ActionType::MoveRight,
                }),
                enabled: true,
                actions: vec![
                    (
                        ActionType::MoveLeft,
                        Action {
                            label: ActionLabel("left"),
                            sticky: false,
                            display: DisplayMode::Off,
                            display_text: None,
                        },
                    ),
                    (
                        ActionType::MoveRight,
                        Action {
                            label: ActionLabel("right"),
                            sticky: false,
                            display: DisplayMode::Off,
                            display_text: None,
                        },
                    ),
                    (
                        ActionType::MoveUp,
                        Action {
                            label: ActionLabel("up"),
                            sticky: false,
                            display: DisplayMode::Off,
                            display_text: None,
                        },
                    ),
                    (
                        ActionType::MoveDown,
                        Action {
                            label: ActionLabel("down"),
                            sticky: false,
                            display: DisplayMode::Off,
                            display_text: None,
                        },
                    ),
                    (
                        ActionType::MainAction,
                        Action {
                            label: ActionLabel("grab"),
                            sticky: true,
                            display: DisplayMode::Active,
                            display_text: None,
                        },
                    ),
                    (
                        ActionType::SecondAction,
                        Action {
                            label: ActionLabel("cut"),
                            sticky: true,
                            display: DisplayMode::Active,
                            display_text: None,
                        },
                    ),
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
                    transform: Transform::from_xyz(0.5, 0.5, 8.0),
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
            label: ActionLabel("focus"),
            target: None,
        });
    }
}

#[derive(Component)]
struct ExtentItem(IVec2, IVec2);

#[derive(Component)]
struct MoveSpeed(f32);

#[derive(Component)]
struct PositionOffset(Vec2);

#[derive(Component, Default)]
struct PrevPosition(pub IVec2);

fn update_positions(
    mut mobiles: Query<(
        Option<&Transform>,
        Option<&mut Position>,
        Option<&mut PositionZ>,
        Option<&PositionOffset>,
    )>,
    mut events: EventReader<ActionEvent>,
) {
    for ev in events.iter() {
        if let Ok((maybe_transform, maybe_position, maybe_position_z, maybe_offset)) =
            mobiles.get_mut(ev.sender)
        {
            let mut translation = match (
                maybe_transform,
                maybe_position.as_ref(),
                maybe_position_z.as_ref(),
            ) {
                (Some(transform), ..) => transform.translation,
                (None, Some(position), Some(pos_z)) => position.0.extend(pos_z.0).as_vec3(),
                (None, Some(position), None) => position.0.extend(0).as_vec3(),
                _ => continue,
            };

            if let Some(offset) = maybe_offset {
                translation -= offset.0.extend(0.0);
            }

            match ev.label.0 {
                "forward" => {
                    if let Some(mut position_z) = maybe_position_z {
                        position_z.0 = (translation.z - 1.0).ceil() as i32;
                    }
                }
                "backward" => {
                    if let Some(mut position_z) = maybe_position_z {
                        position_z.0 = (translation.z + 1.0).floor() as i32;
                    }
                }
                "left" => {
                    if let Some(mut position) = maybe_position {
                        position.0.x = (translation.x - 1.0).ceil() as i32;
                    }
                }
                "right" => {
                    if let Some(mut position) = maybe_position {
                        position.0.x = (translation.x + 1.0).floor() as i32;
                    }
                }
                "down" => {
                    if let Some(mut position) = maybe_position {
                        position.0.y = (translation.y - 1.0).ceil() as i32;
                    }
                }
                "up" => {
                    if let Some(mut position) = maybe_position {
                        position.0.y = (translation.y + 1.0).floor() as i32;
                    }
                }
                _ => continue,
            }
        }
    }
}

fn update_transforms(
    time: Res<Time>,
    mut q: Query<(
        &mut Transform,
        &Position,
        Option<&PositionZ>,
        Option<&PositionOffset>,
        Option<&MoveSpeed>,
    )>,
) {
    for (mut transform, position, maybe_posz, maybe_offset, maybe_speed) in q.iter_mut() {
        let position = position.0.as_vec2() + maybe_offset.map(|o| o.0).unwrap_or_default();

        let speed = maybe_speed.unwrap_or(&MoveSpeed(15.0)).0;
        if transform.translation.x < position.x as f32 {
            transform.translation.x = f32::min(
                position.x as f32,
                transform.translation.x
                    + time.delta_seconds()
                        * f32::max(1.0, position.x - transform.translation.x)
                        * speed,
            );
        } else {
            transform.translation.x = f32::max(
                position.x as f32,
                transform.translation.x
                    - time.delta_seconds()
                        * f32::max(1.0, transform.translation.x - position.x)
                        * speed,
            );
        }
        if transform.translation.y < position.y as f32 {
            transform.translation.y = f32::min(
                position.y as f32,
                transform.translation.y
                    + time.delta_seconds()
                        * f32::max(1.0, position.y - transform.translation.y)
                        * speed,
            );
        } else {
            transform.translation.y = f32::max(
                position.y as f32,
                transform.translation.y
                    - time.delta_seconds()
                        * f32::max(1.0, transform.translation.y - position.y)
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
struct SystemController;

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
    mut cam: Query<
        (&mut Position, &mut PositionZ, &PerspectiveProjection),
        (Without<ExtentItem>, Without<Cursor>),
    >,
    all: Query<(Entity, &Position, &ExtentItem)>,
) {
    for ev in evs.iter() {
        if ev.label.0 == "focus" {
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
                label: ActionLabel("focus"),
                target: Some(cursor),
            });
        }
        for ent in selected.iter() {
            action.send(ActionEvent {
                sender: cam,
                label: ActionLabel("focus"),
                target: Some(ent),
            });
        }
    }
}

fn grab_or_drop(
    mut commands: Commands,
    mut ev: EventReader<ActionEvent>,
    mut to_grab: Query<
        (Entity, &mut Transform),
        (With<Targeted>, Without<Selected>, Without<Cursor>),
    >,
    mut to_drop: Query<(Entity, &mut Transform), (With<Selected>, Without<Cursor>)>,
    asset_server: Res<AssetServer>,
    audio: Res<AudioChannel<GrabDropChannel>>,
    cursor_speed: Res<CursorSpeed>,
    cursor: Query<(&Transform, &Position), With<Cursor>>,
) {
    for ev in ev.iter() {
        let Ok((cursor_trans, cursor_pos)) = cursor.get_single() else {
            continue;
        };

        if ["grab", "swap"]
            .into_iter()
            .find(|l| l == &ev.label.0)
            .is_some()
        {
            if let Ok((grab, mut trans)) = to_grab.get_single_mut() {
                debug!("grab");
                commands
                    .entity(grab)
                    .remove::<Targeted>()
                    .insert(Selected)
                    .insert(MoveSpeed(cursor_speed.0))
                    .insert(Controller {
                        display_order: 4,
                        enabled: true,
                        actions: vec![
                            (
                                ActionType::MoveLeft,
                                Action {
                                    label: ActionLabel("left"),
                                    sticky: false,
                                    display: DisplayMode::Off,
                                    display_text: None,
                                },
                            ),
                            (
                                ActionType::MoveRight,
                                Action {
                                    label: ActionLabel("right"),
                                    sticky: false,
                                    display: DisplayMode::Off,
                                    display_text: None,
                                },
                            ),
                            (
                                ActionType::MoveUp,
                                Action {
                                    label: ActionLabel("up"),
                                    sticky: false,
                                    display: DisplayMode::Off,
                                    display_text: None,
                                },
                            ),
                            (
                                ActionType::MoveDown,
                                Action {
                                    label: ActionLabel("down"),
                                    sticky: false,
                                    display: DisplayMode::Off,
                                    display_text: None,
                                },
                            ),
                            (
                                ActionType::TurnLeft,
                                Action {
                                    label: ActionLabel("rotate left"),
                                    sticky: true,
                                    display: DisplayMode::Active,
                                    display_text: None,
                                },
                            ),
                            (
                                ActionType::TurnRight,
                                Action {
                                    label: ActionLabel("rotate right"),
                                    sticky: true,
                                    display: DisplayMode::Active,
                                    display_text: None,
                                },
                            ),
                        ],
                        ..Default::default()
                    });
                // add cursor's position offset
                trans.translation +=
                    (cursor_trans.translation.xy() - cursor_pos.0.as_vec2()).extend(0.0);
                trans.translation.z = PLANK_Z_SELECTED;
                audio.set_playback_rate(1.1);
                audio.play(
                    asset_server.load("audio/zapsplat_multimedia_pop_up_tone_short_010_78862.mp3"),
                );
            }
        }

        if ["drop", "swap"]
            .into_iter()
            .find(|l| l == &ev.label.0)
            .is_some()
        {
            if let Ok((droppee, mut trans)) = to_drop.get_single_mut() {
                debug!("drop");
                commands
                    .entity(droppee)
                    .remove::<Selected>()
                    .remove::<Controller>();
                trans.translation.z = PLANK_Z;
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
    asset_server: Res<AssetServer>,
    audio: Res<AudioChannel<SwooshChannel>>,
) {
    for ev in ev.iter() {
        let dir = match ev.label.0 {
            "rotate left" => 1,
            "rotate right" => 3,
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

            audio.play(asset_server.load(
                "audio/zapsplat_foley_wood_bambo_swoosh_through_air_001-[AudioTrimmer.com](1).mp3",
            ));
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
    selected: Query<&Selected>,
    cut: Query<(Entity, &Cut, &Position)>,
    (mut meshes, mut std_mats): (ResMut<Assets<Mesh>>, ResMut<Assets<StandardMaterial>>),
    mut spawn_plank: EventWriter<SpawnPlank>,
    mut snap: EventWriter<SnapUndo>,
    asset_server: Res<AssetServer>,
    audio: Res<AudioChannel<GrabDropChannel>>,
    mut last_cutter_pos: Local<IVec2>,
    cut_speed: Res<CutSpeed>,
) {
    for ev in ev.iter() {
        if ev.label.0 == "cancel" {
            if let Ok((cutter, _cut, cutter_pos)) = cut.get(ev.sender) {
                // currently cutting - cancel
                debug!("cancel cut");
                *last_cutter_pos = cutter_pos.0;
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

        if ev.label.0 == "cut" && selected.is_empty() {
            if let Ok((_ent, plank_pos, plank)) = targeted.get_single() {
                if let Ok(pos) = cursor_pos.get(ev.sender) {
                    debug!("begin cut");
                    // not cutting - begin

                    // spawn cutter
                    let mut positions = Vec::new();

                    // prefer last pos
                    let last_offset = *last_cutter_pos - pos.0;
                    if last_offset.max_element() <= 1 && last_offset.min_element() >= 0 {
                        positions.push(*last_cutter_pos);
                    }
                    // then any nearby
                    let offsets = [IVec2::ZERO, IVec2::X, IVec2::Y, IVec2::ONE];
                    for offset in offsets {
                        positions.push(pos.0 + offset);
                    }

                    let valid = positions.iter().find(|&&pos| {
                        let count = offsets
                            .iter()
                            .filter(|&&n| plank.0.contains(pos + n - plank_pos.0 - IVec2::ONE))
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
                            Transform::from_xyz(
                                pos.0.x as f32 + 0.5,
                                pos.0.y as f32 + 0.5,
                                PLANK_Z_HILIGHTED + 0.25,
                            ),
                            GlobalTransform::default(),
                        ))
                        .insert(Position(valid))
                        .insert(PrevPosition(valid))
                        .insert(MoveSpeed(cut_speed.0))
                        .insert(ExtentItem(IVec2::ONE, IVec2::ONE))
                        .insert(Cut::default())
                        .insert(Controller {
                            display_order: 3,
                            display_directions: Some(DisplayDirections {
                                label: "Cut".into(),
                                up: ActionType::MoveUp,
                                down: ActionType::MoveDown,
                                left: ActionType::MoveLeft,
                                right: ActionType::MoveRight,
                            }),
                            enabled: true,
                            actions: vec![
                                (
                                    ActionType::MoveLeft,
                                    Action {
                                        label: ActionLabel("left"),
                                        sticky: false,
                                        display: DisplayMode::Off,
                                        display_text: None,
                                    },
                                ),
                                (
                                    ActionType::MoveRight,
                                    Action {
                                        label: ActionLabel("right"),
                                        sticky: false,
                                        display: DisplayMode::Off,
                                        display_text: None,
                                    },
                                ),
                                (
                                    ActionType::MoveUp,
                                    Action {
                                        label: ActionLabel("up"),
                                        sticky: false,
                                        display: DisplayMode::Off,
                                        display_text: None,
                                    },
                                ),
                                (
                                    ActionType::MoveDown,
                                    Action {
                                        label: ActionLabel("down"),
                                        sticky: false,
                                        display: DisplayMode::Off,
                                        display_text: None,
                                    },
                                ),
                                (
                                    ActionType::MainAction,
                                    Action {
                                        label: ActionLabel("finish cut"),
                                        sticky: true,
                                        display: DisplayMode::Active,
                                        display_text: None,
                                    },
                                ),
                                (
                                    ActionType::SecondAction,
                                    Action {
                                        label: ActionLabel("cancel"),
                                        sticky: true,
                                        display: DisplayMode::Active,
                                        display_text: None,
                                    },
                                ),
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

        if ev.label.0 == "finish cut" {
            if let Ok((cutter, cut, _cutter_pos)) = cut.get(ev.sender) {
                if let Ok((selected_ent, pos, base_plank)) = targeted.get_single() {
                    if cut.finished {
                        debug!("base");
                        debug_plank_mats(&base_plank.0);

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

                            spawn_plank.send(SpawnPlank {
                                plank,
                                position: Position(pos),
                                is_plank: true,
                                is_interactive: true,
                                manual_extents: None,
                            });
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

        debug!("snap {} planks", level.planks.len());

        undo.push_state(
            ev.is_action,
            level,
            done_planks.0.clone(),
            cursor_pos,
            (cam_pos, cam_pos_z),
        );
        debug!("pushed state");
        debug!("forward: {}, back: {}", undo.has_forward(), undo.has_back());
    }

    // undo.update_cursor_and_camera(cursor_pos, (cam_pos, cam_pos_z));
}

fn change_state(
    mut actions: ResMut<Events<ActionEvent>>,
    mut reader: Local<ManualEventReader<ActionEvent>>,
    mut undo: ResMut<UndoBuffer>,
    mut level: ResMut<Level>,
    mut done_planks: ResMut<DonePlanks>,
    mut cursor: Query<(&Transform, &mut Position), (With<Cursor>, Without<Camera>)>,
    mut camera: Query<(&Transform, &mut Position, &mut PositionZ), With<Camera>>,
    mut reset: EventWriter<ResetEvent>,
    to_drop: Query<Entity, With<Selected>>,
    cutter: Query<&Cut>,
    asset_server: Res<AssetServer>,
    audio: Res<AudioChannel<UndoChannel>>,
) {
    let Ok((&cursor_trans, mut cursor_pos)) = cursor.get_single_mut() else { return };
    let Ok((&camera_trans, mut camera_pos, mut camera_pos_z)) = camera.get_single_mut() else { return };

    let mut action_to_send = None;

    for ev in reader.iter(&actions) {
        match ev.label.0 {
            "undo" => {
                debug!(
                    "wants back, forward: {}, back: {}",
                    undo.has_forward(),
                    undo.has_back()
                );

                let current_is_action = undo.current_state().is_action;

                if to_drop.get_single().is_ok() {
                    action_to_send = Some("drop");
                    break;
                }

                if cutter.get_single().is_ok() {
                    action_to_send = Some("cancel");
                    break;
                }

                if let Some(state) = undo.prev() {
                    audio
                        .play(asset_server.load(
                            "audio/zapsplat_sport_surfboard_leash_velcro_strap_undo_003.mp3",
                        ));

                    if current_is_action && *cursor_pos != state.cursor {
                        debug!("repos");
                        *cursor_pos = state.cursor;
                        *camera_pos = state.camera.0;
                        *camera_pos_z = state.camera.1;
                        return;
                    }

                    debug!("act");
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
            "redo" => {
                debug!(
                    "wants forward, forward: {}, back: {}",
                    undo.has_forward(),
                    undo.has_back()
                );

                if to_drop.get_single().is_ok() {
                    action_to_send = Some("drop");
                    break;
                }

                if cutter.get_single().is_ok() {
                    action_to_send = Some("cancel");
                    break;
                }

                if let Some(state) = undo.next() {
                    audio
                        .play(asset_server.load(
                            "audio/zapsplat_sport_surfboard_leash_velcro_strap_undo_004.mp3",
                        ));
                    if state.is_action && *cursor_pos != state.cursor {
                        debug!("repos");
                        *cursor_pos = state.cursor;
                        *camera_pos = state.camera.0;
                        *camera_pos_z = state.camera.1;
                        return;
                    }

                    debug!("{} planks in forward", state.level.planks.len());
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
            _ => (),
        }
    }

    if let Some(action) = action_to_send {
        actions.send(ActionEvent {
            label: ActionLabel(action),
            sender: Entity::from_raw(0),
            target: None,
        });
    }
}

enum CutEvent {
    NewCut { from: IVec2, to: IVec2, speed: f32 },
    UnCut { from: IVec2, to: IVec2 },
    CancelCut,
    FinishCut,
    UnfinishCut,
}

fn extend_cut(
    mut cutter: Query<
        (
            &mut Cut,
            &mut Position,
            &mut PrevPosition,
            &mut Transform,
            &MoveSpeed,
        ),
        (Without<Targeted>, Changed<Position>),
    >,
    selected: Query<(&PlankComponent, &Position), With<Targeted>>,
    mut cuts: EventWriter<CutEvent>,
) {
    for (mut cut, mut position, mut prev, mut trans, speed) in cutter.iter_mut() {
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
                    trans.translation = position.0.as_vec2().extend(trans.translation.z);
                    continue;
                }
            };

            let affected = (affected.0 - plank_pos.0, affected.1 - plank_pos.0);

            if !plank.0.contains(affected.0) && !plank.0.contains(affected.1) {
                debug!("air block");
                position.0 = prev.0;
                trans.translation = position.0.as_vec2().extend(trans.translation.z);
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
                trans.translation = position.0.as_vec2().extend(trans.translation.z);
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
                    speed: speed.0,
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
    time: Res<Time>,
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
            CutEvent::NewCut { from, to, speed } => {
                let id = commands
                    .spawn_bundle(PbrBundle {
                        mesh: meshes.add(
                            BLQuad::new((*from - *to).abs().as_vec2() + 0.2, Vec2::ZERO).into(),
                        ),
                        material: working.clone(),
                        transform: Transform {
                            translation: (from.as_vec2() - 0.1).extend(PLANK_Z_HILIGHTED + 0.01),
                            scale: Vec3::new(0.2 / 1.2, 0.2 / 1.2, 1.0),
                            ..Default::default()
                        },
                        ..Default::default()
                    })
                    .insert(CuttingAnimation {
                        start: time.seconds_since_startup(),
                        speed: *speed,
                        from: from.as_vec2(),
                        to: to.as_vec2(),
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

#[derive(Component)]
pub struct CuttingAnimation {
    pub start: f64,
    pub speed: f32,
    pub from: Vec2,
    pub to: Vec2,
}

fn animate_cuts(
    mut cuts: Query<(Entity, &CuttingAnimation, &mut Transform)>,
    mut commands: Commands,
    time: Res<Time>,
    mut spawn_time: Local<f64>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut data: Local<Option<(Handle<Mesh>, Handle<StandardMaterial>)>>,
    mut prev_cutting: Local<Option<f64>>,
    asset_server: Res<AssetServer>,
    audio: Res<AudioChannel<CutChannel>>,
) {
    let (mesh, mat) = data.get_or_insert_with(|| {
        (
            meshes.add(
                shape::Icosphere {
                    radius: 1.0,
                    subdivisions: 5,
                }
                .into(),
            ),
            mats.add(StandardMaterial {
                base_color: Color::rgba(1.0, 1.0, 0.0, 1.0),
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                ..Default::default()
            }),
        )
    });

    if !cuts.is_empty() {
        if prev_cutting.is_none() {
            debug!("begin sound");
            audio.set_playback_rate(0.5);
            audio.play_looped(asset_server.load("audio/zapsplat_office_compass_pencil_draw_circle_on_paper_003_22758-[AudioTrimmer.com].mp3"));
        }
        *prev_cutting = Some(time.seconds_since_startup());
    }
    for (ent, cut, mut trans) in cuts.iter_mut() {
        let perc = f32::min(
            1.0,
            ((time.seconds_since_startup() - cut.start) * cut.speed as f64) as f32,
        );
        let end = cut.from + (cut.to - cut.from) * perc;
        let bl = cut.from.min(end);
        let tr = cut.from.max(end);
        trans.translation = (bl - 0.1).extend(PLANK_Z_HILIGHTED + 0.01);
        let (spray_x, spray_y);
        if cut.from.x == cut.to.x {
            trans.scale.x = 1.0;
            trans.scale.y = (0.2 + tr.y - bl.y) / 1.2;
            spray_x = -20.0..20.0;
            if cut.from.y > cut.to.y {
                spray_y = -25.0..0.0;
            } else {
                spray_y = 0.0..25.0;
            }
        } else {
            trans.scale.y = 1.0;
            trans.scale.x = (0.2 + tr.x - bl.x) / 1.2;
            spray_y = -20.0..20.0;
            if cut.from.x > cut.to.x {
                spray_x = -25.0..0.0;
            } else {
                spray_x = 0.0..25.0;
            }
        }

        if perc == 1.0 {
            commands.entity(ent).remove::<CuttingAnimation>();
        }

        let mut rng = thread_rng();
        let spawn_count = (rng.gen_range(100.0..200.0) * time.delta_seconds()) as usize;
        for _ in 0..spawn_count {
            commands
                .spawn_bundle(PbrBundle {
                    mesh: mesh.clone(),
                    material: mat.clone(),
                    transform: Transform::from_translation(end.extend(PLANK_Z_HILIGHTED))
                        .with_scale(Vec3::splat(rng.gen_range(0.05..0.10))),
                    ..Default::default()
                })
                .insert(Velocity(Vec3::new(
                    rng.gen_range(spray_x.clone()),
                    rng.gen_range(spray_y.clone()),
                    rng.gen_range(0.0..5.0),
                )))
                .insert(Die(time.seconds_since_startup() + rng.gen_range(0.0..0.1)));

            *spawn_time = time.seconds_since_startup();
        }
    }

    if let Some(end_time) = *prev_cutting {
        if time.seconds_since_startup() > end_time + 0.25 {
            debug!("end sound");
            audio.stop();
            *prev_cutting = None;
        }
    }
}

#[derive(Component)]
pub struct Velocity(pub Vec3);

#[derive(Component)]
pub struct Die(pub f64);

fn animate_sparks(
    mut commands: Commands,
    mut query: Query<(Entity, &Velocity, &Die, &mut Transform)>,
    time: Res<Time>,
) {
    for (ent, vel, die, mut trans) in query.iter_mut() {
        trans.translation += vel.0 * time.delta_seconds();
        if time.seconds_since_startup() > die.0 {
            commands.entity(ent).despawn_recursive();
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
                transform.translation.z = PLANK_Z_HILIGHTED;
                found = Some(ent);
                continue;
            }

            transform.translation.z = PLANK_Z;
            commands.entity(ent).remove::<Targeted>();
        }
    }
}

struct SpawnPlank {
    plank: Plank,
    position: Position,
    is_plank: bool,
    is_interactive: bool,
    manual_extents: Option<IVec2>,
}

fn spawn_planks(
    mut evs: EventReader<SpawnPlank>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<WoodMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    for ev in evs.iter() {
        let mut plank = ev.plank.clone();
        let mut pos = ev.position;

        let fixed_extents = ev.manual_extents;

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

        let colors = match (ev.is_plank, ev.is_interactive) {
            (true, true) => (1.5, 1.2, 1.0),
            (true, false) => (1.5, 1.2, 0.0),
            (false, _) => (0.8, 1.0, 0.0),
        };

        let plank_spec = WoodMaterialSpec {
            texture_offset: plank.texture_offset,
            turns: plank.turns,
            primary_color: Color::rgba(0.562, 0.272, 0.136, 1.0) * colors.0,
            secondary_color: Color::rgba(0.384, 0.13, 0.118, 1.0) * colors.1,
            hilight_color: Color::rgba(0.2, 0.2, 1.0, 1.0) * colors.2,
            size: size.as_uvec2(),
            is_plank: ev.is_plank,
            base_color_texture: create_coordset_image(&mut images, &plank),
        };

        debug!("plank offset: {}", plank.texture_offset);

        let mat_handle = mats.add(SimpleTextureMaterial(plank_spec));
        let cloned_mat = mat_handle.clone_weak();
        let mut cmds = commands.spawn();

        let z = match ev.is_plank {
            true => match ev.is_interactive {
                true => PLANK_Z,
                false => PLANK_Z_DONE,
            },
            false => HOLE_Z,
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

        if ev.is_plank {
            if ev.is_interactive {
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
                ev.0.as_vec2().extend(PLANK_Z_DONE) + Vec3::new(0.5, 0.5, 0.0),
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
    target: Query<(Entity, &PlankComponent, &Position, &Transform), Without<Selected>>,
    mut menu: EventWriter<PopupMenuEvent>,
    levelset: Res<LevelSet>,
    asset_server: Res<AssetServer>,
    audio: Res<AudioChannel<HammerChannel>>,
    mut spawn_nails: EventWriter<SpawnNail>,
    mut snap: EventWriter<SnapUndo>,
    mut settings: ResMut<PkvStore>,
) {
    let Ok(hole_pos) = holes.get_single() else {
        return;
    };

    let mut rng = thread_rng();

    for (plank_ent, plank, pos, trans) in target.iter() {
        let mut shifted = plank.0.clone();
        shifted.shift(pos.0 - hole_pos.0);
        for (i, hole) in level.holes.holes.iter().enumerate() {
            if shifted.equals(&hole) {
                debug!("hammer!");

                let mut new_trans = trans.clone();
                new_trans.translation.z = PLANK_Z_DONE;

                commands
                    .entity(plank_ent)
                    .remove::<PlankComponent>()
                    .remove::<Targeted>()
                    .remove::<Selected>()
                    .remove::<Controller>()
                    .insert(new_trans);
                level.holes.holes.remove(i);

                let max = 2.max(shifted.count() / 2);
                shifted.shift(hole_pos.0);

                let mut coords = shifted.coords.iter().collect::<Vec<_>>();
                coords.shuffle(&mut rng);

                audio.set_playback_rate(rng.gen_range(1.0..1.5));
                audio.play(asset_server.load("audio/aaj_0404_HamrNail4Hits.mp3"));

                let mut nails = Vec::new();
                for coord in coords.into_iter().take(rng.gen_range(2..=max)) {
                    spawn_nails.send(SpawnNail(*coord));
                    nails.push(*coord);
                }

                done_planks.0.push((plank.0.clone(), *pos, nails));

                if level.holes.holes.is_empty() {
                    debug!("you win!");

                    if let Ok(current) = settings.get(levelset.settings_key) {
                        settings
                            .set(
                                levelset.settings_key,
                                &29usize.min(levelset.current_level + 1).max(current),
                            )
                            .unwrap();
                    }

                    let mut items = vec![
                        ("Restart Level".into(), ActionLabel("restart"), true),
                        ("Main Menu".into(), ActionLabel("main menu"), true),
                        (
                            "Quit to Desktop".into(),
                            ActionLabel("quit"),
                            QUIT_TO_DESKTOP,
                        ),
                    ];

                    let next = levelset.current_level + 1;

                    items.insert(
                        0,
                        ("Next Level".into(), ActionLabel("next level"), next < 30),
                    );

                    menu.send(PopupMenuEvent {
                        sender: Entity::from_raw(0),
                        menu: PopupMenu {
                            heading: format!("Nice one!\n {}/{} completed!", next, 30),
                            items,
                            ..Default::default()
                        },
                        sound: true,
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
    mut ev: EventReader<ActionEvent>,
    mut quit: EventWriter<AppExit>,
    mut levelset: ResMut<LevelSet>,
) {
    for ev in ev.iter() {
        match ev.label.0 {
            "next level" => {
                levelset.current_level += 1;
                spawn_event.send(SpawnLevelEvent {
                    def: levelset.levels[levelset.current_level].clone(),
                });
            }
            "restart" => {
                spawn_event.send(SpawnLevelEvent {
                    def: levelset.levels[levelset.current_level].clone(),
                });
            }
            "quit" => {
                quit.send_default();
            }
            _ => (),
        }
    }
}

fn check_cut_actions(
    mut cutter: Query<
        (&mut Controller, &Cut),
        (With<Cut>, Without<Cursor>, Without<SystemController>),
    >,
    mut cursor: Query<&mut Controller, (Without<Cut>, With<Cursor>, Without<SystemController>)>,
    mut system: Query<&mut Controller, (Without<Cut>, Without<Cursor>, With<SystemController>)>,
    undo: Res<UndoBuffer>,
    select: Query<(), With<Selected>>,
    target: Query<(), With<Targeted>>,
) {
    fn set(controller: &mut Controller, label: &'static str, active: bool) {
        controller
            .actions
            .iter_mut()
            .find(|(_, action)| action.label == ActionLabel(label))
            .unwrap()
            .1
            .display = match active {
            true => DisplayMode::Active,
            false => DisplayMode::Inactive,
        }
    }

    let mut cutting = false;
    for (mut controller, cut) in cutter.iter_mut() {
        set(&mut controller, "finish cut", cut.finished);
        cutting = true;
    }

    for mut controller in system.iter_mut() {
        set(&mut controller, "undo", !cutting && undo.has_back());
        set(&mut controller, "redo", !cutting && undo.has_forward());
    }

    let has_select = !select.is_empty();
    let has_target = !target.is_empty();

    for mut controller in cursor.iter_mut() {
        let main_action = controller
            .actions
            .iter_mut()
            .find(|(typ, _)| typ == &ActionType::MainAction)
            .unwrap();
        match (has_select, has_target) {
            (true, true) => {
                main_action.1.label = ActionLabel("swap");
                main_action.1.display = DisplayMode::Active;
            }
            (true, false) => {
                main_action.1.label = ActionLabel("drop");
                main_action.1.display = DisplayMode::Active;
            }
            (false, true) => {
                main_action.1.label = ActionLabel("grab");
                main_action.1.display = DisplayMode::Active;
            }
            (false, false) => {
                main_action.1.label = ActionLabel("grab");
                main_action.1.display = DisplayMode::Inactive;
            }
        }

        set(&mut controller, "cut", !has_select && has_target);
    }
}

#[allow(unused)]
fn debug_actions(mut evs: EventReader<ActionEvent>) {
    for ev in evs.iter() {
        println!("{:?}", ev);
    }
}
