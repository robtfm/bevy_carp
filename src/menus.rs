use bevy::{
    ecs::event::{Events, ManualEventReader},
    prelude::*,
    utils::HashMap,
};
use bevy_egui::{egui, EguiContext};
use bevy_kira_audio::AudioChannel;
use egui_extras::StripBuilder;

use crate::{
    input::Controller,
    model::LevelBase,
    spawn_random,
    structs::{ActionEvent, MenuItem, PopupMenu, PopupMenuEvent, Position},
    LevelDef, LevelSet, MenuChannel, Permanent, SpawnLevelEvent,
};

#[derive(Component)]
struct MenuSelect;

pub fn spawn_main_menu(mut evs: EventReader<ActionEvent>, mut spawn: EventWriter<PopupMenuEvent>) {
    for ev in evs.iter() {
        if ev.label == "main menu" {
            spawn.send(PopupMenuEvent {
                sender: ev.sender,
                menu: PopupMenu {
                    heading: "Main Menu".into(),
                    items: vec![
                        ("Play".into(), "play"),
                        ("Options".into(), "options"),
                        ("Quit to Desktop".into(), "quit"),
                    ],
                    cancel_action: None,
                },
            })
        }
    }
}

pub fn spawn_play_menu(
    evs: ResMut<Events<ActionEvent>>,
    mut reader: Local<ManualEventReader<ActionEvent>>,
    mut spawn_menu: EventWriter<PopupMenuEvent>,
    mut spawn_level: EventWriter<SpawnLevelEvent>,
    mut levelset: ResMut<LevelSet>,
) {
    for ev in reader.iter(&evs) {
        match ev.label {
            "play" => {
                spawn_menu.send(PopupMenuEvent {
                    sender: ev.sender,
                    menu: PopupMenu {
                        heading: "Choose Difficulty".into(),
                        items: vec![
                            ("Easy".into(), "play easy"),
                            ("Medium".into(), "play medium"),
                            ("Hard".into(), "play hard"),
                            ("Mixed".into(), "play mix"),
                        ],
                        cancel_action: Some("main menu"),
                    },
                });
            }
            "play easy" => {
                *levelset = LevelSet::default();
                spawn_random(&mut spawn_level, &mut levelset, 90, 0);
            }
            "play medium" => {
                *levelset = LevelSet::default();
                spawn_random(&mut spawn_level, &mut levelset, 90, 30);
            }
            "play hard" => {
                *levelset = LevelSet::default();
                spawn_random(&mut spawn_level, &mut levelset, 90, 60);
            }
            "play mix" => {
                *levelset = LevelSet::default();
                spawn_random(&mut spawn_level, &mut levelset, 30, 0);
            }
            _ => (),
        }
    }
}

pub fn spawn_in_level_menu(
    mut evs: EventReader<ActionEvent>,
    level: Res<LevelDef>,
    set: Res<LevelSet>,
    base: Res<LevelBase>,
    mut spawn: EventWriter<PopupMenuEvent>,
) {
    for ev in evs.iter() {
        if ev.label == "pause" {
            println!(
                "Paused\n[{}/{}/{}]",
                level.num_holes, level.total_blocks, level.seed
            );
            println!("difficulty: {}", base.0.difficulty());
            spawn.send(PopupMenuEvent {
                sender: ev.sender,
                menu: PopupMenu {
                    heading: format!(
                        "Paused ({}/{})\n",
                        set.1 + 1,
                        set.0.iter().filter(|l| l.is_some()).count()
                    ),
                    items: vec![
                        ("Resume".into(), "cancel"),
                        ("Restart Level".into(), "restart"),
                        ("Main Menu".into(), "main menu"),
                        ("Quit to Desktop".into(), "quit"),
                    ],
                    cancel_action: Some("cancel"),
                },
            })
        }
    }
}

pub fn spawn_popup_menu(
    mut commands: Commands,
    mut other_controllers: Query<(Entity, &mut Controller), Without<MenuItem>>,
    mut prev_controller_state: Local<HashMap<Entity, bool>>,
    mut spawn_evs: EventReader<PopupMenuEvent>,
    mut actions: ResMut<Events<ActionEvent>>,
    mut action_reader: Local<ManualEventReader<ActionEvent>>,
    mut egui_context: ResMut<EguiContext>,
    menu_items: Query<Entity, With<MenuItem>>,
    mut active_menu: Local<Option<(PopupMenu, Entity)>>,
    mut menu_position: Local<usize>,
    asset_server: Res<AssetServer>,
    audio: Res<AudioChannel<MenuChannel>>,
) {
    for ev in spawn_evs.iter() {
        for (ent, mut controller) in other_controllers.iter_mut() {
            prev_controller_state.insert(ent, controller.enabled);
            controller.enabled = false;
        }

        commands
            .spawn()
            .insert(Controller {
                action: vec![
                    ("up", ("move up", true), false),
                    ("up", ("pan up", true), true),
                    ("up", ("zoom in", true), false),
                    ("down", ("move down", true), false),
                    ("down", ("pan down", true), true),
                    ("down", ("zoom out", true), false),
                    ("cancel", ("menu", true), false),
                    ("cancel", ("second action", true), true),
                    ("select", ("main action", true), true),
                ],
                enabled: true,
                ..Default::default()
            })
            .insert(Position(IVec2::ZERO))
            .insert(MenuItem)
            .insert(Permanent);

        debug!("menu");

        *active_menu = Some((ev.menu.clone(), ev.sender));
        *menu_position = 0;
    }

    if let Some((menu, _)) = active_menu.as_ref() {
        egui::CentralPanel::default().show(egui_context.ctx_mut(), |ui| {
            StripBuilder::new(ui)
                .size(egui_extras::Size::relative(0.33))
                .sizes(
                    egui_extras::Size::relative(0.5 / menu.items.len() as f32),
                    menu.items.len(),
                )
                .size(egui_extras::Size::remainder())
                .vertical(|mut strip| {
                    strip.cell(|ui| {
                        let heading = egui::RichText::from(menu.heading.as_str()).size(100.0);
                        ui.vertical_centered(|ui| ui.label(heading));
                    });

                    for (i, (text, _)) in menu.items.iter().enumerate() {
                        strip.cell(|ui| {
                            let text = egui::RichText::from(text).size(60.0);
                            ui.vertical_centered(|ui| {
                                if i == *menu_position {
                                    ui.label(text.background_color(
                                        egui::Rgba::from_rgba_premultiplied(0.2, 0.2, 0.2, 0.2),
                                    ));
                                } else {
                                    ui.label(text);
                                }
                            });
                        });
                    }
                });
        });
    }

    let mut to_send = None;

    for ev in action_reader.iter(&actions) {
        if menu_items.get(ev.sender).is_ok() {
            match ev.label {
                "up" => {
                    if *menu_position == 0 {
                        *menu_position = active_menu.as_ref().unwrap().0.items.len() - 1;
                    } else {
                        *menu_position -= 1;
                    }
                    audio.set_playback_rate(1.2);
                    audio.play(asset_server.load("audio/zapsplat_multimedia_alert_mallet_hit_short_single_generic_003_79278.mp3"));
                }
                "down" => {
                    *menu_position =
                        (*menu_position + 1) % active_menu.as_ref().unwrap().0.items.len();
                    audio.set_playback_rate(1.2);
                    audio.play(asset_server.load("audio/zapsplat_multimedia_alert_mallet_hit_short_single_generic_003_79278.mp3"));
                }
                "cancel" => {
                    let Some(cancel_action) = active_menu.as_ref().unwrap().0.cancel_action else {
                        continue;
                    };

                    for item in menu_items.iter() {
                        commands.entity(item).despawn_recursive();
                    }

                    for (ent, mut controller) in other_controllers.iter_mut() {
                        if let Some(prev) = prev_controller_state.get(&ent) {
                            controller.enabled = *prev;
                        }
                    }

                    to_send = Some(ActionEvent {
                        sender: ev.sender,
                        label: cancel_action,
                        target: None,
                    });
                    *active_menu = None;
                    audio.set_playback_rate(1.2);
                    audio.play(asset_server.load("audio/zapsplat_multimedia_game_sound_game_show_correct_tone_bright_positive_006_80747.mp3"));
                }
                "select" => {
                    for item in menu_items.iter() {
                        commands.entity(item).despawn_recursive();
                    }

                    for (ent, mut controller) in other_controllers.iter_mut() {
                        if let Some(prev) = prev_controller_state.get(&ent) {
                            controller.enabled = *prev;
                        }
                    }

                    let (menu, sender) = active_menu.take().unwrap();
                    to_send = Some(ActionEvent {
                        sender: sender,
                        label: menu.items[*menu_position].1,
                        target: None,
                    });
                    audio.set_playback_rate(1.2);
                    audio.play(asset_server.load("audio/zapsplat_multimedia_game_sound_game_show_correct_tone_bright_positive_006_80747.mp3"));
                }
                _ => (),
            }
        }
    }

    if let Some(event) = to_send {
        actions.send(event);
    }
}
