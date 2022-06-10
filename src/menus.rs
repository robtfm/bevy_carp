use bevy::{
    ecs::event::{Events, ManualEventReader},
    prelude::*,
    utils::HashMap,
};
use bevy_egui::{egui, EguiContext};
use bevy_kira_audio::AudioChannel;
use bevy_pkv::PkvStore;
use egui_extras::StripBuilder;

use crate::{
    input::Controller,
    model::{CoordSet, LevelBase},
    spawn_random,
    structs::{
        ActionEvent, ChangeBackground, MenuItem, PopupMenu, PopupMenuEvent, Position, PositionZ, QUIT_TO_DESKTOP,
    },
    LevelDef, LevelSet, MenuChannel, Permanent, SpawnLevelEvent, SpawnPlank,
};

#[derive(Component)]
struct MenuSelect;

pub(crate) fn spawn_main_menu(
    mut actions: ResMut<Events<ActionEvent>>,
    mut reader: Local<ManualEventReader<ActionEvent>>,
    mut commands: Commands,
    all: Query<Entity>,
    mut spawn_planks: EventWriter<SpawnPlank>,
    mut popup: EventWriter<PopupMenuEvent>,
    mut bg: EventWriter<ChangeBackground>,
    images: Res<Assets<Image>>,
    server: Res<AssetServer>,
    mut handle: Local<Option<Handle<Image>>>,
) {
    let mut run = false;

    for ev in reader.iter(&actions) {
        if ev.label == "main menu" {
            run = true;
        }
    }

    if !run {
        return;
    }

    let handle = handle.get_or_insert_with(|| server.load("images/title.png"));

    let Some(image) = images.get(&*handle) else {
        // try again next time
        actions.send(ActionEvent{
            sender: Entity::from_raw(0),
            label: "main menu",
            target: None,
        });
        return;
    };

    for ent in all.iter() {
        // even permanents
        commands.entity(ent).despawn_recursive();
    }

    bg.send_default();

    let mut plank = CoordSet::default();
    let width = image.size().x as usize;

    for (i, word) in image.data.chunks(4).enumerate() {
        if word.iter().any(|b| *b < 254) {
            plank
                .coords
                .insert(IVec2::new((i % width) as i32, -((i / width) as i32)));
        }
    }

    plank = plank.normalize();
    plank.shift(IVec2::ONE);
    // let manual_extents = Some(image.size().as_ivec2());

    spawn_planks.send(SpawnPlank {
        plank,
        position: Position::default(),
        is_plank: true,
        is_interactive: false,
        manual_extents: None,
    });

    let cam_id = commands
        .spawn_bundle(PerspectiveCameraBundle {
            perspective_projection: PerspectiveProjection {
                fov: std::f32::consts::PI / 4.0,
                ..Default::default()
            },
            ..default()
        })
        .insert(Position::default())
        .insert(PositionZ::default())
        .id();

    actions.send(ActionEvent {
        sender: cam_id,
        label: "focus",
        target: None,
    });

    popup.send(PopupMenuEvent {
        sender: cam_id,
        menu: PopupMenu {
            items: vec![
                ("Play".into(), "play", true),
                ("Options (tbd)".into(), "options", false),
                ("Credits".into(), "credits", true),
                ("Quit to Desktop".into(), "quit", QUIT_TO_DESKTOP),
            ],
            transparent: true,
            header_size: 0.4,
            footer: format!("v{}", env!("CARGO_PKG_VERSION")),
            ..Default::default()
        },
        sound: false,
    });
}

pub fn spawn_credits(mut ev: EventReader<ActionEvent>, mut menu: EventWriter<PopupMenuEvent>) {
    for ev in ev.iter() {
        if ev.label == "credits" {
            menu.send(PopupMenuEvent {
                sender: Entity::from_raw(0),
                menu: PopupMenu {
                    items: vec![
                        ("".into(), "", false),
                        ("Measure".into(), "", false),
                        ("".into(), "", false),
                        ("by".into(), "", false),
                        ("".into(), "", false),
                        ("".into(), "", false),
                        ("Once".into(), "", false),
                        ("".into(), "", false),
                        ("robtfm".into(), "", false),
                        ("".into(), "", false),
                        ("".into(), "", false),
                        ("".into(), "", false),
                        ("".into(), "", false),
                        ("".into(), "", false),
                        ("".into(), "", false),
                        ("".into(), "", false),
                        ("built".into(), "", false),
                        ("".into(), "", false),
                        ("bevy".into(), "", false),
                        ("".into(), "", false),
                        ("".into(), "", false),
                        ("using".into(), "", false),
                        ("".into(), "", false),
                        ("kira".into(), "", false),
                        ("".into(), "", false),
                        ("".into(), "", false),
                        ("".into(), "", false),
                        ("".into(), "", false),
                        ("egui".into(), "", false),
                        ("".into(), "", false),
                        ("".into(), "", false),
                        ("".into(), "", false),
                        ("".into(), "", false),
                        ("pkvstore".into(), "", false),
                        ("".into(), "", false),
                        ("".into(), "", false),
                        ("".into(), "", false),
                        ("".into(), "", false),
                        ("".into(), "", false),
                        ("".into(), "", false),
                        ("".into(), "", false),
                        ("SFX".into(), "", false),
                        ("".into(), "", false),
                        ("zapsplat".into(), "", false),
                        ("".into(), "", false),
                        ("".into(), "", false),
                        ("".into(), "", false),
                        ("Ok".into(), "main menu", true),
                        ("".into(), "", false),
                        ("".into(), "", false),
                    ],
                    cancel_action: Some("main menu"),
                    header_size: 0.0,
                    width: 5,
                    ..Default::default()
                },
                sound: false,
            });
        }
    }
}

pub fn spawn_play_menu(
    evs: ResMut<Events<ActionEvent>>,
    mut reader: Local<ManualEventReader<ActionEvent>>,
    mut spawn_menu: EventWriter<PopupMenuEvent>,
    mut spawn_level: EventWriter<SpawnLevelEvent>,
    mut levelset: ResMut<LevelSet>,
    mut settings: ResMut<PkvStore>,
) {
    let today = chrono::Utc::today().naive_utc();
    let start_date = chrono::NaiveDate::from_ymd(2022, 6, 1);

    // this sucks i know
    let strs = [
        "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15", "16",
        "17", "18", "19", "20", "21", "22", "23", "24", "25", "26", "27", "28", "29",
    ];

    for ev in reader.iter(&evs) {
        let key;
        for i in 0..30 {
            if ev.label == strs[i] {
                levelset.current_level = i;
                spawn_level.send(SpawnLevelEvent {
                    def: levelset.levels[i].clone(),
                });
            }
        }

        match ev.label {
            "play" => {
                spawn_menu.send(PopupMenuEvent {
                    sender: ev.sender,
                    menu: PopupMenu {
                        heading: "Choose Difficulty".into(),
                        items: vec![
                            ("Daily Mix".into(), "play daily", true),
                            ("Easy".into(), "play easy", true),
                            ("Medium".into(), "play medium", true),
                            ("Hard".into(), "play hard", true),
                        ],
                        cancel_action: Some("main menu"),
                        ..Default::default()
                    },
                    sound: false,
                });
                return;
            }
            "play easy" => {
                key = "Easy";
                *levelset = spawn_random(90, 0, "Easy Set".into(), 25, key);
            }
            "play medium" => {
                key = "Medium";
                *levelset = spawn_random(90, 30, "Medium Set".into(), 15, key);
            }
            "play hard" => {
                key = "Hard";
                *levelset = spawn_random(90, 60, "Hard Set".into(), 15, key);
            }
            "play daily" => {
                let dur = today.signed_duration_since(start_date);
                let seed = dur.num_days() * 1068;
                key = "Daily";
                *levelset =
                    spawn_random(30, 0, format!("Daily Set for {}", today), seed as u64, key);
            }
            _ => return,
        }

        if key == "Daily" {
            let current_daily = settings.get("current daily date").unwrap_or(start_date);
            if current_daily != today {
                settings.set(key, &0usize).unwrap();
                settings.set("current daily date", &today).unwrap();
            }
        }

        let max_level: usize = settings.get(key).unwrap_or_default();
        if max_level == 0 {
            spawn_level.send(SpawnLevelEvent {
                def: levelset.levels[0].clone(),
            });
            return;
        }

        // if we get here we must have chosen a set, and already started the set
        let items = (0..30)
            .map(|i| ((i + 1).to_string(), strs[i], i <= max_level))
            .collect();

        let menu = PopupMenu {
            heading: format!("{}\nSelect Level", levelset.title),
            items,
            cancel_action: Some("play"),
            width: 6,
            ..Default::default()
        };

        spawn_menu.send(PopupMenuEvent {
            menu,
            sender: Entity::from_raw(0),
            sound: false,
        });
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
                    heading: format!("Paused ({}/{})\n{}", set.current_level + 1, 30, set.title,),
                    items: vec![
                        ("Resume".into(), "cancel", true),
                        ("Restart Level".into(), "restart", true),
                        ("Main Menu".into(), "main menu", true),
                        ("Quit to Desktop".into(), "quit", QUIT_TO_DESKTOP),
                    ],
                    cancel_action: Some("cancel"),
                    ..Default::default()
                },
                sound: true,
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
        if ev.sound {
            audio.set_playback_rate(1.2);
            audio.play(asset_server.load("audio/zapsplat_multimedia_game_sound_game_show_correct_tone_bright_positive_006_80747.mp3"));
        }

        for (ent, mut controller) in other_controllers.iter_mut() {
            prev_controller_state.insert(ent, controller.enabled);
            controller.enabled = false;
        }

        let mut action = vec![
            ("up", ("move up", true), false),
            ("up", ("pan up", true), true),
            ("up", ("zoom in", true), false),
            ("down", ("move down", true), false),
            ("down", ("pan down", true), true),
            ("down", ("zoom out", true), false),
            ("cancel", ("menu", true), false),
            ("cancel", ("second action", true), true),
            ("select", ("main action", true), true),
        ];

        if ev.menu.width > 1 {
            action.extend(vec![
                ("left", ("move left", true), false),
                ("left", ("pan left", true), true),
                ("right", ("move right", true), false),
                ("right", ("pan right", true), true),
            ])
        }

        commands
            .spawn()
            .insert(Controller {
                display_order: 4,
                enabled: true,
                action,
                ..Default::default()
            })
            .insert(Position(IVec2::ZERO))
            .insert(MenuItem)
            .insert(Permanent);

        debug!("menu");

        *active_menu = Some((ev.menu.clone(), ev.sender));
        *menu_position = 0;

        while !ev.menu.items[*menu_position].2 {
            *menu_position += 1;
        }
    }

    if let Some((menu, _)) = active_menu.as_ref() {
        let fill: egui::Color32 = match menu.transparent {
            true => egui::Rgba::from_rgba_premultiplied(0.0, 0.0, 0.0, 0.0).into(),
            false => egui::Rgba::from_rgba_premultiplied(0.0, 0.0, 0.0, 0.8).into(),
        };

        let row_count = (menu.items.len() as f32 / menu.width as f32).ceil() as usize;

        egui::CentralPanel::default()
            .frame(egui::Frame {
                fill,
                ..Default::default()
            })
            .show(egui_context.ctx_mut(), |ui| {
                StripBuilder::new(ui)
                    .size(egui_extras::Size::relative(0.1))
                    .size(egui_extras::Size::relative(menu.header_size))
                    .sizes(
                        egui_extras::Size::relative(
                            (1.0 - 0.1 - menu.header_size - 0.166) / row_count as f32,
                        ),
                        menu.items.len(),
                    )
                    .size(egui_extras::Size::remainder())
                    .vertical(|mut strip| {
                        strip.empty();
                        strip.cell(|ui| {
                            let heading = egui::RichText::from(menu.heading.as_str()).size(100.0);
                            ui.vertical_centered(|ui| ui.label(heading));
                        });

                        if menu.width == 1 {
                            for (i, (text, _, enabled)) in menu.items.iter().enumerate() {
                                strip.cell(|ui| {
                                    let mut text = egui::RichText::from(text).size(50.0);
                                    if !enabled {
                                        text = text.color(egui::Color32::from_rgb(100, 100, 75));
                                    }
                                    ui.vertical_centered(|ui| {
                                        if i == *menu_position {
                                            ui.label(text.background_color(
                                                egui::Rgba::from_rgba_premultiplied(
                                                    0.2, 0.2, 0.2, 0.2,
                                                ),
                                            ));
                                        } else {
                                            ui.label(text);
                                        }
                                    });
                                });
                            }
                        } else {
                            for i in 0..row_count {
                                strip.strip(|strip| {
                                    strip
                                        .sizes(
                                            egui_extras::Size::relative(1.0 / menu.width as f32),
                                            menu.width,
                                        )
                                        .horizontal(|mut strip| {
                                            for j in 0..menu.width {
                                                strip.cell(|ui| {
                                                    let pos = i * menu.width + j;
                                                    let (text, _, enabled) = &menu.items[pos];
                                                    let mut text =
                                                        egui::RichText::from(text).size(50.0);
                                                    if !enabled {
                                                        text = text.color(egui::Color32::from_rgb(
                                                            100, 100, 75,
                                                        ));
                                                    }
                                                    ui.vertical_centered(|ui| {
                                                        if pos == *menu_position {
                                                            ui.label(text.background_color(
                                                                egui::Rgba::from_rgba_premultiplied(
                                                                    0.2, 0.2, 0.2, 0.2,
                                                                ),
                                                            ));
                                                        } else {
                                                            ui.label(text);
                                                        }
                                                    });
                                                });
                                            }
                                        });
                                })
                            }
                        }

                        strip.cell(|ui| {
                            let footer = egui::RichText::from(menu.footer.as_str()).size(15.0);
                            ui.with_layout(egui::Layout::bottom_up(egui::Align::Max), |ui| ui.label(footer));
                        });
                    });
            });
    }

    let mut to_send = None;

    for ev in action_reader.iter(&actions) {
        if menu_items.get(ev.sender).is_ok() {
            match ev.label {
                "up" | "left" => {
                    let active_items = &active_menu.as_ref().unwrap().0.items;
                    let width = match ev.label {
                        "up" => active_menu.as_ref().unwrap().0.width,
                        "left" => 1,
                        _ => panic!(),
                    };
                    loop {
                        if *menu_position < width {
                            while *menu_position + width < active_items.len() {
                                *menu_position += width;
                            }
                        } else {
                            *menu_position -= width;
                        }

                        if active_items[*menu_position].2
                        // enabled
                        {
                            break;
                        }
                    }
                    audio.set_playback_rate(1.2);
                    audio.play(asset_server.load("audio/zapsplat_multimedia_alert_mallet_hit_short_single_generic_003_79278.mp3"));
                }
                "down" | "right" => {
                    let active_items = &active_menu.as_ref().unwrap().0.items;
                    let width = match ev.label {
                        "down" => active_menu.as_ref().unwrap().0.width,
                        "right" => 1,
                        _ => panic!(),
                    };
                    loop {
                        if *menu_position + width >= active_items.len() {
                            while *menu_position >= width {
                                *menu_position -= width;
                            }
                        } else {
                            *menu_position += width;
                        }

                        if active_items[*menu_position].2
                        // enabled
                        {
                            break;
                        }
                    }
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
