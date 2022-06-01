use bevy::prelude::*;

pub struct ActionEvent{
    pub sender: Entity,
    pub label: &'static str,
    pub target: Option<Entity>,
}


#[derive(Component)]
pub struct Position(pub IVec2);

#[derive(Component)]
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

//menus 

#[derive(Clone)]
pub struct PopupMenu {
    pub heading: String,
    pub items: Vec<(String, &'static str)>,
    pub cancel_action: Option<&'static str>,
}

pub struct PopupMenuEvent {
    pub sender: Entity,
    pub menu: PopupMenu,
}

#[derive(Component)]
pub struct MenuItem;

