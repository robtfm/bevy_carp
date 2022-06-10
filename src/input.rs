use serde::{Deserialize, Serialize};
use std::marker::PhantomData;

use bevy::{
    ecs::system::SystemParam,
    prelude::*,
    utils::{HashMap, HashSet},
};

use bevy_egui::{egui, EguiContext};
use egui_extras::StripBuilder;

use crate::structs::{ActionEvent, Position, PositionZ};

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GamePadRes>()
            .init_resource::<ActionInputs>()
            .init_resource::<LastControlType>()
            .add_event::<ActionEvent>()
            // ui
            .add_system(show_controls)
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

#[derive(Component, Default, Clone)]
pub struct Controller {
    pub display_order: usize,
    pub display_directions: Option<&'static str>,
    pub forward: (&'static str, bool),
    pub back: (&'static str, bool),
    pub left: (&'static str, bool),
    pub right: (&'static str, bool),
    pub up: (&'static str, bool),
    pub down: (&'static str, bool),
    pub action: Vec<(&'static str, (&'static str, bool), bool)>,
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
        Option<&Transform>,
        Option<&mut Position>,
        Option<&mut PositionZ>,
        &mut Controller,
    )>,
    mut action: EventWriter<ActionEvent>,
    mut mapping: ResMut<ActionInputs>,
    mut last_used: ResMut<LastControlType>,
) {
    // Handle key input
    for (ent, maybe_transform, maybe_position, maybe_position_z, mut options) in
        controllers.iter_mut()
    {
        if !options.enabled {
            options.initialized = false;
            continue;
        }

        if let Some(mut position_z) = maybe_position_z {
            if mapping.active(options.forward, &inputs) {
                if let Some(transform) = maybe_transform {
                    position_z.0 = (transform.translation.z - 1.0).ceil() as i32;
                } else {
                    position_z.0 -= 1;
                }
            }
            if mapping.active(options.back, &inputs) {
                if let Some(transform) = maybe_transform {
                    position_z.0 = (transform.translation.z + 1.0).floor() as i32;
                } else {
                    position_z.0 -= 1;
                }
            }
        }

        if let Some(mut position) = maybe_position {
            let translation = match maybe_transform {
                Some(transform) => transform.translation.truncate(),
                None => position.0.as_vec2(),
            };

            if mapping.active(options.left, &inputs) {
                position.0.x = (translation.x - 1.0).ceil() as i32;
            }
            if mapping.active(options.right, &inputs) {
                position.0.x = (translation.x + 1.0).floor() as i32;
            }
            if mapping.active(options.down, &inputs) {
                position.0.y = (translation.y - 1.0).ceil() as i32;
            }
            if mapping.active(options.up, &inputs) {
                position.0.y = (translation.y + 1.0).floor() as i32;
            }
        }

        for &(label, trigger, _) in options.action.iter() {
            if mapping.active(trigger, &inputs) && options.initialized {
                action.send(ActionEvent {
                    sender: ent,
                    label,
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
    items: HashMap<&'static str, Vec<InputItem>>,
    #[serde(skip)]
    prev: HashSet<&'static str>,
    #[serde(skip)]
    last_used: LastControlType,
}

impl Default for ActionInputs {
    fn default() -> Self {
        use InputItem::*;
        Self {
            items: HashMap::from_iter(vec![
                (
                    "menu",
                    vec![Key(KeyCode::Escape), Button(GamepadButtonType::Start)],
                ),
                (
                    "zoom in",
                    vec![
                        Key(KeyCode::PageUp),
                        Button(GamepadButtonType::RightTrigger2),
                    ],
                ),
                (
                    "zoom out",
                    vec![
                        Key(KeyCode::PageDown),
                        Button(GamepadButtonType::LeftTrigger2),
                    ],
                ),
                (
                    "pan left",
                    vec![
                        Key(KeyCode::Left),
                        Axis(GamepadAxisType::RightStickX, false),
                    ],
                ),
                (
                    "pan right",
                    vec![
                        Key(KeyCode::Right),
                        Axis(GamepadAxisType::RightStickX, true),
                    ],
                ),
                (
                    "pan up",
                    vec![Key(KeyCode::Up), Axis(GamepadAxisType::RightStickY, true)],
                ),
                (
                    "pan down",
                    vec![
                        Key(KeyCode::Down),
                        Axis(GamepadAxisType::RightStickY, false),
                    ],
                ),
                (
                    "select all",
                    vec![Key(KeyCode::P), Button(GamepadButtonType::RightThumb)],
                ),
                (
                    "move left",
                    vec![
                        Key(KeyCode::A),
                        Axis(GamepadAxisType::LeftStickX, false),
                        Button(GamepadButtonType::DPadLeft),
                    ],
                ),
                (
                    "move right",
                    vec![
                        Key(KeyCode::D),
                        Axis(GamepadAxisType::LeftStickX, true),
                        Button(GamepadButtonType::DPadRight),
                    ],
                ),
                (
                    "move up",
                    vec![
                        Key(KeyCode::W),
                        Axis(GamepadAxisType::LeftStickY, true),
                        Button(GamepadButtonType::DPadUp),
                    ],
                ),
                (
                    "move down",
                    vec![
                        Key(KeyCode::S),
                        Axis(GamepadAxisType::LeftStickY, false),
                        Button(GamepadButtonType::DPadDown),
                    ],
                ),
                (
                    "main action",
                    vec![
                        Key(KeyCode::Space),
                        Key(KeyCode::Return),
                        Button(GamepadButtonType::South),
                    ],
                ),
                (
                    "second action",
                    vec![Key(KeyCode::LControl), Button(GamepadButtonType::North)],
                ),
                (
                    "third action",
                    vec![Key(KeyCode::Home), Button(GamepadButtonType::West)],
                ),
                (
                    "fourth action",
                    vec![Key(KeyCode::End), Button(GamepadButtonType::East)],
                ),
                (
                    "turn left",
                    vec![Key(KeyCode::Q), Button(GamepadButtonType::LeftTrigger)],
                ),
                (
                    "turn right",
                    vec![Key(KeyCode::E), Button(GamepadButtonType::RightTrigger)],
                ),
            ]),
            prev: Default::default(),
            last_used: Default::default(),
        }
    }
}

impl ActionInputs {
    pub fn active(&mut self, action: (&'static str, bool), inputs: &InputParams) -> bool {
        if !action.1 {
            return self.check_active(action.0, inputs);
        }

        let is_active = self.check_active(action.0, inputs);
        if is_active {
            if !self.prev.contains(action.0) {
                self.prev.insert(action.0);
                return true;
            }
            return false;
        } else {
            self.prev.remove(action.0);
            return false;
        }
    }

    fn check_active(&mut self, action: &'static str, inputs: &InputParams) -> bool {
        let Some(items) = self.items.get(action) else {
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

fn show_controls(
    mut egui_context: ResMut<EguiContext>,
    controllers: Query<&Controller>,
    actions: Res<ActionInputs>,
    last_used: Res<LastControlType>,
) {
    let add = |ui: &mut egui::Ui, item: &'static str, action: String| {
        if let Some(input) = actions.items.get(item).and_then(|v| {
            v.iter().find(|i| {
                matches!(i, InputItem::Key(_)) == (*last_used == LastControlType::Keyboard)
            })
        }) {
            ui.horizontal(|ui| {
                if action != "" {
                    ui.label(format!("{}: ", action));
                }

                input.print(ui);
            });
        }
    };

    let sz_y = egui_extras::Size::exact(23.0);
    let sz_x = egui_extras::Size::exact(30.0);
    egui::Window::new("controls")
        .anchor(egui::Align2::RIGHT_TOP, (-5.0, 5.0))
        .title_bar(false)
        .resizable(false)
        .show(egui_context.ctx_mut(), |ui| {
            ui.set_max_width(100.0);
            let mut enabled_controllers = controllers.iter().filter(|c| c.enabled).collect::<Vec<_>>();
            enabled_controllers.sort_by_key(|c| c.display_order);
            for (i, &controller) in enabled_controllers.iter().enumerate() {
                if let Some(disp) = controller.display_directions {
                    ui.scope(|ui| {
                        ui.style_mut().spacing.item_spacing = egui::vec2(0.0, 0.0);
                        StripBuilder::new(ui).sizes(sz_y, 3).vertical(|mut row| {
                            row.strip(|strip| {
                                strip.sizes(sz_x, 3).horizontal(|mut col| {
                                    col.empty();
                                    col.cell(|ui| {
                                        add(ui, controller.up.0, "".into());
                                    });
                                    col.empty();
                                });
                            });
                            row.strip(|strip| {
                                strip.sizes(sz_x, 3).horizontal(|mut col| {
                                    col.cell(|ui| {
                                        add(ui, controller.left.0, "".into());
                                    });
                                    col.cell(|ui| {
                                        ui.horizontal_centered(|ui| {
                                            ui.vertical_centered(|ui| {
                                                ui.label(disp);
                                            });
                                        });
                                    });
                                    col.cell(|ui| {
                                        add(ui, controller.right.0, "".into());
                                    });
                                });
                            });
                            row.strip(|strip| {
                                strip.sizes(sz_x, 3).horizontal(|mut col| {
                                    col.empty();
                                    col.cell(|ui| {
                                        add(ui, controller.down.0, "".into());
                                    });
                                    col.empty();
                                });
                            });
                        });
                    });

                    if controller.back.0 != "" {
                        add(ui, controller.back.0, "zoom out".into());
                    }
                    if controller.forward.0 != "" {
                        add(ui, controller.forward.0, "zoom in".into());
                    }
                }

                for (action, (item, _), display) in controller.action.iter() {
                    if *display {
                        add(ui, item, action.to_string());
                    }
                }

                if i < enabled_controllers.len() - 1 {
                    ui.add(egui::Separator::default().horizontal().spacing(25.0));
                }
            }
        });
}
