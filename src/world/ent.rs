/*!
Entity: ID associated with a set of components
*/

use std::{fmt, slice};

use crate::world::{comp, sparse::*};

/// Identifier that represents an object made of components
///
/// Components of entities are stored in a sparse set-based Struct of Arrays.
///
/// # Non-zero type use
///
/// ```
/// # use std::mem::size_of;
/// # use toecs::world::ent::Entity;
/// assert_eq!(size_of::<Entity>(), size_of::<Option<Entity>>());
///
/// struct Test { a: u32, e: Entity, x: u32 }
/// assert_eq!(size_of::<Test>(), size_of::<Option<Test>>());
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Entity(pub(crate) SparseIndex);

impl fmt::Debug for Entity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        <Self as fmt::Display>::fmt(self, f)
    }
}

impl fmt::Display for Entity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Entity({}, {})",
            self.0.raw().to_usize(),
            self.0.generation().to_usize()
        )
    }
}

impl Entity {
    fn initial(slot: RawSparseIndex) -> Self {
        Self(SparseIndex::initial(slot))
    }

    pub fn generation(&self) -> Generation {
        self.0.generation()
    }

    /// FIXME: Abstract `&Comp<T>` and `&CompMut<T>` (with `AsRef<T>`?)
    pub fn get<'a, T: comp::Component>(&self, comp: &'a comp::Comp<T>) -> Option<&'a T> {
        comp.get(*self)
    }
}

/// Pool of entities
///
/// # Implementation
///
/// It is different from ordinary sparse set in two points:
///
/// 1. It takes sparse index and returns sparse index, so it doesn't need to handle a dense to
/// sparse map.
/// 2. It needs to recycle sparse index so that the generation is incremented.
#[derive(Debug, Default)]
pub struct EntityPool {
    sparse: Vec<Entry>,
    dense: Vec<Entity>,
    first_free: Option<RawSparseIndex>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Entry {
    ToDense(DenseIndex),
    Empty {
        gen: Generation,
        next_free: Option<RawSparseIndex>,
    },
}

impl fmt::Debug for Entry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ToDense(dense) => write!(
                f,
                "ToDense({}, {})",
                dense.raw().to_usize(),
                dense.generation().to_usize()
            ),
            Self::Empty { gen, next_free } => {
                write!(f, "Empty({}, {:?})", gen.to_usize(), next_free)
            }
        }
    }
}

impl EntityPool {
    pub fn slice(&self) -> &[Entity] {
        &self.dense
    }

    pub fn contains(&self, ent: Entity) -> bool {
        let dense = match self.sparse.get(ent.0.to_usize()) {
            Some(Entry::ToDense(dense)) => dense,
            _ => return false,
        };

        let e = &self.dense[dense.to_usize()];
        e.generation() == ent.generation()
    }

    pub fn iter(&self) -> slice::Iter<Entity> {
        self.dense.iter()
    }

    pub fn alloc(&mut self) -> Entity {
        if let Some(free) = self.first_free.take() {
            let (old_gen, second_free) = match self.sparse[free.to_usize()] {
                Entry::Empty { gen, next_free } => (gen, next_free),
                _ => unreachable!("free slot bug"),
            };

            let gen = old_gen.increment();
            let entity = Entity(SparseIndex::new(free, gen));
            let dense = DenseIndex::new(RawDenseIndex::from_usize(self.dense.len()), gen);

            // update the sparse/dense array and the free slot
            self.first_free = second_free.clone();
            self.dense.push(entity.clone());
            self.sparse[free.to_usize()] = Entry::ToDense(dense);

            entity
        } else {
            // full
            debug_assert_eq!(self.dense.len(), self.sparse.len(), "free slot bug");

            let index = self.dense.len();
            let entity = Entity::initial(RawSparseIndex::from_usize(index));

            // update the sparse/dense array (the free slot is None)
            self.dense.push(entity.clone());
            self.sparse.push(Entry::ToDense(DenseIndex::initial(
                RawDenseIndex::from_usize(index),
            )));

            entity
        }
    }

    pub fn dealloc(&mut self, ent: Entity) -> bool {
        let slot = ent.0.to_usize();
        if slot > self.sparse.len() - 1 {
            return false;
        }

        let dense = match self.sparse[slot] {
            Entry::ToDense(e) => e,
            Entry::Empty { .. } => return false,
        };

        if dense.generation() != ent.generation() {
            return false;
        }

        // update sparse/dense array and the free slots
        self.sparse[slot] = Entry::Empty {
            gen: ent.generation(),
            next_free: self.first_free,
        };
        self.dense.remove(dense.to_usize());
        self.first_free = Some(RawSparseIndex::from_usize(slot));

        true
    }
}
