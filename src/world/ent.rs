/*!
Entity: ID associated with a set of components
*/

use std::{
    fmt, slice,
    sync::atomic::{AtomicU32, Ordering},
};

use crate::{
    prelude::ComponentPool,
    world::{comp, sparse::*},
};

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

    pub fn get<'a, T: comp::Component>(&self, comp: &'a ComponentPool<T>) -> Option<&'a T> {
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
    /// Tracks the number of free entries
    n_free: usize,
    /// Tracks the number of entities reserved atomically
    n_reserved: AtomicU32,
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
        if let Some(free) = self.first_free {
            let (old_gen, second_free) = match self.sparse[free.to_usize()] {
                Entry::Empty { gen, next_free } => (gen, next_free),
                _ => unreachable!("free slot bug"),
            };

            let gen = old_gen.increment();
            let entity = Entity(SparseIndex::new(free, gen));
            let dense = DenseIndex::new(RawDenseIndex::from_usize(self.dense.len()), gen);

            // update the sparse/dense array and the free slot
            self.first_free = second_free.clone();
            self.n_free -= 1;
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
        self.n_free += 1;

        true
    }

    /// Reserves an [`Entity`] only requiring `&self`. Make sure to call
    /// [`synchronize`](Self::synchronize) before use.
    pub fn reserve_atomic(&self) -> Entity {
        let n_reserved = self.n_reserved.fetch_add(1, Ordering::Relaxed) as usize;

        if n_reserved >= self.n_free {
            let nth_push = n_reserved - self.n_free;
            let slot = self.sparse.len() + nth_push;
            Entity::initial(RawSparseIndex::from_usize(slot))
        } else {
            // run linear serarch for the free slots
            let sparse = self.find_nth_free(n_reserved);

            let gen = match self.sparse[sparse.to_usize()] {
                Entry::ToDense(_) => unreachable!("free slot bug (atomic)"),
                Entry::Empty { gen, .. } => gen,
            };

            Entity(SparseIndex::new(sparse, gen))
        }
    }

    fn find_nth_free(&self, nth: usize) -> RawSparseIndex {
        let mut sparse = match self.first_free {
            Some(free) => free,
            None => unreachable!("free slot bug: tried to get free slot, but there's none"),
        };

        debug_assert!(
            matches!(
                self.sparse.get(sparse.to_usize()),
                Some(Entry::Empty { .. })
            ),
            "free slot bug: first free slot is actually filled"
        );

        for i in 0..nth {
            sparse = match self.sparse[i] {
                Entry::Empty {
                    next_free: Some(free_index),
                    ..
                } => free_index,
                _ => unreachable!("free slot bug: free slot at `{}` is actually filled", i),
            }
        }

        sparse
    }

    /// Spawns all the reserved entities
    pub fn synchronize(&mut self) {
        let n_reserved = *self.n_reserved.get_mut();
        *self.n_reserved.get_mut() = 0;

        (0..n_reserved).for_each(|_| {
            self.alloc();
        });
    }
}
