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
    entries: Vec<Entry>,
    data: Vec<Entity>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Entry {
    ToDense(DenseIndex),
    Empty { gen: Generation },
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
            Self::Empty { gen } => write!(f, "Empty({})", gen.to_usize()),
        }
    }
}

impl EntityPool {
    pub fn slice(&self) -> &[Entity] {
        &self.data
    }

    pub fn contains(&self, ent: Entity) -> bool {
        let dense = match self.entries.get(ent.0.to_usize()) {
            Some(Entry::ToDense(dense)) => dense,
            _ => return false,
        };

        let e = &self.data[dense.to_usize()];
        e.generation() == ent.generation()
    }

    pub fn iter(&self) -> slice::Iter<Entity> {
        self.data.iter()
    }

    pub fn alloc(&mut self) -> Entity {
        if self.data.len() >= self.entries.len() {
            // full
            debug_assert_eq!(self.data.len(), self.entries.len());

            let slot = self.data.len();
            let entity = Entity::initial(RawSparseIndex::from_usize(slot));

            self.data.push(entity.clone());
            self.entries.push(Entry::ToDense(DenseIndex::initial(
                RawDenseIndex::from_usize(slot),
            )));

            entity
        } else {
            // not full
            let (i_entry, gen) = self.next_empty_entry();
            let gen = gen.increment();

            // recycle the empty slot
            let dense_slot = self.data.len();
            let entity = Entity(SparseIndex::new(
                RawSparseIndex::from_usize(dense_slot),
                gen,
            ));

            self.data.push(entity.clone());
            self.entries[i_entry] =
                Entry::ToDense(DenseIndex::new(RawDenseIndex::from_usize(dense_slot), gen));

            entity
        }
    }

    pub fn dealloc(&mut self, ent: Entity) -> bool {
        let slot = ent.0.to_usize();
        if slot > self.entries.len() - 1 {
            return false;
        }

        let dense = match self.entries[slot] {
            Entry::ToDense(e) => e,
            Entry::Empty { .. } => return false,
        };

        if dense.generation() == ent.generation() {
            self.entries[slot] = Entry::Empty {
                gen: ent.generation(),
            };
            self.data.remove(dense.to_usize());
            true
        } else {
            false
        }
    }

    // FIXME: Linear search is too slow! Make a linked list of free slots instead.
    fn next_empty_entry(&self) -> (usize, &Generation) {
        for (i, entry) in self.entries.iter().enumerate() {
            if let Entry::Empty { gen: g } = entry {
                return (i, g);
            }
        }

        unreachable!()
    }
}
