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
    input::{
        Action, ActionInputs, ActionType, Controller, DisplayMode, InputItem, NewInputController,
        NewInputEvent,
    },
    model::{CoordSet, LevelBase},
    spawn_random,
    structs::{
        ActionEvent, ActionLabel, ChangeBackground, ControlHelp, Position, PositionZ,
        QUIT_TO_DESKTOP,
    },
    window::{update_window, WindowModeSerial},
    CursorSpeed, CutSpeed, LevelDef, LevelSet, MenuChannel, MusicVolume, Permanent, SfxVolume,
    SpawnLevelEvent, SpawnPlank,
};

//menus

#[derive(Clone)]
pub enum MenuItem {
    Text(&'static str),
    DynText(String),
    Slider(i32, i32),
    InputList {
        items: Vec<InputItem>,
        focus: Option<usize>,
    },
}

impl From<&'static str> for MenuItem {
    fn from(s: &'static str) -> Self {
        MenuItem::Text(s)
    }
}

impl From<String> for MenuItem {
    fn from(s: String) -> Self {
        MenuItem::DynText(s)
    }
}

impl MenuItem {
    pub fn render(
        &self,
        ui: &mut egui::Ui,
        size: f32,
        color: Option<egui::Color32>,
        bg: Option<egui::Color32>,
    ) {
        match self {
            MenuItem::Text(_) | MenuItem::DynText(_) => {
                let mut text = match self {
                    MenuItem::Text(text) => egui::RichText::from(*text),
                    MenuItem::DynText(text) => egui::RichText::from(text),
                    _ => unreachable!(),
                }
                .size(size);

                if let Some(color) = color {
                    text = text.color(color);
                }

                if let Some(bg) = bg {
                    text = text.background_color(bg);
                }

                ui.vertical_centered(|ui| {
                    ui.label(text);
                });
            }
            MenuItem::Slider(current, max) => {
                let stroke = egui::Stroke {
                    width: 1.0,
                    color: color.unwrap_or(egui::Color32::WHITE),
                };
                let mut rect = ui.max_rect();
                let ppp = ui.painter().ctx().pixels_per_point();
                if rect.width() > 240.0 * ppp {
                    let extra = (rect.width() - 240.0 * ppp) / 2.0;
                    rect.set_left(rect.left() + extra);
                    rect.set_right(rect.right() - extra);
                }
                let painter = ui.painter();

                if let Some(bg) = bg {
                    painter.rect_filled(rect, egui::Rounding::none(), bg);
                }
                rect.min += egui::vec2(1.0, 1.0);
                rect.max -= egui::vec2(1.0, 1.0);

                painter.rect_stroke(rect, egui::Rounding::none(), stroke);

                let region = rect.width() - 4.0;
                let per = region as f32 / *max as f32;
                for i in 0..*current {
                    let fill_color =
                        egui::Color32::from_gray((127.0 + 128.0 * (i as f32 / *max as f32)) as u8);
                    let min = rect.left_top() + egui::vec2(2.0 + (i as f32 * per), 3.0);
                    let max = rect.left_top()
                        + egui::vec2(1.0 + ((i + 1) as f32 * per), rect.height() - 4.0);
                    painter.rect_filled(
                        egui::Rect { min, max },
                        egui::Rounding::none(),
                        fill_color,
                    );
                }
            }
            MenuItem::InputList { items, focus } => {
                let count = if focus.is_some() {
                    items.len() + 1
                } else {
                    items.len()
                };

                let req_size = count as f32 * 100.0;
                let mut rect = ui.max_rect();
                if req_size > rect.width() {
                    let extra = rect.width() - req_size / 2.0;
                    rect.min.x += extra;
                    rect.max.x -= extra;
                }

                if let Some(bg) = bg {
                    if focus.is_none() {
                        ui.painter().rect_filled(rect, egui::Rounding::none(), bg);
                    } else {
                        ui.painter().rect_stroke(
                            rect,
                            egui::Rounding::none(),
                            egui::Stroke {
                                width: 1.0,
                                color: bg,
                            },
                        );
                    }
                }

                for (i, item) in items.iter().enumerate() {
                    let rect = egui::Rect {
                        min: rect.min + egui::vec2(i as f32 * 100.0, 0.0),
                        max: egui::pos2(rect.min.x + i as f32 * 100.0 + 100.0, rect.max.y),
                    };
                    if Some(i) == *focus {
                        ui.painter().rect_filled(
                            rect,
                            egui::Rounding::none(),
                            egui::Rgba::from_rgba_premultiplied(0.6, 0.2, 0.2, 0.2),
                        );
                    }
                    let mut ui = ui.child_ui(rect, egui::Layout::top_down(egui::Align::Center));
                    item.print(&mut ui, true);
                }

                if let Some(focus) = focus {
                    let rect = egui::Rect {
                        min: rect.min + egui::vec2(items.len() as f32 * 100.0, 0.0),
                        max: egui::pos2(
                            rect.min.x + items.len() as f32 * 100.0 + 100.0,
                            rect.max.y,
                        ),
                    };
                    if items.len() == *focus {
                        ui.painter().rect_filled(
                            rect,
                            egui::Rounding::same(3.0),
                            egui::Rgba::from_rgba_premultiplied(0.2, 0.6, 0.2, 0.2),
                        );
                    }
                    let mut ui = ui.child_ui(rect, egui::Layout::top_down(egui::Align::Center));
                    ui.label("ADD");
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct PopupMenu {
    pub heading: String,
    pub items: Vec<(MenuItem, ActionLabel, bool)>,
    pub cancel_action: Option<ActionLabel>,
    pub transparent: bool,
    pub header_size: f32,
    pub width: usize,
    pub footer: String,
    pub initial_position: i32, //  -1 -> prev position
    pub inactive_color: egui::Color32,
    pub text_size: f32,
    pub modal_controller: Option<Entity>,
}

impl Default for PopupMenu {
    fn default() -> Self {
        Self {
            heading: "".into(),
            items: Vec::new(),
            cancel_action: None,
            transparent: false,
            header_size: 0.35,
            width: 1,
            footer: "".into(),
            initial_position: 0,
            inactive_color: egui::Color32::from_rgb(50, 50, 100),
            text_size: 50.0,
            modal_controller: None,
        }
    }
}

pub struct PopupMenuEvent {
    pub sender: Entity,
    pub menu: PopupMenu,
    pub sound: bool,
}

#[derive(Component)]
pub struct MenuMarker;

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
    mut def: ResMut<LevelDef>,
) {
    let mut run = false;

    for ev in reader.iter(&actions) {
        if ev.label == ActionLabel("main menu") {
            run = true;
        }
    }

    if !run {
        return;
    }

    def.num_holes = 0;

    let handle = handle.get_or_insert_with(|| server.load("images/title.png"));

    let Some(image) = images.get(&*handle) else {
        // try again next time
        actions.send(ActionEvent{
            sender: Entity::from_raw(0),
            label: ActionLabel("main menu"),
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
        label: ActionLabel("focus"),
        target: None,
    });

    popup.send(PopupMenuEvent {
        sender: cam_id,
        menu: PopupMenu {
            items: vec![
                ("Play".into(), ActionLabel("play"), true),
                ("Options".into(), ActionLabel("options"), true),
                ("Credits".into(), ActionLabel("credits"), true),
                (
                    "Quit to Desktop".into(),
                    ActionLabel("quit"),
                    QUIT_TO_DESKTOP,
                ),
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
        if ev.label == ActionLabel("credits") {
            menu.send(PopupMenuEvent {
                sender: Entity::from_raw(0),
                menu: PopupMenu {
                    items: vec![
                        ("".into(), ActionLabel(""), false),
                        ("Measure".into(), ActionLabel(""), false),
                        ("".into(), ActionLabel(""), false),
                        ("by".into(), ActionLabel(""), false),
                        ("".into(), ActionLabel(""), false),
                        ("".into(), ActionLabel(""), false),
                        ("Once".into(), ActionLabel(""), false),
                        ("".into(), ActionLabel(""), false),
                        ("robtfm".into(), ActionLabel(""), false),
                        ("".into(), ActionLabel(""), false),
                        ("".into(), ActionLabel(""), false),
                        ("".into(), ActionLabel(""), false),
                        ("".into(), ActionLabel(""), false),
                        ("".into(), ActionLabel(""), false),
                        ("".into(), ActionLabel(""), false),
                        ("".into(), ActionLabel(""), false),
                        ("built".into(), ActionLabel(""), false),
                        ("".into(), ActionLabel(""), false),
                        ("bevy".into(), ActionLabel(""), false),
                        ("".into(), ActionLabel(""), false),
                        ("".into(), ActionLabel(""), false),
                        ("using".into(), ActionLabel(""), false),
                        ("".into(), ActionLabel(""), false),
                        ("kira".into(), ActionLabel(""), false),
                        ("".into(), ActionLabel(""), false),
                        ("".into(), ActionLabel(""), false),
                        ("".into(), ActionLabel(""), false),
                        ("".into(), ActionLabel(""), false),
                        ("egui".into(), ActionLabel(""), false),
                        ("".into(), ActionLabel(""), false),
                        ("".into(), ActionLabel(""), false),
                        ("".into(), ActionLabel(""), false),
                        ("".into(), ActionLabel(""), false),
                        ("pkvstore".into(), ActionLabel(""), false),
                        ("".into(), ActionLabel(""), false),
                        ("".into(), ActionLabel(""), false),
                        ("".into(), ActionLabel(""), false),
                        ("".into(), ActionLabel(""), false),
                        ("".into(), ActionLabel(""), false),
                        ("".into(), ActionLabel(""), false),
                        ("".into(), ActionLabel(""), false),
                        ("SFX".into(), ActionLabel(""), false),
                        ("".into(), ActionLabel(""), false),
                        ("zapsplat".into(), ActionLabel(""), false),
                        ("".into(), ActionLabel(""), false),
                        ("".into(), ActionLabel(""), false),
                        ("Music".into(), ActionLabel(""), false),
                        ("".into(), ActionLabel(""), false),
                        ("Alexander".into(), ActionLabel(""), false),
                        ("".into(), ActionLabel(""), false),
                        ("".into(), ActionLabel(""), false),
                        ("".into(), ActionLabel(""), false),
                        ("".into(), ActionLabel(""), false),
                        ("Nakarada".into(), ActionLabel(""), false),
                        ("".into(), ActionLabel(""), false),
                        ("".into(), ActionLabel(""), false),
                        ("".into(), ActionLabel(""), false),
                        ("Ok".into(), ActionLabel("main menu"), true),
                        ("".into(), ActionLabel(""), false),
                        ("".into(), ActionLabel(""), false),
                    ],
                    cancel_action: Some(ActionLabel("main menu")),
                    header_size: 0.25,
                    width: 5,
                    inactive_color: egui::Color32::from_rgb(255, 255, 255),
                    text_size: 30.0,
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
            if ev.label == ActionLabel(strs[i]) {
                levelset.current_level = i;
                spawn_level.send(SpawnLevelEvent {
                    def: levelset.levels[i].clone(),
                });
            }
        }

        match ev.label.0 {
            "play" => {
                spawn_menu.send(PopupMenuEvent {
                    sender: ev.sender,
                    menu: PopupMenu {
                        heading: "Choose Difficulty".into(),
                        items: vec![
                            ("Easy".into(), ActionLabel("play easy"), true),
                            ("Medium".into(), ActionLabel("play medium"), true),
                            ("Hard".into(), ActionLabel("play hard"), true),
                            ("Daily Mix".into(), ActionLabel("play daily"), true),
                        ],
                        cancel_action: Some(ActionLabel("main menu")),
                        ..Default::default()
                    },
                    sound: false,
                });
                return;
            }
            "play easy" => {
                key = "Easy";
                *levelset = spawn_random(90, 0, "Easy Set".into(), 11, key);
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
            .map(|i| {
                (
                    (i + 1).to_string().into(),
                    ActionLabel(strs[i]),
                    i <= max_level,
                )
            })
            .collect();

        let menu = PopupMenu {
            heading: format!("{}\nSelect Level", levelset.title),
            items,
            cancel_action: Some(ActionLabel("play")),
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
        if ev.label.0 == "pause" {
            debug!(
                "Paused\n[{}/{}/{}]",
                level.num_holes, level.total_blocks, level.seed
            );
            debug!("difficulty: {}", base.0.difficulty());
            spawn.send(PopupMenuEvent {
                sender: ev.sender,
                menu: PopupMenu {
                    heading: format!("Paused ({}/{})\n{}", set.current_level + 1, 30, set.title,),
                    items: vec![
                        ("Resume".into(), ActionLabel("cancel"), true),
                        ("Restart Level".into(), ActionLabel("restart"), true),
                        ("Main Menu".into(), ActionLabel("main menu"), true),
                        (
                            "Quit to Desktop".into(),
                            ActionLabel("quit"),
                            QUIT_TO_DESKTOP,
                        ),
                    ],
                    cancel_action: Some(ActionLabel("cancel")),
                    ..Default::default()
                },
                sound: true,
            })
        }
    }
}

pub fn spawn_popup_menu(
    mut commands: Commands,
    mut other_controllers: Query<(Entity, &mut Controller), Without<MenuMarker>>,
    mut prev_controller_state: Local<HashMap<Entity, bool>>,
    mut spawn_evs: EventReader<PopupMenuEvent>,
    mut actions: ResMut<Events<ActionEvent>>,
    mut action_reader: Local<ManualEventReader<ActionEvent>>,
    mut egui_context: ResMut<EguiContext>,
    menu_items: Query<Entity, With<MenuMarker>>,
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
            if Some(ent) != ev.menu.modal_controller {
                prev_controller_state.insert(ent, controller.enabled);
                controller.enabled = false;
            }
        }

        if ev.menu.modal_controller.is_none() {
            let mut actions = vec![
                (
                    ActionType::MoveUp,
                    Action {
                        label: ActionLabel("up"),
                        sticky: true,
                        display: DisplayMode::Active,
                        display_text: None,
                    },
                ),
                (
                    ActionType::PanUp,
                    Action {
                        label: ActionLabel("up"),
                        sticky: true,
                        display: DisplayMode::Off,
                        display_text: None,
                    },
                ),
                (
                    ActionType::MoveDown,
                    Action {
                        label: ActionLabel("down"),
                        sticky: true,
                        display: DisplayMode::Active,
                        display_text: None,
                    },
                ),
                (
                    ActionType::PanDown,
                    Action {
                        label: ActionLabel("down"),
                        sticky: true,
                        display: DisplayMode::Off,
                        display_text: None,
                    },
                ),
                (
                    ActionType::MainAction,
                    Action {
                        label: ActionLabel("select"),
                        sticky: true,
                        display: DisplayMode::Active,
                        display_text: None,
                    },
                ),
            ];

            if ev.menu.cancel_action.is_some() {
                actions.extend(vec![
                    (
                        ActionType::SecondAction,
                        Action {
                            label: ActionLabel("cancel"),
                            sticky: true,
                            display: DisplayMode::Active,
                            display_text: None,
                        },
                    ),
                    (
                        ActionType::Menu,
                        Action {
                            label: ActionLabel("cancel"),
                            sticky: true,
                            display: DisplayMode::Off,
                            display_text: None,
                        },
                    ),
                ]);
            }

            if ev.menu.width > 1 {
                actions.extend(vec![
                    (
                        ActionType::MoveLeft,
                        Action {
                            label: ActionLabel("left"),
                            sticky: true,
                            display: DisplayMode::Active,
                            display_text: None,
                        },
                    ),
                    (
                        ActionType::PanLeft,
                        Action {
                            label: ActionLabel("left"),
                            sticky: true,
                            display: DisplayMode::Off,
                            display_text: None,
                        },
                    ),
                    (
                        ActionType::MoveRight,
                        Action {
                            label: ActionLabel("right"),
                            sticky: true,
                            display: DisplayMode::Active,
                            display_text: None,
                        },
                    ),
                    (
                        ActionType::PanRight,
                        Action {
                            label: ActionLabel("right"),
                            sticky: true,
                            display: DisplayMode::Off,
                            display_text: None,
                        },
                    ),
                ]);
            }

            commands
                .spawn()
                .insert(Controller {
                    display_order: 4,
                    enabled: true,
                    actions,
                    ..Default::default()
                })
                .insert(Position(IVec2::ZERO))
                .insert(MenuMarker)
                .insert(Permanent);
        }

        debug!("menu");

        *active_menu = Some((ev.menu.clone(), ev.sender));

        if ev.menu.initial_position != -1 {
            *menu_position = ev.menu.initial_position as usize;

            while !ev.menu.items[*menu_position].2 {
                *menu_position += 1;
            }
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
                                    let color = (!enabled).then_some(menu.inactive_color);
                                    let bg = (i == *menu_position).then_some(
                                        egui::Rgba::from_rgba_premultiplied(0.2, 0.2, 0.4, 0.2)
                                            .into(),
                                    );
                                    text.render(ui, menu.text_size, color, bg);
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
                                                    let color =
                                                        (!enabled).then_some(menu.inactive_color);
                                                    let bg = (pos == *menu_position).then_some(
                                                        egui::Rgba::from_rgba_premultiplied(
                                                            0.2, 0.2, 0.4, 0.2,
                                                        )
                                                        .into(),
                                                    );
                                                    text.render(ui, menu.text_size, color, bg);
                                                });
                                            }
                                        });
                                })
                            }
                        }

                        strip.cell(|ui| {
                            let footer = egui::RichText::from(menu.footer.as_str()).size(15.0);
                            ui.with_layout(egui::Layout::bottom_up(egui::Align::Max), |ui| {
                                ui.label(footer)
                            });
                        });
                    });
            });
    }

    let mut to_send = None;

    for ev in action_reader.iter(&actions) {
        if menu_items.get(ev.sender).is_ok() {
            match ev.label.0 {
                "up" | "left" => {
                    let Some(menu) = &active_menu.as_ref() else {
                        continue;
                    };
                    let active_items = &menu.0.items;
                    let width = match ev.label.0 {
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
                    let Some(menu) = &active_menu.as_ref() else {
                        continue;
                    };
                    let active_items = &menu.0.items;
                    let width = match ev.label.0 {
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
                    let Some(menu) = &active_menu.as_ref() else {
                        continue;
                    };
                    let Some(cancel_action) = menu.0.cancel_action else {
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
                        sender: active_menu.as_ref().unwrap().1,
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

                    let Some((menu, sender)) = active_menu.take() else {
                        continue;
                    };

                    for (ent, mut controller) in other_controllers.iter_mut() {
                        if let Some(prev) = prev_controller_state.get(&ent) {
                            controller.enabled = *prev;
                        }
                    }

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

pub fn spawn_options_menu(
    mut commands: Commands,
    mut evs: ResMut<Events<ActionEvent>>,
    mut reader: Local<ManualEventReader<ActionEvent>>,
    mut spawn: EventWriter<PopupMenuEvent>,
    mut settings: ResMut<PkvStore>,
    mut windows: ResMut<Windows>,
    mut control_help: ResMut<ControlHelp>,
    mut cursor_speed: ResMut<CursorSpeed>,
    mut cutter_speed: ResMut<CutSpeed>,
    mut music: ResMut<MusicVolume>,
    mut sfx: ResMut<SfxVolume>,
    mut sender: Local<EntityMarker>,
    mut modal_sender: Local<EntityMarker>,
) {
    if matches!(*sender, EntityMarker::None) {
        *sender = commands.spawn().id().into();
    }

    let slide_controller = |left: &'static str, right: &'static str| -> Controller {
        Controller {
            display_order: 5,
            actions: vec![
                (
                    ActionType::MoveLeft,
                    Action {
                        label: ActionLabel(left),
                        sticky: false,
                        display: DisplayMode::Active,
                        display_text: None,
                    },
                ),
                (
                    ActionType::PanLeft,
                    Action {
                        label: ActionLabel(left),
                        sticky: false,
                        display: DisplayMode::Off,
                        display_text: None,
                    },
                ),
                (
                    ActionType::MoveRight,
                    Action {
                        label: ActionLabel(right),
                        sticky: false,
                        display: DisplayMode::Active,
                        display_text: None,
                    },
                ),
                (
                    ActionType::PanRight,
                    Action {
                        label: ActionLabel(right),
                        sticky: false,
                        display: DisplayMode::Off,
                        display_text: None,
                    },
                ),
                (
                    ActionType::MainAction,
                    Action {
                        label: ActionLabel("done"),
                        sticky: true,
                        display: DisplayMode::Active,
                        display_text: None,
                    },
                ),
                (
                    ActionType::SecondAction,
                    Action {
                        label: ActionLabel("done"),
                        sticky: true,
                        display: DisplayMode::Off,
                        display_text: None,
                    },
                ),
            ],
            enabled: true,
            ..Default::default()
        }
    };

    let mut to_send;
    let mut modal_entity = None;
    let mut keep_position = false;

    while !reader.is_empty(&evs) {
        to_send = None;
        for ev in reader.iter(&evs) {
            // any sender
            match ev.label.0 {
                "options" => {
                    let window_mode = match settings
                        .get::<WindowModeSerial>("window mode")
                        .unwrap_or_default()
                    {
                        WindowModeSerial::Fullscreen => "Full screen",
                        WindowModeSerial::Windowed => "Windowed",
                    };
                    let controls_help = match control_help.0 {
                        true => "Yes",
                        false => "No",
                    };

                    spawn.send(PopupMenuEvent {
                        sender: sender.entity(),
                        menu: PopupMenu {
                            heading: "Options".into(),
                            items: vec![
                                ("Window mode".into(), ActionLabel(""), false),
                                (window_mode.into(), ActionLabel("toggle fullscreen"), true),
                                ("Show controls".into(), ActionLabel(""), false),
                                (
                                    controls_help.into(),
                                    ActionLabel("toggle controls help"),
                                    true,
                                ),
                                ("Music".into(), ActionLabel(""), false),
                                (
                                    MenuItem::Slider((music.0 * 100.0) as i32, 100),
                                    ActionLabel("music"),
                                    true,
                                ),
                                ("FX".into(), ActionLabel(""), false),
                                (
                                    MenuItem::Slider((sfx.0 * 100.0) as i32, 100),
                                    ActionLabel("sfx"),
                                    true,
                                ),
                                ("Cursor Speed".into(), ActionLabel(""), false),
                                (
                                    MenuItem::Slider((cursor_speed.0 * 100.0 / 30.0) as i32, 100),
                                    ActionLabel("cursor speed"),
                                    true,
                                ),
                                ("Cutter Speed".into(), ActionLabel(""), false),
                                (
                                    MenuItem::Slider((cutter_speed.0 * 10.0) as i32, 100),
                                    ActionLabel("cutter speed"),
                                    true,
                                ),
                                ("Controls".into(), ActionLabel(""), false),
                                ("Edit".into(), ActionLabel("controls"), true),
                                ("".into(), ActionLabel(""), false),
                                ("".into(), ActionLabel(""), false),
                                ("".into(), ActionLabel(""), false),
                                ("Ok".into(), ActionLabel("main menu"), true),
                            ],
                            cancel_action: Some(ActionLabel("main menu")),
                            width: 2,
                            initial_position: if keep_position { -1 } else { 0 },
                            inactive_color: egui::Color32::from_rgb(255, 255, 255),
                            text_size: 30.0,
                            modal_controller: modal_entity,
                            ..Default::default()
                        },
                        sound: false,
                    });

                    *modal_sender = modal_entity.into();
                }
                _ => (),
            }

            // just our top-level sender
            if sender.matches(&ev.sender) {
                match ev.label.0 {
                    "toggle fullscreen" => {
                        let new_mode = match settings
                            .get::<WindowModeSerial>("window mode")
                            .unwrap_or_default()
                        {
                            WindowModeSerial::Fullscreen => WindowModeSerial::Windowed,
                            WindowModeSerial::Windowed => WindowModeSerial::Fullscreen,
                        };

                        settings.set("window mode", &new_mode).unwrap();
                        update_window(&*settings, windows.get_primary_mut().unwrap());
                        to_send = Some("options");
                    }
                    "toggle controls help" => {
                        control_help.0 = !control_help.0;
                        settings.set("control help", &control_help.0).unwrap();
                        to_send = Some("options");
                    }
                    "music" => {
                        let controller = commands
                            .spawn()
                            .insert(slide_controller("music down", "music up"))
                            .id();
                        modal_entity = Some(controller);
                        to_send = Some("options");
                    }
                    "sfx" => {
                        let controller = commands
                            .spawn()
                            .insert(slide_controller("fx down", "fx up"))
                            .id();
                        modal_entity = Some(controller);
                        to_send = Some("options");
                    }
                    "cursor speed" => {
                        let controller = commands
                            .spawn()
                            .insert(slide_controller("slower cursor", "faster cursor"))
                            .id();
                        modal_entity = Some(controller);
                        to_send = Some("options");
                    }
                    "cutter speed" => {
                        let controller = commands
                            .spawn()
                            .insert(slide_controller("slower cutter", "faster cutter"))
                            .id();
                        modal_entity = Some(controller);
                        to_send = Some("options");
                    }
                    _ => (),
                }
            }

            // just our modal sender
            if modal_sender.matches(&ev.sender) {
                match ev.label.0 {
                    "music up" => {
                        music.0 = f32::min(1.0, music.0 + 0.01);
                        modal_entity = Some(ev.sender);
                        to_send = Some("options")
                    }
                    "music down" => {
                        music.0 = f32::max(0.0, music.0 - 0.01);
                        modal_entity = Some(ev.sender);
                        to_send = Some("options")
                    }
                    "fx up" => {
                        sfx.0 = f32::min(1.0, sfx.0 + 0.01);
                        modal_entity = Some(ev.sender);
                        to_send = Some("options")
                    }
                    "fx down" => {
                        sfx.0 = f32::max(0.0, sfx.0 - 0.01);
                        modal_entity = Some(ev.sender);
                        to_send = Some("options")
                    }
                    "faster cursor" => {
                        cursor_speed.0 = f32::min(30.0, cursor_speed.0 + 0.3);
                        modal_entity = Some(ev.sender);
                        to_send = Some("options")
                    }
                    "slower cursor" => {
                        cursor_speed.0 = f32::max(1.0, cursor_speed.0 - 0.3);
                        modal_entity = Some(ev.sender);
                        to_send = Some("options")
                    }
                    "faster cutter" => {
                        cutter_speed.0 = f32::min(10.0, cutter_speed.0 + 0.1);
                        modal_entity = Some(ev.sender);
                        to_send = Some("options")
                    }
                    "slower cutter" => {
                        cutter_speed.0 = f32::max(1.0, cutter_speed.0 - 0.1);
                        modal_entity = Some(ev.sender);
                        to_send = Some("options")
                    }
                    "done" => {
                        commands.entity(ev.sender).despawn_recursive();
                        to_send = Some("options");
                    }
                    _ => (),
                }
            }
        }

        if let Some(action) = to_send {
            evs.send(ActionEvent {
                sender: sender.entity(),
                label: ActionLabel(action),
                target: None,
            });
            keep_position = true;
        }
    }
}

#[derive(Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum EntityMarker {
    E(Entity),
    #[default]
    None,
}

impl EntityMarker {
    fn entity(&self) -> Entity {
        match self {
            EntityMarker::E(e) => *e,
            EntityMarker::None => {
                error!("using raw marker");
                Entity::from_raw(0)
            }
        }
    }

    pub fn matches(&self, ent: &Entity) -> bool {
        match self {
            EntityMarker::E(e) => e == ent,
            EntityMarker::None => false,
        }
    }
}

impl From<Entity> for EntityMarker {
    fn from(e: Entity) -> Self {
        EntityMarker::E(e)
    }
}

impl From<Option<Entity>> for EntityMarker {
    fn from(maybe_ent: Option<Entity>) -> Self {
        match maybe_ent {
            Some(e) => EntityMarker::E(e),
            None => EntityMarker::None,
        }
    }
}

pub fn spawn_controls(
    mut commands: Commands,
    mut sender: Local<EntityMarker>,
    mut modal_sender: Local<EntityMarker>,
    mut evs: ResMut<Events<ActionEvent>>,
    mut reader: Local<ManualEventReader<ActionEvent>>,
    mut spawn: EventWriter<PopupMenuEvent>,
    mut actions: ResMut<ActionInputs>,
    mut settings: ResMut<PkvStore>,
    mut current_page: Local<usize>,
    mut selection: Local<Option<ActionType>>,
    mut selected_index: Local<usize>,
    mut new_inputs: EventReader<NewInputEvent>,
) {
    if matches!(*sender, EntityMarker::None) {
        *sender = commands.spawn().id().into();
    }

    let mut to_send;
    let mut modal_entity = None;
    let mut keep_position = true;

    while !reader.is_empty(&evs) {
        to_send = None;
        let mut reset_position = false;
        for ev in reader.iter(&evs) {
            // process regardless of sender
            match ev.label.0 {
                "controls" => {
                    reset_position = true;
                    *selected_index = 0;
                    *selection = None;
                    *current_page = 1;
                    to_send = Some("controls");
                }
                _ => (),
            }

            // only for my events
            if sender.matches(&ev.sender) {
                match ev.label.0 {
                    "controls 1" | "controls 2" => {
                        let mut items = Vec::new();

                        let mut inputs = actions.items.iter().collect::<Vec<_>>();
                        inputs.sort_by_key(|(ty, _)| *ty);

                        for (ty, inputs) in inputs
                            .into_iter()
                            .skip(if ev.label.0 == "controls 1" { 0 } else { 9 })
                            .take(9)
                        {
                            items.push((ty.as_str().into(), ActionLabel(""), false));
                            if Some(*ty) == *selection {
                                items.push((
                                    MenuItem::InputList {
                                        items: inputs.clone(),
                                        focus: Some(*selected_index),
                                    },
                                    ActionLabel(ty.as_str()),
                                    true,
                                ));
                            } else {
                                items.push((
                                    MenuItem::InputList {
                                        items: inputs.clone(),
                                        focus: None,
                                    },
                                    ActionLabel(ty.as_str()),
                                    true,
                                ));
                            }
                        }

                        items.push(("".into(), ActionLabel(""), false));
                        items.push(("".into(), ActionLabel(""), false));

                        items.push(("".into(), ActionLabel(""), false));
                        if ev.label.0 == "controls 1" {
                            items.push(("Next".into(), ActionLabel("controls 2"), true));
                            *current_page = 1;
                        } else {
                            items.push(("Prev".into(), ActionLabel("controls 1"), true));
                            *current_page = 2;
                        }

                        items.push(("Restore Defaults".into(), ActionLabel("defaults"), true));
                        items.push(("Cancel".into(), ActionLabel("cancel"), true));
                        items.push(("".into(), ActionLabel(""), false));
                        items.push(("Done".into(), ActionLabel("done"), true));

                        spawn.send(PopupMenuEvent {
                            sender: sender.entity(),
                            menu: PopupMenu {
                                heading: "Controls".into(),
                                items,
                                cancel_action: Some(ActionLabel("cancel")),
                                width: 2,
                                initial_position: if keep_position { -1 } else { 0 },
                                inactive_color: egui::Color32::from_rgb(255, 255, 255),
                                text_size: 30.0,
                                header_size: 0.25,
                                modal_controller: modal_entity,
                                ..Default::default()
                            },
                            sound: false,
                        });
                    }
                    "defaults" => {
                        *actions = ActionInputs::default();
                        to_send = Some("controls");
                    }
                    "cancel" => {
                        *actions = settings.get::<ActionInputs>("inputs").unwrap();
                        to_send = Some("options");
                    }
                    "done" => {
                        settings.set("inputs", &*actions).unwrap();
                        to_send = Some("options");
                    }
                    _ => (),
                }

                for (ty, _) in actions.items.iter() {
                    if ev.label.0 == ty.as_str() {
                        let modal = commands
                            .spawn()
                            .insert(Controller {
                                display_order: 5,
                                actions: vec![
                                    (
                                        ActionType::MoveLeft,
                                        Action {
                                            label: ActionLabel("control_left"),
                                            sticky: true,
                                            display: DisplayMode::Off,
                                            display_text: None,
                                        },
                                    ),
                                    (
                                        ActionType::PanLeft,
                                        Action {
                                            label: ActionLabel("control_left"),
                                            sticky: true,
                                            display: DisplayMode::Off,
                                            display_text: None,
                                        },
                                    ),
                                    (
                                        ActionType::MoveRight,
                                        Action {
                                            label: ActionLabel("control_right"),
                                            sticky: true,
                                            display: DisplayMode::Off,
                                            display_text: None,
                                        },
                                    ),
                                    (
                                        ActionType::PanRight,
                                        Action {
                                            label: ActionLabel("control_right"),
                                            sticky: true,
                                            display: DisplayMode::Off,
                                            display_text: None,
                                        },
                                    ),
                                    (
                                        ActionType::MainAction,
                                        Action {
                                            label: ActionLabel("control_pick"),
                                            sticky: true,
                                            display: DisplayMode::Active,
                                            display_text: Some("pick"),
                                        },
                                    ),
                                    (
                                        ActionType::SecondAction,
                                        Action {
                                            label: ActionLabel("control_cancel"),
                                            sticky: true,
                                            display: DisplayMode::Active,
                                            display_text: Some("cancel"),
                                        },
                                    ),
                                ],
                                enabled: true,
                                ..Default::default()
                            })
                            .id();
                        *modal_sender = modal.into();
                        modal_entity = Some(modal);
                        *selection = Some(*ty);
                        *selected_index = actions
                            .items
                            .iter()
                            .find(|(typ, _)| ty == *typ)
                            .unwrap()
                            .1
                            .len();
                        to_send = Some("controls");
                    }
                }
            }

            // for my modal controller
            if modal_sender.matches(&ev.sender) {
                match ev.label.0 {
                    "control_left" => {
                        *selected_index = selected_index.saturating_sub(1);
                        to_send = Some("controls");
                        modal_entity = Some(ev.sender);
                    }
                    "control_right" => {
                        *selected_index = usize::min(
                            *selected_index + 1,
                            actions
                                .items
                                .iter()
                                .find(|(typ, _)| *selection == Some(**typ))
                                .unwrap()
                                .1
                                .len(),
                        );
                        to_send = Some("controls");
                        modal_entity = Some(ev.sender);
                    }
                    "control_pick" => {
                        let items = actions
                            .items
                            .iter_mut()
                            .find(|(typ, _)| *selection == Some(**typ))
                            .unwrap()
                            .1;
                        if *selected_index == items.len() {
                            // add new
                            let new_input =
                                commands.spawn().insert(NewInputController::default()).id();
                            modal_entity = Some(new_input);
                            commands.entity(ev.sender).despawn_recursive();
                        } else {
                            items.remove(*selected_index);
                            modal_entity = Some(ev.sender);
                        }
                        to_send = Some("controls");
                    }
                    "control_cancel" => {
                        commands.entity(ev.sender).despawn_recursive();
                        to_send = Some("controls");
                        *selection = None;
                        modal_entity = None;
                    }
                    _ => (),
                }
            }
        }

        if let Some(to_send) = to_send {
            let mut action = to_send;

            if to_send == "controls" {
                action = match *current_page {
                    1 => "controls 1",
                    2 => "controls 2",
                    _ => unreachable!(),
                };
            }
            evs.send(ActionEvent {
                sender: sender.entity(),
                label: ActionLabel(action),
                target: None,
            });
            keep_position = !reset_position;
        }
    }

    for ev in new_inputs.iter() {
        if let Some((_, items)) = actions
            .items
            .iter_mut()
            .find(|(typ, _)| *selection == Some(**typ))
        {
            if !items.contains(&ev.1) {
                items.push(ev.1.clone());
            }
            commands.entity(ev.0).despawn_recursive();
            *selection = None;

            let action = match *current_page {
                1 => "controls 1",
                2 => "controls 2",
                _ => unreachable!(),
            };
            evs.send(ActionEvent {
                sender: sender.entity(),
                label: ActionLabel(action),
                target: None,
            });
        }
    }
}
