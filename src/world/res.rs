/*!
Resources: virtually `World` fields backed by an anymap
*/

use std::{
    any::{self, TypeId},
    cell::RefCell,
    fmt, mem, ops,
};

use atomic_refcell::{AtomicRef, AtomicRefCell, AtomicRefMut};
use downcast_rs::{impl_downcast, Downcast};
use rustc_hash::FxHashMap;

/// Type boundary for resource types
pub trait Resource: 'static + fmt::Debug + Downcast {}

impl_downcast!(Resource);

impl<T: 'static + fmt::Debug> Resource for T {}

/// Dynamic fields of a `World` backed by an anymap
#[derive(Debug, Default)]
pub struct ResourceMap {
    cells: FxHashMap<TypeId, AtomicRefCell<AnyResource>>,
}

#[derive(Debug)]
struct AnyResource {
    /// Type name string for debug print
    #[allow(unused)]
    of_type: &'static str,
    any: Box<dyn Resource>,
}

impl ResourceMap {
    pub fn insert<T: Resource>(&mut self, x: T) -> Option<T> {
        let new_cell = AtomicRefCell::new(AnyResource {
            any: Box::new(x),
            of_type: any::type_name::<T>(),
        });
        let old_cell = self.cells.insert(TypeId::of::<T>(), new_cell)?;
        Some(Self::unwrap_res(old_cell.into_inner()))
    }

    pub fn remove<T: Resource>(&mut self) -> Option<T> {
        let old_cell = self.cells.remove(&TypeId::of::<T>())?;
        Some(Self::unwrap_res(old_cell.into_inner()))
    }

    fn unwrap_res<T: Resource>(res: AnyResource) -> T {
        let box_t = res.any.downcast::<T>().unwrap_or_else(|_| unreachable!());
        *box_t
    }

    pub fn contains<T: Resource>(&self) -> bool {
        self.cells.contains_key(&TypeId::of::<T>())
    }

    /// Tries to get an immutable access to a resource
    /// # Panics
    /// Panics when breaking the aliasing rules.
    pub fn borrow<T: Resource>(&self) -> Option<Res<T>> {
        let cell = self.cells.get(&TypeId::of::<T>())?;
        let borrow = AtomicRef::map(cell.borrow(), |res| res.any.downcast_ref::<T>().unwrap());
        Some(Res { borrow })
    }

    /// Tries to get a mutable access to a resource
    /// # Panics
    /// Panics when breaking the aliasing rules.
    pub fn borrow_mut<T: Resource>(&self) -> Option<ResMut<T>> {
        let cell = self.cells.get(&TypeId::of::<T>())?;
        let borrow = AtomicRefMut::map(cell.borrow_mut(), |res| {
            res.any
                .downcast_mut::<T>()
                .unwrap_or_else(|| unreachable!())
        });
        Some(ResMut { borrow })
    }

    /// Returns a debug display. This is safe because it has exclusive access.
    pub fn display(&mut self) -> ResourceMapDisplay {
        let mut res = Self::default();
        mem::swap(&mut res, self);
        ResourceMapDisplay {
            res: RefCell::new(res),
            original_res: self,
        }
    }
}

/// See [`ResourceMap::display`]
pub struct ResourceMapDisplay<'r> {
    res: RefCell<ResourceMap>,
    original_res: &'r mut ResourceMap,
}

impl<'w> Drop for ResourceMapDisplay<'w> {
    fn drop(&mut self) {
        mem::swap(self.original_res, self.res.get_mut());
    }
}

impl<'r> fmt::Debug for ResourceMapDisplay<'r> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(
                self.res
                    .borrow_mut()
                    .cells
                    .values_mut()
                    .map(|cell| cell.get_mut()),
            )
            .finish()
    }
}

/// Immutable access to a resource of type `T`
#[derive(Debug)]
pub struct Res<'r, T: Resource> {
    borrow: AtomicRef<'r, T>,
}

impl<'r, T: Resource> ops::Deref for Res<'r, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        self.borrow.deref()
    }
}

impl<'r, T: Resource> Res<'r, T> {
    #[inline]
    pub fn deref(&self) -> &T {
        ops::Deref::deref(self)
    }
}

/// Mutable access to a resource of type `T`
#[derive(Debug)]
pub struct ResMut<'r, T: Resource> {
    borrow: AtomicRefMut<'r, T>,
}

impl<'r, T: Resource> ops::Deref for ResMut<'r, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        self.borrow.deref()
    }
}

impl<'r, T: Resource> ops::DerefMut for ResMut<'r, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.borrow.deref_mut()
    }
}

impl<'r, T: Resource> ResMut<'r, T> {
    #[inline]
    pub fn deref(&self) -> &T {
        ops::Deref::deref(self)
    }
}

impl<'r, T: Resource> ResMut<'r, T> {
    #[inline]
    pub fn deref_mut(&mut self) -> &mut T {
        ops::DerefMut::deref_mut(self)
    }
}
