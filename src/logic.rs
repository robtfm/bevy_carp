use bevy::utils::HashSet;
use bevy::prelude::*;
use rand::{Rng, prelude::{SliceRandom, StdRng}};

pub fn neighbours() -> [IVec2;4] {
    [IVec2::X, IVec2::Y, -IVec2::X, -IVec2::Y]
}


#[derive(Default, Clone)]
pub struct Level {
    pub extents: IVec2,
    pub holes: Holes,
    pub plank: Plank,
    pub setup: bool,
}

#[derive(Default)]
pub struct LevelBase(pub Level);

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct CoordSet {
    pub coords: HashSet<IVec2>,
}

impl CoordSet {
    pub fn extents(&self) -> ((i32, i32), (i32,i32)) {
        let mut coords = self.coords.iter().copied();
        let Some(first) = coords.next() else {
            return ((0,0), (0,0))
        };
        coords.fold(((first.x, first.x), (first.y, first.y)), |(x,y),b| {
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
        self.coords.contains(&IVec2::new(x,y))
    }

    pub fn touches(&self, other: &CoordSet) -> bool {
        self.coords.iter().any(|c1| 
            neighbours().iter().any(|n| other.contains(*c1 + *n))
        )
    }

    pub fn _overlaps(&self, other: &CoordSet) -> bool {
        self.coords.iter().any(|c1| 
            other.contains(*c1)
        )
    }

    pub fn equals(&self, other: &CoordSet) -> bool {
        self.coords == other.coords
    }

    pub fn rotate(&mut self) {
        self.coords = HashSet::from_iter(self.coords.drain().map(|c| IVec2::new(-c.y, c.x)));
    }

    pub fn normalize(mut self) -> Self {
        let exts = self.extents();
        self.coords = HashSet::from_iter(self.coords.drain().map(|c| c - IVec2::new(exts.0.0, exts.1.0)));
        self
    }

    pub fn shift(&mut self, by: IVec2) {
        self.coords = HashSet::from_iter(self.coords.drain().map(|c| c + by));
    }

    pub fn merge<'a>(holes: impl Iterator<Item=&'a CoordSet>) -> Hole {
        let mut coords = HashSet::default();
        for hole in holes {
            coords.extend(hole.coords.iter());
        }

        Hole { coords }
    }
}

impl std::fmt::Display for CoordSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let exts = self.extents();
        f.write_str(format!("[{},{}]", exts.0.0, exts.0.1).as_str())?;
        f.write_str("|")?;
        for _ in exts.0.0..=exts.0.1 {
            f.write_str("-")?;
        }
        f.write_str("|\n")?;
        for row in exts.1.0..=exts.1.1 {
            f.write_str("|")?;
            for col in exts.0.0..=exts.0.1 {
                if self.contains_xy(col, row) {
                    f.write_str("#")?;
                } else {
                    f.write_str(" ")?;
                }
            }
            f.write_str("|\n")?;
        }
        f.write_str("|")?;
        for _ in exts.0.0..=exts.0.1 {
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
    pub holes: Vec<Hole>
}

impl Plank {
    pub fn from_holes(holes: &Holes, mut rng: &mut StdRng) -> Self {
        let mut indexes = (0..holes.holes.len()).collect::<Vec<_>>();
        indexes.shuffle(&mut rng);

        let mut plank = Plank { coords: holes.holes[indexes[0]].coords.clone() };

        for i in 1..indexes.len() {
            plank = plank.attach_hole(&holes.holes[indexes[i]], &mut rng);
        }

        for _ in 0..rng.gen_range(0..4) {
            plank.rotate();
        }
        
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

        // let original_extents = hole.extents();
        let y_shift_range = (self.extents().1.0 - hole.extents().1.1)..=(self.extents().1.1 - hole.extents().1.0);
        let y_shift = rng.gen_range(y_shift_range.clone());

        hole.shift(IVec2::new(self.extents().0.0 - hole.extents().0.1 - 1, y_shift));

        while !self.touches(&hole) {
            hole.shift(IVec2::X);

            // if hole.extents().0.0 > self.extents().0.1 {
            //     println!("failed: \n{}, \n{}, {:?}, {:?}, {}, {:?}, {:?}", self, hole, self.extents(), hole.extents(), y_shift, y_shift_range, original_extents);
            // } else {
            //     println!("{} vs {}", hole.extents().0.0, self.extents().0.1);
            // }
        }

        self.coords.extend(hole.coords.drain());
        self
    }
}

pub fn gen_hole(size: usize, rng: &mut StdRng) -> Hole {
    let mut hole = Hole{ coords: HashSet::from_iter(std::iter::once(IVec2::ZERO)) };

    for _ in 1..size {
        let extents = hole.extents();

        loop {
            let next = IVec2::new(rng.gen_range(extents.0.0 - 1..=extents.0.1 + 1), rng.gen_range(extents.1.0 - 1..=extents.1.1 + 1));
            if !hole.contains(next) {
                // valid coord, check if attached
                if neighbours().iter().any(|offset| hole.contains(next + *offset)) {
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

    println!("count: {}, total: {}, smallest: {}, largest: {}", count, total, smallest, largest);

    let mut holes = Vec::new();
    while count > 0 {
        count -= 1;
        let small = smallest.max(remainder - (count * largest).min(remainder));
        let large = largest.min(remainder - (count * smallest).min(remainder));
        println!("remaining: {}, piece: [{},{}]", remainder, small, large);
        let size = rng.gen_range(small..=large);
        println!(" -> {}", size);
        holes.push(gen_hole(size, &mut rng).normalize());
        remainder -= size;
    }
    
    Holes{ holes }
}
