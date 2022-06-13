use serde::{Deserialize, Serialize};
use std::marker::PhantomData;

use bevy::{
    ecs::system::SystemParam,
    prelude::*,
    utils::{HashMap, HashSet},
};

use bevy_egui::{egui, EguiContext};
use egui_extras::StripBuilder;

use crate::{structs::{ActionEvent, LevelDef, ActionLabel}, LevelSet};

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GamePadRes>()
            .init_resource::<ActionInputs>()
            .init_resource::<LastControlType>()
            .add_event::<ActionEvent>()
            // ui
            .add_system(show_controls)
            .add_system(show_status)
            // input
            .add_system(pad_connection)
            .add_system_to_stage(CoreStage::PreUpdate, controller);
    }
}

#[derive(Default)]
pub struct GamePadRes(pub Option<Gamepad>);

fn pad_connection(mut pad: ResMut<GamePadRes>, mut gamepad_event: EventReader<GamepadEvent>) {
    for event in gamepad_event.iter() {
        match &event {
            /* 0.8
            GamepadEvent {
                gamepad,
                event_type: GamepadEventType::Connected,
            } => {
                */
            GamepadEvent(
                gamepad,
                GamepadEventType::Connected,
            ) => {
                pad.0 = Some(*gamepad);
                debug!("C");
            }
            /* 0.8
            GamepadEvent {
                gamepad,
                event_type: GamepadEventType::Disconnected,
            } => {
                 */
            GamepadEvent(
                gamepad,
                GamepadEventType::Disconnected,
            ) => {
                if let Some(cur_pad) = pad.0 {
                    if &cur_pad == gamepad {
                        pad.0 = None;
                        debug!("DC");
                    }
                }
            }
            _ => (),
        }
    }
}

#[derive(Clone, Copy)]
pub struct Action {
    pub label: ActionLabel,
    pub sticky: bool,
    pub display: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ActionType {
    Menu,
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    PanUp,
    PanDown,
    PanLeft,
    PanRight,
    PanFocus,
    MainAction,
    SecondAction,
    ThirdAction,
    FourthAction,
    ZoomIn,
    ZoomOut,
    TurnLeft,
    TurnRight,

}

#[derive(Clone)]
pub struct DisplayDirections {
    pub label: String,
    pub up: ActionType,
    pub down: ActionType,
    pub left: ActionType,
    pub right: ActionType,
}

#[derive(Component, Default, Clone)]
pub struct Controller {
    pub display_order: usize,
    pub display_directions: Option<DisplayDirections>,
    pub actions: Vec<(ActionType, Action)>,
    pub enabled: bool,
    pub initialized: bool,
}

#[derive(SystemParam)]
pub struct InputParams<'w, 's> {
    key_input: Res<'w, Input<KeyCode>>,
    pad: Res<'w, GamePadRes>,
    axes: Res<'w, Axis<GamepadAxis>>,
    buttons: Res<'w, Axis<GamepadButton>>,

    #[system_param(ignore)]
    _marker: PhantomData<(&'w (), &'s ())>,
}

fn controller(
    inputs: InputParams,
    mut controllers: Query<(
        Entity,
        &mut Controller,
    )>,
    mut actions: EventWriter<ActionEvent>,
    mut mapping: ResMut<ActionInputs>,
    mut last_used: ResMut<LastControlType>,
) {
    // Handle key input
    for (ent, mut options) in
        controllers.iter_mut()
    {
        if !options.enabled {
            options.initialized = false;
            continue;
        }

        for &(trigger, action) in options.actions.iter() {
            if mapping.active(trigger, action.sticky, &inputs) && options.initialized {
                actions.send(ActionEvent {
                    sender: ent,
                    label: action.label,
                    target: None,
                });
            }
        }

        options.initialized = true;
        *last_used = mapping.last_used;
    }
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum LastControlType {
    #[default]
    Keyboard,
    Gamepad,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum InputItem {
    Key(KeyCode),
    Axis(GamepadAxisType, bool),
    Button(GamepadButtonType),
}

fn key_text(k: &KeyCode) -> String {
    match k {
        KeyCode::Escape => "ESC".into(),
        KeyCode::Insert => "Ins".into(),
        KeyCode::Delete => "Del".into(),
        KeyCode::Left => "⬅".into(),
        KeyCode::Up => "⬆".into(),
        KeyCode::Right => "➡".into(),
        KeyCode::Down => "⬇".into(),
        KeyCode::Return => "Enter".into(),
        _ => format!("{:?}", k),
    }
}

impl InputItem {
    pub fn print(&self, ui: &mut egui::Ui) {
        fn draw_key(text: String, ui: &mut egui::Ui) {
            let galley = egui::WidgetText::RichText(text.into()).into_galley(
                ui,
                Some(false),
                0.0,
                egui::FontSelection::Default,
            );
            let req_size = (galley.size() + egui::vec2(20.0, 10.0)).max(egui::vec2(35.0, 10.0));

            let (response, painter) = ui.allocate_painter(req_size, egui::Sense::hover());
            let rect = response.rect;
            let tl = rect.left_top();
            let w = rect.width();
            let h = rect.height();
            let color = egui::Color32::from_gray(255);
            let stroke = egui::Stroke::new(1.0, color);
            painter.line_segment(
                [tl + egui::vec2(8.0, 1.0), tl + egui::vec2(w - 8.0, 1.0)],
                stroke,
            );
            painter.line_segment(
                [
                    tl + egui::vec2(w - 8.0, 1.0),
                    tl + egui::vec2(w - 5.0, h - 1.0),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    tl + egui::vec2(w - 5.0, h - 1.0),
                    tl + egui::vec2(5.0, h - 1.0),
                ],
                stroke,
            );
            painter.line_segment(
                [tl + egui::vec2(5.0, h - 1.0), tl + egui::vec2(8.0, 1.0)],
                stroke,
            );
            painter.add(egui::epaint::TextShape {
                pos: rect.left_top() + egui::vec2((req_size.x - galley.size().x) / 2.0, 5.0),
                galley: galley.galley,
                override_text_color: Some(color),
                underline: egui::Stroke::none(),
                angle: 0.0,
            });
        }

        fn draw_thumb(text: &str, color: egui::Color32, ui: &mut egui::Ui) -> egui::Painter {
            let galley = egui::WidgetText::RichText(text.into()).into_galley(
                ui,
                Some(false),
                0.0,
                egui::FontSelection::Default,
            );
            let req_size = galley.size() + egui::vec2(16.0, 10.0);

            let (response, painter) = ui.allocate_painter(req_size, egui::Sense::hover());
            let stroke = egui::Stroke::new(1.0, color);

            painter.circle_stroke(
                response.rect.center(),
                f32::max(galley.size().x, galley.size().y) * 0.55,
                stroke,
            );

            painter.add(egui::epaint::TextShape {
                pos: response.rect.left_top() + egui::vec2(8.0, 4.0),
                galley: galley.galley,
                override_text_color: Some(color),
                underline: egui::Stroke::none(),
                angle: 0.0,
            });

            painter
        }

        fn draw_arrow(
            painter: &egui::Painter,
            point: egui::Pos2,
            base_offset: egui::Vec2,
            color: egui::Color32,
        ) {
            let stroke = egui::Stroke::new(1.0, color);
            let norm = egui::vec2(base_offset.y, -base_offset.x);
            painter.line_segment([point, point + base_offset + norm], stroke);
            painter.line_segment([point, point + base_offset - norm], stroke);
        }

        fn draw_buttons(square: bool, hilight: usize, color: egui::Color32, ui: &mut egui::Ui) {
            let step = 8.0;
            let space = 1.0;
            let size = 3.0 * step + 4.0 * space;
            let stroke = egui::Stroke::new(1.0, color);
            let (response, painter) =
                ui.allocate_painter(egui::vec2(size + 4.0, size + 4.0), egui::Sense::hover());
            let tl = response.rect.left_top();
            for (i, origin) in [
                (space * 2.0 + step * 1.5, space * 1.0 + step * 0.5),
                (space * 3.0 + step * 2.5, space * 2.0 + step * 1.5),
                (space * 2.0 + step * 1.5, space * 3.0 + step * 2.5),
                (space * 1.0 + step * 0.5, space * 2.0 + step * 1.5),
            ]
            .into_iter()
            .enumerate()
            {
                if square {
                    if i == hilight {
                        painter.rect_filled(
                            egui::Rect::from_center_size(
                                tl + origin.into(),
                                egui::vec2(step / 2.0, step / 2.0),
                            ),
                            egui::Rounding::none(),
                            color,
                        );
                    } else {
                        painter.rect_stroke(
                            egui::Rect::from_center_size(
                                tl + origin.into(),
                                egui::vec2(step / 2.0, step / 2.0),
                            ),
                            egui::Rounding::none(),
                            stroke,
                        );
                    }
                } else {
                    if i == hilight {
                        painter.circle_filled(tl + origin.into(), step / 2.0, color);
                    } else {
                        painter.circle_stroke(tl + origin.into(), step / 2.0, stroke);
                    }
                }
            }
        }

        match self {
            InputItem::Key(k) => {
                let text = key_text(k);
                draw_key(text, ui);
            }
            InputItem::Axis(x, right) => {
                let (text, horiz) = match x {
                    GamepadAxisType::LeftStickX => ("L", true),
                    GamepadAxisType::LeftStickY => ("L", false),
                    GamepadAxisType::RightStickX => ("R", true),
                    GamepadAxisType::RightStickY => ("R", false),
                    _ => ("?", false),
                };

                let color = egui::Color32::from_rgb(255, 255, 255);
                let painter = draw_thumb(text, color, ui);
                let clip_rect = painter.clip_rect();
                let req_size = clip_rect.size();

                let (arrow_mid, offset) = match (horiz, right) {
                    (true, true) => (
                        egui::vec2(req_size.x - 1.0, req_size.y / 2.0),
                        egui::vec2(-3.0, 0.0),
                    ),
                    (true, false) => (egui::vec2(1.0, req_size.y / 2.0), egui::vec2(3.0, 0.0)),
                    (false, false) => (
                        egui::vec2(req_size.x / 2.0, req_size.y),
                        egui::vec2(0.0, -3.0),
                    ),
                    (false, true) => (egui::vec2(req_size.x / 2.0, 1.0), egui::vec2(0.0, 3.0)),
                };

                draw_arrow(&painter, clip_rect.left_top() + arrow_mid, offset, color);
            }
            InputItem::Button(b) => {
                let color = egui::Color32::from_rgb(255, 255, 255);
                match b {
                    GamepadButtonType::LeftTrigger => draw_key("L1".into(), ui),
                    GamepadButtonType::LeftTrigger2 => draw_key("L2".into(), ui),
                    GamepadButtonType::RightTrigger => draw_key("R1".into(), ui),
                    GamepadButtonType::RightTrigger2 => draw_key("R2".into(), ui),
                    GamepadButtonType::LeftThumb | GamepadButtonType::RightThumb => {
                        let text = match b {
                            GamepadButtonType::LeftThumb => "L",
                            GamepadButtonType::RightThumb => "R",
                            _ => unreachable!(),
                        };
                        let painter = draw_thumb(text, color, ui);
                        let rect = painter.clip_rect();
                        draw_arrow(
                            &painter,
                            rect.left_top() + egui::vec2(3.0, 3.0),
                            egui::vec2(-2.0, -2.0),
                            color,
                        );
                        draw_arrow(
                            &painter,
                            rect.right_top() + egui::vec2(-3.0, 3.0),
                            egui::vec2(2.0, -2.0),
                            color,
                        );
                        draw_arrow(
                            &painter,
                            rect.left_bottom() + egui::vec2(3.0, -3.0),
                            egui::vec2(-2.0, 2.0),
                            color,
                        );
                        draw_arrow(
                            &painter,
                            rect.right_bottom() + egui::vec2(-3.0, -3.0),
                            egui::vec2(2.0, 2.0),
                            color,
                        );
                    }
                    GamepadButtonType::South => draw_buttons(false, 2, color, ui),
                    GamepadButtonType::East => draw_buttons(false, 1, color, ui),
                    GamepadButtonType::North => draw_buttons(false, 0, color, ui),
                    GamepadButtonType::West => draw_buttons(false, 3, color, ui),
                    GamepadButtonType::DPadUp => draw_buttons(true, 2, color, ui),
                    GamepadButtonType::DPadRight => draw_buttons(true, 1, color, ui),
                    GamepadButtonType::DPadDown => draw_buttons(true, 0, color, ui),
                    GamepadButtonType::DPadLeft => draw_buttons(true, 3, color, ui),
                    b => draw_key(format!("{:?}", b), ui),
                    // GamepadButtonType::C => todo!(),
                    // GamepadButtonType::Z => todo!(),
                    // GamepadButtonType::Select => todo!(),
                    // GamepadButtonType::Start => todo!(),
                    // GamepadButtonType::Mode => todo!(),
                }
                // ui.label(format!("{:?}", b));
            }
        };
    }
}

#[derive(Serialize, Deserialize)]
#[serde(bound(deserialize = "'de: 'static"))]
pub struct ActionInputs {
    items: HashMap<ActionType, Vec<InputItem>>,
    #[serde(skip)]
    prev: HashSet<ActionType>,
    #[serde(skip)]
    last_used: LastControlType,
}

impl Default for ActionInputs {
    fn default() -> Self {
        use InputItem::*;
        use ActionType::*;
        Self {
            items: HashMap::from_iter(vec![
                (
                    Menu,
                    vec![Key(KeyCode::Escape), Button(GamepadButtonType::Start)],
                ),
                (
                    ZoomIn,
                    vec![
                        Key(KeyCode::PageUp),
                        Button(GamepadButtonType::RightTrigger2),
                    ],
                ),
                (
                    ZoomOut,
                    vec![
                        Key(KeyCode::PageDown),
                        Button(GamepadButtonType::LeftTrigger2),
                    ],
                ),
                (
                    PanLeft,
                    vec![
                        Key(KeyCode::Left),
                        Axis(GamepadAxisType::RightStickX, false),
                    ],
                ),
                (
                    PanRight,
                    vec![
                        Key(KeyCode::Right),
                        Axis(GamepadAxisType::RightStickX, true),
                    ],
                ),
                (
                    PanUp,
                    vec![Key(KeyCode::Up), Axis(GamepadAxisType::RightStickY, true)],
                ),
                (
                    PanDown,
                    vec![
                        Key(KeyCode::Down),
                        Axis(GamepadAxisType::RightStickY, false),
                    ],
                ),
                (
                    PanFocus,
                    vec![Key(KeyCode::P), Button(GamepadButtonType::RightThumb)],
                ),
                (
                    MoveLeft,
                    vec![
                        Key(KeyCode::A),
                        Axis(GamepadAxisType::LeftStickX, false),
                        Button(GamepadButtonType::DPadLeft),
                    ],
                ),
                (
                    MoveRight,
                    vec![
                        Key(KeyCode::D),
                        Axis(GamepadAxisType::LeftStickX, true),
                        Button(GamepadButtonType::DPadRight),
                    ],
                ),
                (
                    MoveUp,
                    vec![
                        Key(KeyCode::W),
                        Axis(GamepadAxisType::LeftStickY, true),
                        Button(GamepadButtonType::DPadUp),
                    ],
                ),
                (
                    MoveDown,
                    vec![
                        Key(KeyCode::S),
                        Axis(GamepadAxisType::LeftStickY, false),
                        Button(GamepadButtonType::DPadDown),
                    ],
                ),
                (
                    MainAction,
                    vec![
                        Key(KeyCode::Space),
                        Key(KeyCode::Return),
                        Button(GamepadButtonType::South),
                    ],
                ),
                (
                    SecondAction,
                    vec![Key(KeyCode::LControl), Button(GamepadButtonType::North)],
                ),
                (
                    ThirdAction,
                    vec![Key(KeyCode::Home), Button(GamepadButtonType::West)],
                ),
                (
                    FourthAction,
                    vec![Key(KeyCode::End), Button(GamepadButtonType::East)],
                ),
                (
                    TurnLeft,
                    vec![Key(KeyCode::Q), Button(GamepadButtonType::LeftTrigger)],
                ),
                (
                    TurnRight,
                    vec![Key(KeyCode::E), Button(GamepadButtonType::RightTrigger)],
                ),
            ]),
            prev: Default::default(),
            last_used: Default::default(),
        }
    }
}

impl ActionInputs {
    pub fn active(&mut self, action: ActionType, sticky: bool, inputs: &InputParams) -> bool {
        if !sticky {
            return self.check_active(action, inputs);
        }

        let is_active = self.check_active(action, inputs);
        if is_active {
            if !self.prev.contains(&action) {
                self.prev.insert(action);
                return true;
            }
            return false;
        } else {
            self.prev.remove(&action);
            return false;
        }
    }

    fn check_active(&mut self, action: ActionType, inputs: &InputParams) -> bool {
        let Some(items) = self.items.get(&action) else {
            return false;
        };

        for item in items.iter() {
            match item {
                InputItem::Key(key) => {
                    if inputs.key_input.pressed(*key) {
                        self.last_used = LastControlType::Keyboard;
                        return true;
                    }
                }
                InputItem::Axis(axis_type, right) => {
                    if let Some(gamepad) = inputs.pad.0 {
                        let axis = inputs
                            .axes
                            /* 0.8
                            .get(GamepadAxis {
                                gamepad,
                                axis_type: *axis_type,
                            })  */
                            .get(GamepadAxis(
                                gamepad,
                                *axis_type,
                            ))
                            .unwrap();
                        if axis > 0.5 && *right {
                            self.last_used = LastControlType::Gamepad;
                            return true;
                        }
                        if axis < -0.5 && !*right {
                            self.last_used = LastControlType::Gamepad;
                            return true;
                        }
                    }
                }
                InputItem::Button(button_type) => {
                    if let Some(gamepad) = inputs.pad.0 {
                        let button = inputs
                            .buttons
                            /* 0.8
                            .get(GamepadButton {
                                gamepad,
                                button_type: *button_type,
                            })
                             */
                            .get(GamepadButton(
                                gamepad,
                                *button_type,
                            ))
                            .unwrap();
                        if button > 0.5 {
                            self.last_used = LastControlType::Gamepad;
                            return true;
                        }
                    }
                }
            }
        }

        return false;
    }
}

fn show_action(actions: &ActionInputs, ui: &mut egui::Ui, item: ActionType, action: Option<ActionLabel>, prefer_keyboard: bool) {
    if let Some(input) = actions.items.get(&item).and_then(|v| {
        v.iter().find(|i| {
            matches!(i, InputItem::Key(_)) == prefer_keyboard
        })
    }) {
        ui.horizontal(|ui| {
            if let Some(action) = action {
                ui.label(format!("{}: ", action.0));
            }

            input.print(ui);
        });
    }
}

fn show_directions(actions: &ActionInputs, directions: &DisplayDirections, ui: &mut egui::Ui, prefer_keyboard: bool) {
    ui.style_mut().spacing.item_spacing = egui::vec2(0.0, 0.0);
    let sz_y = egui_extras::Size::exact(23.0);
    let sz_x = egui_extras::Size::exact(30.0);
    StripBuilder::new(ui).sizes(sz_y, 3).vertical(|mut row| {
        row.strip(|strip| {
            strip.sizes(sz_x, 3).horizontal(|mut col| {
                col.empty();
                col.cell(|ui| {
                    show_action(actions, ui, directions.up, None, prefer_keyboard);
                });
                col.empty();
            });
        });
        row.strip(|strip| {
            strip.sizes(sz_x, 3).horizontal(|mut col| {
                col.cell(|ui| {
                    show_action(actions, ui, directions.left, None, prefer_keyboard);
                });
                col.cell(|ui| {
                    ui.horizontal_centered(|ui| {
                        ui.vertical_centered(|ui| {
                            ui.label(&directions.label);
                        });
                    });
                });
                col.cell(|ui| {
                    show_action(actions, ui, directions.right, None, prefer_keyboard);
                });
            });
        });
        row.strip(|strip| {
            strip.sizes(sz_x, 3).horizontal(|mut col| {
                col.empty();
                col.cell(|ui| {
                    show_action(actions, ui, directions.down, None, prefer_keyboard);
                });
                col.empty();
            });
        });
    });
}

fn show_controls(
    mut egui_context: ResMut<EguiContext>,
    controllers: Query<&Controller>,
    actions: Res<ActionInputs>,
    last_used: Res<LastControlType>,
) {
    let prefer_keyboard = *last_used == LastControlType::Keyboard;

    egui::Window::new("controls")
        .anchor(egui::Align2::RIGHT_TOP, (-5.0, 5.0))
        .title_bar(false)
        .resizable(false)
        .show(egui_context.ctx_mut(), |ui| {
            ui.set_max_width(100.0);
            let mut enabled_controllers = controllers.iter().filter(|c| c.enabled).collect::<Vec<_>>();
            enabled_controllers.sort_by_key(|c| c.display_order);
            for (i, &controller) in enabled_controllers.iter().enumerate() {
                if let Some(directions) = &controller.display_directions {
                    ui.scope(|ui| {
                        show_directions(&*actions, directions, ui, prefer_keyboard);
                    });
                }

                for (action_type, action) in controller.actions.iter() {
                    if action.display {
                        show_action(&*actions, ui, *action_type, Some(action.label), prefer_keyboard);
                    }
                }

                if i < enabled_controllers.len() - 1 {
                    ui.add(egui::Separator::default().horizontal().spacing(25.0));
                }
            }
        });
}

fn show_status(
    mut egui_context: ResMut<EguiContext>,
    set: Res<LevelSet>,
    def: Res<LevelDef>,
) {
    if def.num_holes != 0 {
        egui::Window::new("status")
        .title_bar(false)
        .resizable(false)
        .show(egui_context.ctx_mut(), |ui| {
            ui.set_max_width(100.0);
            ui.label(&set.title);
            ui.label(format!("{}/{}", set.current_level+1, 30));
        });
    }
}