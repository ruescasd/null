//! The chasm's occupancy table: what each piece of the way takes, as boxes
//! in the world (on the grid every face is square to the world's axes), each
//! for a use: its solid; the space a walker needs; what it carves out of the
//! rock. A piece fits where its boxes agree with the rock and with every
//! other piece's, tested box against box, exactly; the boxes are found
//! through an index of 1 m cells. (Pieces joined end to end, `friends`, may
//! meet and overlap where they join, and only there: in the box round their
//! join, its port; they, and pieces a step further along the way,
//! `neighbours`, may carve close to each other anywhere, but stand nowhere
//! in each other's way.)

use std::collections::HashMap;

use bevy::prelude::*;

/// What a box is for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Use {
    /// Stone: a deck, steps, a stair's mass.
    Solid,
    /// Where a walker goes: nothing may stand in it, and it must be clear of
    /// rock, or carved out of it.
    Space,
    /// Carved out of the rock: kept apart from what else is carved.
    Cut,
    /// Kept clear of anything solid but its own and what it joins: before a
    /// door (what stands there belongs to another leg of the way).
    Keep,
}

/// A box for a use.
#[derive(Clone, Copy, Debug)]
pub struct Claim {
    pub lo: Vec3,
    pub hi: Vec3,
    pub kind: Use,
}

impl Claim {
    /// The box between two corners (either way round).
    pub fn new(a: Vec3, b: Vec3, kind: Use) -> Claim {
        Claim { lo: a.min(b), hi: a.max(b), kind }
    }

    /// How much of it lies in another box grown by `margin` all round.
    fn shared(&self, o: &Claim, margin: f32) -> f32 {
        let d = (self.hi.min(o.hi + margin) - self.lo.max(o.lo - margin)).max(Vec3::ZERO);
        d.x * d.y * d.z
    }
}

/// How much two boxes may share before it counts (m3): a rounding error.
const SHARED: f32 = 0.002;

/// The rock kept between what is carved by pieces not joined to each other.
pub const ROCK: f32 = 1.0;

/// The table: every piece's boxes, by piece (in order, for taking pieces
/// back), indexed by 1 m cells.
#[derive(Default)]
pub struct Table {
    claims: Vec<(usize, Claim)>,
    cells: HashMap<IVec3, Vec<u32>>,
    /// Pieces that keep rock round what they carve from all but the pieces
    /// they join (a hall: a room in the rock, entered by its doors), each
    /// with how many boxes the table held when it came.
    solo: Vec<(usize, usize)>,
}

/// The cells a box covers (grown by `margin`).
fn cells(c: &Claim, margin: f32) -> impl Iterator<Item = IVec3> {
    let (lo, hi) = ((c.lo - margin).floor().as_ivec3(), (c.hi + margin).floor().as_ivec3());
    (lo.x..=hi.x).flat_map(move |x| (lo.y..=hi.y).flat_map(move |y| (lo.z..=hi.z).map(move |z| IVec3::new(x, y, z))))
}

impl Table {
    /// Why a piece's boxes do not fit, if they do not: against the rock
    /// (`rock(lo, hi)`: how much of a box is rock) and every other piece's,
    /// but where it joins another (`friends`: each with its port), and, for
    /// what is carved, near those a step further along (`neighbours`).
    /// (`solo`: the piece keeps rock from all but the pieces it joins, as a
    /// room in the rock does, its neighbours too; and neighbours that do
    /// keep it from this one.)
    pub fn misfit(&self, owner: usize, piece: &[Claim], friends: &[(usize, Claim)], neighbours: &[usize], solo: bool, rock: &dyn Fn(Vec3, Vec3) -> f32) -> Option<String> {
        let near = |c: &Claim, margin: f32, also: &[usize]| -> Vec<(usize, Claim)> {
            let mut ids: Vec<u32> = cells(c, margin).filter_map(|k| self.cells.get(&k)).flatten().copied().collect();
            ids.sort_unstable();
            ids.dedup();
            ids.into_iter()
                .map(|i| self.claims[i as usize])
                .filter(|(o, k)| {
                    // (Where it joins another, inside their port.)
                    let (lo, hi) = (c.lo.max(k.lo - margin), c.hi.min(k.hi + margin));
                    *o != owner && !also.contains(o) && !friends.iter().any(|(f, port)| f == o && port.lo.cmple(lo).all() && hi.cmple(port.hi).all())
                })
                .collect()
        };
        for c in piece {
            match c.kind {
                Use::Space => {
                    // (In the rock only where carved out: by one of its own
                    // boxes, or another piece's.)
                    let r = rock(c.lo, c.hi);
                    if r > SHARED {
                        let carved = piece
                            .iter()
                            .chain(self.claims.iter().map(|(_, c)| c))
                            .filter(|k| k.kind == Use::Cut)
                            .map(|k| {
                                let lo = c.lo.max(k.lo);
                                let hi = c.hi.min(k.hi);
                                if lo.cmplt(hi).all() { rock(lo, hi) } else { 0.0 }
                            })
                            .sum::<f32>();
                        if r - carved > SHARED {
                            return Some(format!("space {:?}..{:?} in rock ({:.2} m3)", c.lo, c.hi, r - carved));
                        }
                    }
                    if let Some((o, k)) = near(c, 0.0, &[]).into_iter().find(|(_, k)| k.kind == Use::Solid && c.shared(k, 0.0) > SHARED) {
                        return Some(format!("space {:?}..{:?} into piece {o}'s solid {:?}..{:?}", c.lo, c.hi, k.lo, k.hi));
                    }
                }
                Use::Solid => {
                    if let Some((o, k)) = near(c, 0.0, &[]).into_iter().find(|(_, k)| matches!(k.kind, Use::Space | Use::Keep) && c.shared(k, 0.0) > SHARED) {
                        return Some(format!("solid {:?}..{:?} into piece {o}'s {:?} {:?}..{:?}", c.lo, c.hi, k.kind, k.lo, k.hi));
                    }
                }
                Use::Keep => {
                    if let Some((o, k)) = near(c, 0.0, &[]).into_iter().find(|(_, k)| k.kind == Use::Solid && c.shared(k, 0.0) > SHARED) {
                        return Some(format!("before a door {:?}..{:?}: piece {o}'s solid {:?}..{:?}", c.lo, c.hi, k.lo, k.hi));
                    }
                }
                Use::Cut => {
                    // (The rock kept between what separate pieces carve; not
                    // between what pieces joined along the way do, anywhere:
                    // what one carves passing under another it joins is sound
                    // rock between, and hidden.)
                    let joined: Vec<usize> = neighbours.iter().copied().filter(|o| !solo && !self.solo.iter().any(|s| s.0 == *o)).chain(friends.iter().map(|f| f.0)).collect();
                    if let Some((o, k)) = near(c, ROCK, &joined).into_iter().find(|(_, k)| k.kind == Use::Cut && c.shared(k, ROCK) > SHARED) {
                        return Some(format!("cut {:?}..{:?} within {ROCK} m of piece {o}'s cut {:?}..{:?}", c.lo, c.hi, k.lo, k.hi));
                    }
                }
            }
        }
        None
    }

    /// Takes in a piece's boxes.
    pub fn add(&mut self, owner: usize, piece: &[Claim]) {
        for c in piece {
            let i = self.claims.len() as u32;
            for k in cells(c, 0.0) {
                self.cells.entry(k).or_default().push(i);
            }
            self.claims.push((owner, *c));
        }
    }

    /// Marks a piece as keeping rock from all but the pieces it joins (see
    /// `misfit`).
    pub fn solo(&mut self, owner: usize) {
        self.solo.push((owner, self.claims.len()));
    }

    /// How many boxes it holds (to take back to, see `truncate`).
    pub fn len(&self) -> usize {
        self.claims.len()
    }

    /// Takes back every box after the first `n`.
    pub fn truncate(&mut self, n: usize) {
        for i in n..self.claims.len() {
            let c = self.claims[i].1;
            for k in cells(&c, 0.0) {
                if let Some(list) = self.cells.get_mut(&k) {
                    list.retain(|&j| (j as usize) < n);
                }
            }
        }
        self.claims.truncate(n);
        self.solo.retain(|s| s.1 < n);
    }
}
