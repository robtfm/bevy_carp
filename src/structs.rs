use bevy::prelude::*;

#[derive(Clone, Copy, Deref, PartialEq, Eq)]
pub struct ActionLabel(pub &'static str);

pub struct ActionEvent {
    pub sender: Entity,
    pub label: ActionLabel,
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

#[derive(Default, Clone)]
pub struct LevelSet {
    pub levels: [LevelDef; 30],
    pub current_level: usize,
    pub title: String,
    pub settings_key: &'static str,
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
pub struct MusicChannel;

#[derive(Component)]
pub struct Permanent;

#[derive(Default)]
pub struct ChangeBackground;

#[cfg(not(target_arch = "wasm32"))]
pub const QUIT_TO_DESKTOP: bool = true;
#[cfg(target_arch = "wasm32")]
pub const QUIT_TO_DESKTOP: bool = false;

pub struct ControlHelp(pub bool);