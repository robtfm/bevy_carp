use bevy::prelude::*;
use bevy::utils::HashSet;
use rand::{
    prelude::{SliceRandom, StdRng},
    Rng,
};

use crate::structs::{Position, PositionZ};

pub fn neighbours() -> [IVec2; 4] {
    [IVec2::X, IVec2::Y, -IVec2::X, -IVec2::Y]
}

#[derive(Default, Clone)]
pub struct Level {
    pub extents: IVec2,
    pub holes: Holes,
    pub planks: Vec<(Plank, Position)>,
    pub setup: bool,
}

impl Level {
    pub fn difficulty(&self) -> f32 {
        // let hole_count = self.holes.holes.len() as f32;
        let plank = &self.planks[0].0;
        let area = plank.size().x * plank.size().y;
        let density = plank.coords.len() as f32 / area as f32;
        let hole_difficulty = self.holes.holes.iter().fold(1.0, |sum, hole| {
            let hole_difficulty = f32::sqrt(1.0 / f32::max(4.0, hole.coords.len() as f32));
            sum + hole_difficulty
        });

        println!("hole score {} * (1 + density {})", hole_difficulty, density);
        return hole_difficulty * (1.0 + density);
    }
}

// holds the built level in initial state. probably not necessary with reproduceable seeded builds, could just use LevelDef
#[derive(Default)]
pub struct LevelBase(pub Level);

pub struct UndoState {
    pub is_action: bool,
    pub level: Level,
    pub done_planks: Vec<(Plank, Position, Vec<IVec2>)>,
    pub cursor: Position,
    pub camera: (Position, PositionZ),
}

pub struct UndoBuffer {
    states: Vec<UndoState>,
    pos: usize,
}

impl Default for UndoBuffer {
    fn default() -> Self {
        Self::invalid()
    }
}

impl UndoBuffer {
    pub fn invalid() -> Self {
        Self {
            states: Vec::new(),
            pos: usize::MAX,
        }
    }

    pub fn new(level: Level) -> Self {
        Self {
            states: vec![UndoState {
                is_action: false,
                level,
                done_planks: Vec::new(),
                cursor: Position::default(),
                camera: (Position::default(), PositionZ::default()),
            }],
            pos: 0,
        }
    }

    pub fn push_state(
        &mut self,
        is_action: bool,
        level: Level,
        done_planks: Vec<(Plank, Position, Vec<IVec2>)>,
        cursor: Position,
        camera: (Position, PositionZ),
    ) {
        self.states.truncate(self.pos + 1);
        self.states.push(UndoState {
            is_action,
            level,
            done_planks,
            cursor,
            camera,
        });
        self.pos = self.states.len() - 1;
    }

    fn get_state(&self, dir: i32) -> &UndoState {
        &self.states[(self.pos as i32 + dir) as usize]
    }

    pub fn current_state(&self) -> &UndoState {
        self.get_state(0)
    }

    pub fn prev(&self) -> Option<&UndoState> {
        if !self.has_back() {
            return None;
        }

        Some(self.get_state(-1))
    }

    pub fn next(&self) -> Option<&UndoState> {
        if !self.has_forward() {
            return None;
        }

        Some(self.get_state(1))
    }

    pub fn has_back(&self) -> bool {
        self.pos > 0
    }

    pub fn has_forward(&self) -> bool {
        self.pos < self.states.len() - 1
    }

    pub fn move_back(&mut self) {
        if self.has_back() {
            self.pos -= 1;
        }
    }

    pub fn move_forward(&mut self) {
        if self.has_forward() {
            self.pos += 1;
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct CoordSet {
    pub coords: HashSet<IVec2>,
    pub turns: usize,
    pub texture_offset: IVec2,
}

impl CoordSet {
    pub fn extents(&self) -> ((i32, i32), (i32, i32)) {
        let mut coords = self.coords.iter().copied();
        let Some(first) = coords.next() else {
            return ((0,0), (0,0))
        };
        coords.fold(((first.x, first.x), (first.y, first.y)), |(x, y), b| {
            ((x.0.min(b.x), x.1.max(b.x)), (y.0.min(b.y), y.1.max(b.y)))
        })
    }

    pub fn size(&self) -> IVec2 {
        let (x, y) = self.extents();
        IVec2::new(x.1 - x.0 + 1, y.1 - y.0 + 1)
    }

    pub fn count(&self) -> usize {
        self.coords.len()
    }

    pub fn contains(&self, xy: IVec2) -> bool {
        self.coords.contains(&xy)
    }
    pub fn contains_xy(&self, x: i32, y: i32) -> bool {
        self.coords.contains(&IVec2::new(x, y))
    }

    pub fn touches(&self, other: &CoordSet) -> bool {
        self.coords
            .iter()
            .any(|c1| neighbours().iter().any(|n| other.contains(*c1 + *n)))
    }

    pub fn overlaps(&self, other: &CoordSet) -> bool {
        self.coords.iter().any(|c1| other.contains(*c1))
    }

    pub fn equals(&self, other: &CoordSet) -> bool {
        self.coords == other.coords
    }

    pub fn rotate(&mut self) {
        self.coords = HashSet::from_iter(self.coords.drain().map(|c| IVec2::new(-c.y, c.x)));
        self.turns = (self.turns + 1) % 4;
    }

    pub fn normalize(mut self) -> Self {
        let exts = self.extents();
        self.coords = HashSet::from_iter(
            self.coords
                .drain()
                .map(|c| c - IVec2::new(exts.0 .0, exts.1 .0)),
        );
        self.turns = 0;
        self
    }

    pub fn shift(&mut self, mut by: IVec2) {
        self.coords = HashSet::from_iter(self.coords.drain().map(|c| c + by));

        for _ in 0..self.turns {
            by = IVec2::new(by.y, -by.x);
        }
        self.texture_offset -= by;
    }

    pub fn merge<'a>(holes: impl Iterator<Item = &'a CoordSet>) -> Hole {
        let mut coords = HashSet::default();
        for hole in holes {
            coords.extend(hole.coords.iter());
        }

        Hole {
            coords,
            ..Default::default()
        }
    }
}

impl std::fmt::Display for CoordSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let exts = self.extents();
        f.write_str(format!("[{},{}]", exts.0 .0, exts.0 .1).as_str())?;
        f.write_str("|")?;
        for _ in exts.0 .0..=exts.0 .1 {
            f.write_str("-")?;
        }
        f.write_str("|\n")?;
        for row in exts.1 .0..=exts.1 .1 {
            f.write_str("|")?;
            for col in exts.0 .0..=exts.0 .1 {
                if self.contains_xy(col, row) {
                    f.write_str("#")?;
                } else {
                    f.write_str(" ")?;
                }
            }
            f.write_str("|\n")?;
        }
        f.write_str("|")?;
        for _ in exts.0 .0..=exts.0 .1 {
            f.write_str("-")?;
        }
        f.write_str("|\n")?;

        Ok(())
    }
}

pub type Hole = CoordSet;
pub type Plank = CoordSet;

#[derive(Default, Clone)]
pub struct Holes {
    pub holes: Vec<Hole>,
}

impl Plank {
    pub fn from_holes(holes: &Holes, mut rng: &mut StdRng) -> Self {
        let mut indexes = (0..holes.holes.len()).collect::<Vec<_>>();
        indexes.shuffle(&mut rng);

        let mut plank = Plank {
            coords: holes.holes[indexes[0]].coords.clone(),
            ..Default::default()
        };

        for i in 1..indexes.len() {
            plank = plank.attach_hole(&holes.holes[indexes[i]], &mut rng);
        }

        for _ in 0..rng.gen_range(0..4) {
            plank.rotate();
        }

        plank.texture_offset = IVec2::new(rng.gen_range(0..1000), rng.gen_range(0..1000));
        plank.normalize()
    }

    fn attach_hole(mut self, hole: &Hole, rng: &mut StdRng) -> Self {
        let mut hole = hole.clone();
        for _ in 0..rng.gen_range(0..4) {
            hole.rotate();
        }

        for _ in 0..rng.gen_range(0..4) {
            self.rotate();
        }

        let y_shift_range = (self.extents().1 .0 - hole.extents().1 .1)
            ..=(self.extents().1 .1 - hole.extents().1 .0);
        let y_shift = rng.gen_range(y_shift_range.clone());

        hole.shift(IVec2::new(
            self.extents().0 .0 - hole.extents().0 .1 - 1,
            y_shift,
        ));

        let mut possible = Vec::new();
        loop {
            if self.touches(&hole) {
                possible.push(hole.clone());
            }
            hole.shift(IVec2::X);
            if self.overlaps(&hole) {
                break;
            }
        }

        // possible.shuffle(rng);
        let mut hole = possible.pop().unwrap();

        self.coords.extend(hole.coords.drain());
        self
    }
}

pub fn gen_hole(size: usize, rng: &mut StdRng) -> Hole {
    let mut hole = Hole {
        coords: HashSet::from_iter(std::iter::once(IVec2::ZERO)),
        turns: 0,
        texture_offset: IVec2::new(rng.gen_range(0..1000), rng.gen_range(0..1000)),
    };

    for _ in 1..size {
        let extents = hole.extents();

        loop {
            let next = IVec2::new(
                rng.gen_range(extents.0 .0 - 1..=extents.0 .1 + 1),
                rng.gen_range(extents.1 .0 - 1..=extents.1 .1 + 1),
            );
            if !hole.contains(next) {
                // valid coord, check if attached
                if neighbours()
                    .iter()
                    .any(|offset| hole.contains(next + *offset))
                {
                    // connected
                    hole.coords.insert(next);
                    break;
                }
            }
        }
    }

    hole
}

pub fn gen_holes(mut count: usize, total: usize, mut rng: &mut StdRng) -> Holes {
    let mut remainder = total;

    let avg = total as f32 / count as f32;
    let smallest = (avg * 0.5).ceil() as usize;
    let largest = (avg * 1.5).floor() as usize;

    debug!(
        "count: {}, total: {}, smallest: {}, largest: {}",
        count, total, smallest, largest
    );

    let mut holes = Vec::new();
    while count > 0 {
        count -= 1;
        let small = smallest.max(remainder - (count * largest).min(remainder));
        let large = largest.min(remainder - (count * smallest).min(remainder));
        debug!("remaining: {}, piece: [{},{}]", remainder, small, large);
        let size = rng.gen_range(small..=large);
        debug!(" -> {}", size);
        holes.push(gen_hole(size, &mut rng).normalize());
        remainder -= size;
    }

    Holes { holes }
}
