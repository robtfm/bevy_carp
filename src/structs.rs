use bevy::prelude::*;

pub struct ActionEvent {
    pub sender: Entity,
    pub label: &'static str,
    pub target: Option<Entity>,
}

#[derive(Component, Clone, Copy, Default, PartialEq, Eq)]
pub struct Position(pub IVec2);

#[derive(Component, Clone, Copy, Default)]
pub struct PositionZ(pub i32);

#[derive(Clone, Default)]
pub struct LevelDef {
    pub num_holes: usize,
    pub total_blocks: usize,
    pub seed: u64,
}

pub struct SpawnLevelEvent {
    pub def: LevelDef,
}

// audio channels

pub struct MenuChannel;
pub struct GrabDropChannel;
pub struct HammerChannel;
pub struct SwooshChannel;
pub struct CutChannel;
pub struct UndoChannel;
//menus

#[derive(Clone)]
pub struct PopupMenu {
    pub heading: String,
    pub items: Vec<(String, &'static str, bool)>,
    pub cancel_action: Option<&'static str>,
    pub transparent: bool,
    pub header_size: f32,
    pub width: usize,
}

pub struct PopupMenuEvent {
    pub sender: Entity,
    pub menu: PopupMenu,
    pub sound: bool,
}

#[derive(Component)]
pub struct MenuItem;

#[derive(Component)]
pub struct Permanent;

#[derive(Default)]
pub struct ChangeBackground;
