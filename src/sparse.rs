/*!
Internal [`SparseSet`] utilities

This module is public, but just for the intenral documentation. See also `EntityPool` as a sparse
index allocator.
*/

use std::{iter, num::NonZeroU32, slice};

/// The length of [`SparseArray`] will be multiples of this value
const UNIT_LEN: usize = 64;

macro_rules! newtype_index {
    ($(#[$meta:meta])* $vis:vis $ty:ident($internal:ty);) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        $(#[$meta])*
        $vis struct $ty(pub(crate) $internal);

        #[allow(unused)]
        impl $ty {
            pub const ZERO: Self = Self(0);

            pub(crate) fn from_usize(x: usize) -> Self {
                Self(x as $internal)
            }

            pub fn to_usize(&self) -> usize {
                self.0 as usize
            }
        }
    };
}

newtype_index! {
    /// Newtype sparse index
    pub(crate) RawSparseIndex(u32);
}

newtype_index! {
    /// Newtype dense index
    pub(crate) RawDenseIndex(u32);
}

/// Identifies new/old items at the same slot
///
/// Generation of a slot is incremnted on new item insertion by a sparse index allocator.
///
/// ```
/// # use std::mem::size_of;
/// # use toecs::sparse::Generation;
/// assert_eq!(size_of::<Generation>(), size_of::<Option<Generation>>());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Generation {
    raw: NonZeroU32,
}

impl Generation {
    pub const INITIAL: Generation = Self {
        raw: unsafe { NonZeroU32::new_unchecked(1) },
    };

    pub(crate) fn increment(self) -> Self {
        Self {
            raw: unsafe { NonZeroU32::new_unchecked(self.raw.get() + 1) },
        }
    }

    pub fn to_usize(&self) -> usize {
        self.raw.get() as usize
    }
}

macro_rules! generational_index {
    ($(#[$meta:meta])* $vis:vis $ty:ident($index:ty);) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        $(#[$meta])*
        $vis struct $ty {
            raw: $index,
            gen: Generation,
        }

        #[allow(unused)]
        impl $ty {
            pub(crate) fn new(raw: $index, gen: Generation) -> Self {
                Self {
                    raw,
                    gen,
                }
            }

            pub(crate) fn initial(raw: $index) -> Self {
                Self {
                    raw,
                    gen: Generation::INITIAL,
                }
            }

            pub(crate) fn increment_generation(self) -> Self {
                Self {
                    raw: self.raw,
                    gen: self.gen.increment(),
                }
            }

            pub fn generation(&self) -> Generation {
                self.gen
            }

            pub(crate) fn raw(&self) -> $index {
                self.raw
            }

            pub fn to_usize(&self) -> usize {
                self.raw.to_usize()
            }
        }
    };
}

generational_index!(
    /// Sparse index with generation
    ///
    /// It's not generic because one sparse index can be used to index multiple sparse sets.
    pub SparseIndex(RawSparseIndex);
);

generational_index!(
     /// Dense index with generation
     ///
     /// It's not generic because one dense index can be used to index multiple dense arrays.
     pub DenseIndex(RawDenseIndex);
);

/**
Dense vec indexed by [`SparseIndex`]

See [ECS back and Forth][ebaf] for original information.

[ebaf]: https://skypjack.github.io/2019-02-14-ecs-baf-part-1/

# External access via sparse index

Sparse index doesn't always have a corresponding item. So `SparseSet<T>` is like `Vec<Option<T>>`,
but all the items are stored in a dense, packed array, thanks to the indrection of the sparse-dense
index map:

```text
sparse index:   0 1 2 3 4 5
                | | | | | |
                v v v v v v
         map: [ 0 - 2 1 - - ]
                |   | |
                | +---+
                | | |
                v v v
    data_vec: [ a b c ]
```

Note that this access mode is slow!

# Iteration of the dense vec

Iteration of `SparseSet` starts from the dense vec: the fastest iterator!

Sometimes we want to iterate through both the data and the sparse indices so that we can access
other sparse sets with that index. So the `SparseSet` internally manages a dense-sparse index map:

```
# type SparseArray = Vec<u32>;
# type Index = u32;
pub struct SparseSet<T> {
    to_dense: SparseArray,
    to_sparse: Vec<Index>,
    data: Vec<T>,
}
```

# SoA and perfect SoA

Sparse set is intended for Struct of Arrays. Ideally, all relevant dense vecs should be accessed
with the same dense index, which is called "perfect SoA". It requires syncing and sorting. There's a
known workaround called "groups".
*/
#[derive(Debug, Clone)]
pub struct SparseSet<T> {
    /// Maps `SparseIndex` to `DenseIndex`
    to_dense: SparseArray,
    /// Maps `DenseIndex` back to `SparseIndex` (`Index`)
    ///
    /// It must be synced with the `data`.
    to_sparse: Vec<SparseIndex>,
    /// Dense, packed array of targt data
    data: Vec<T>,
}

impl<T> Default for SparseSet<T> {
    fn default() -> Self {
        Self {
            to_dense: Default::default(),
            to_sparse: Default::default(),
            data: Default::default(),
        }
    }
}

impl<T> SparseSet<T> {
    #[inline]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn as_slice(&self) -> &[T] {
        &self.data
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.data
    }

    pub fn indices(&self) -> &[SparseIndex] {
        &self.to_sparse
    }

    pub fn as_slice_with_indices(&self) -> (&[SparseIndex], &[T]) {
        (&self.to_sparse, &self.data)
    }

    pub fn as_mut_slice_with_indices(&mut self) -> (&[SparseIndex], &mut [T]) {
        (&self.to_sparse, &mut self.data)
    }

    pub fn iter(&self) -> slice::Iter<T> {
        self.data.iter()
    }

    pub fn iter_with_index(&self) -> iter::Zip<slice::Iter<SparseIndex>, slice::Iter<T>> {
        self.to_sparse.iter().zip(self.data.iter())
    }

    pub fn contains(&self, sparse: SparseIndex) -> bool {
        let dense = match self.to_dense.get(sparse) {
            Some(dense) => dense,
            _ => return false,
        };
        dense.gen == sparse.gen
    }

    pub fn get(&self, sparse: SparseIndex) -> Option<&T> {
        let dense = self.to_dense.get(sparse)?;
        if dense.gen == sparse.gen {
            Some(&self.data[dense.to_usize()])
        } else {
            None
        }
    }

    pub fn get_with_index(&self, sparse: SparseIndex) -> Option<(&SparseIndex, &T)> {
        let dense = self.to_dense.get(sparse)?;
        if dense.gen == sparse.gen {
            Some((
                &self.to_sparse[dense.to_usize()],
                &self.data[dense.to_usize()],
            ))
        } else {
            None
        }
    }

    /// # Safety
    /// UB if the slot is out of the bounds.
    pub unsafe fn get_by_slot_unchecked(&self, slot: usize) -> &T {
        self.data.get_unchecked(slot)
    }

    pub fn get_mut(&mut self, sparse: SparseIndex) -> Option<&mut T> {
        let dense = self.to_dense.get(sparse)?;
        if dense.gen == sparse.gen {
            Some(&mut self.data[dense.to_usize()])
        } else {
            None
        }
    }

    pub fn get_with_index_mut(&mut self, sparse: SparseIndex) -> Option<(&SparseIndex, &mut T)> {
        let dense = self.to_dense.get(sparse)?;
        if dense.gen == sparse.gen {
            Some((
                &self.to_sparse[dense.to_usize()],
                &mut self.data[dense.to_usize()],
            ))
        } else {
            None
        }
    }

    /// # Safety
    /// UB if the slot is out of the bounds.
    pub unsafe fn get_by_slot_unchecked_mut(&mut self, slot: usize) -> &mut T {
        self.data.get_unchecked_mut(slot)
    }

    /// Returns old item if it's present
    pub fn insert(&mut self, sparse: SparseIndex, mut data: T) -> Option<T> {
        match self.to_dense.get_or_alloc_mut(sparse) {
            Some(dense) => {
                debug_assert!(
                    sparse.gen >= dense.gen,
                    "generation has to increase monotonically"
                );
                // overwrite the existing slots
                dense.gen = sparse.gen;
                std::mem::swap(&mut self.data[dense.to_usize()], &mut data);
                self.to_sparse[dense.to_usize()] = sparse;

                Some(data)
            }
            None => {
                // create new slot
                self.to_dense.set(
                    sparse.to_usize(),
                    DenseIndex {
                        raw: RawDenseIndex::from_usize(self.data.len()),
                        gen: sparse.gen,
                    },
                );

                self.data.push(data);
                self.to_sparse.push(sparse);

                None
            }
        }
    }

    pub fn swap_remove(&mut self, sparse: SparseIndex) -> Option<T> {
        let dense = self.to_dense.remove(sparse)?;
        debug_assert!(sparse.gen <= dense.gen);

        if dense.gen != sparse.gen {
            return None;
        }

        let removal = self.data.swap_remove(dense.to_usize());
        self.to_sparse.swap_remove(dense.to_usize());

        // if we swap the last item with the hole
        if let Some(swapped_sparse) = self.to_sparse.get(dense.to_usize()) {
            // update the sparse-dense map to the swapped item
            self.to_dense.set(
                swapped_sparse.to_usize(),
                DenseIndex {
                    raw: dense.raw,
                    gen: swapped_sparse.gen,
                },
            );
        }

        Some(removal)
    }

    pub fn parts(&self) -> (&[Option<DenseIndex>], &[SparseIndex], &[T]) {
        (&self.to_dense.data, &self.to_sparse, &self.data)
    }

    pub fn parts_mut(&mut self) -> (&[Option<DenseIndex>], &[SparseIndex], &mut [T]) {
        (&self.to_dense.data, &self.to_sparse, &mut self.data)
    }
}

/// Maps [`SparseIndex`] to [`DenseIndex`]
#[derive(Debug, Clone)]
struct SparseArray {
    data: Vec<Option<DenseIndex>>,
}

impl Default for SparseArray {
    fn default() -> Self {
        Self {
            data: Vec::default(),
        }
    }
}

impl SparseArray {
    /// Returns the corresponding item's slot
    pub fn get(&self, sparse: SparseIndex) -> Option<DenseIndex> {
        self.data.get(sparse.to_usize())?.map(|dense| {
            debug_assert!(
                sparse.gen <= dense.gen,
                "generation has to increase monotonically"
            );
            dense.clone()
        })
    }

    pub fn set(&mut self, sparse_slot: usize, dense: DenseIndex) {
        self.data[sparse_slot] = Some(dense);
    }

    /// Returns the corresponding item's index to the sparse, generational index.
    /// If there's no slot, the array will allocate more slots.
    pub fn get_or_alloc_mut(&mut self, sparse: SparseIndex) -> Option<&mut DenseIndex> {
        let idx_usize = sparse.to_usize();
        self.maybe_grow(idx_usize);
        self.data.get_mut(idx_usize).unwrap().as_mut()
    }

    pub fn remove(&mut self, idx: SparseIndex) -> Option<DenseIndex> {
        self.data.get_mut(idx.to_usize())?.take()
    }

    /// After `grow`, `self.data.len() >= target_slot + 1`
    fn maybe_grow(&mut self, target_slot: usize) -> bool {
        if self.data.len() >= target_slot + 1 {
            false
        } else {
            let n_units = (UNIT_LEN + target_slot) / UNIT_LEN;
            let new_len = n_units * UNIT_LEN;
            self.data.resize(new_len, None);
            true
        }
    }
}
