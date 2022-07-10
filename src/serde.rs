//! Serde support

mod ser;

pub use erased_serde;
pub use serde;

use std::{
    any::{self, Any, TypeId},
    fmt,
};

use rustc_hash::FxHashMap;
use serde::ser::SerializeStruct;

use crate::{prelude::*, world::res::Resource};

/// Stable type ID across compliations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct StableTypeId {
    raw: &'static str,
}

impl StableTypeId {
    /// Gives type a stable ID
    pub fn of<T: Any>() -> Self {
        Self {
            raw: any::type_name::<T>(),
        }
    }
}

/// Fetches the target as `&dyn erased_serde::Serialize` and then run a closure
type FetchFn = fn(&World, &mut dyn FnMut(&dyn erased_serde::Serialize));

/// (Resource) Type information registry for ser/de support
// TODO: concurrency
#[derive(Default)]
pub struct Registry {
    /// Dynamic type ID to stable type ID
    d2s: FxHashMap<TypeId, StableTypeId>,

    /// Stable type ID to dynamic type ID
    s2d: FxHashMap<StableTypeId, TypeId>,

    /// See [`FetchFn`]
    serialize_fetch: FxHashMap<TypeId, FetchFn>,
}

impl fmt::Debug for Registry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Registry")
    }
}

impl Registry {
    /// Registers a [`Serialize`] resource
    pub fn register_res<T: Resource + serde::Serialize + 'static>(&mut self) {
        let ty = self.register_::<T>();

        self.serialize_fetch.insert(ty, |world, closure| {
            let res = match world.try_res::<T>() {
                Ok(res) => res,
                _ => return,
            };

            (closure)(&*res);
        });
    }

    /// Registers a [`Serialize`] resource
    pub fn register<T: Component + serde::Serialize + 'static>(&mut self) {
        let ty = self.register_::<T>();

        self.serialize_fetch.insert(ty, |world, closure| {
            let comps = match world.try_comp::<T>() {
                Ok(c) => c,
                _ => return,
            };

            let serialize = ser::ComponentPoolSerialize { comps: &comps };

            (closure)(&serialize);
        });
    }

    fn register_<T: 'static>(&mut self) -> TypeId {
        let s = StableTypeId::of::<T>();
        let d = TypeId::of::<T>();

        self.d2s.insert(d, s);
        self.s2d.insert(s, d);

        d
    }
}

impl Registry {
    /// Converts type to stable ID
    pub fn to_stable(&self, d: &TypeId) -> Option<&StableTypeId> {
        self.d2s.get(&d)
    }

    /// Converts stable ID to type ID
    pub fn to_dynamic(&self, s: &StableTypeId) -> Option<&TypeId> {
        self.s2d.get(&s)
    }

    /// Converts type to stable ID
    pub fn to_stable_t<T: 'static>(&self) -> Option<&StableTypeId> {
        self.to_stable(&TypeId::of::<T>())
    }

    /// Converts stable ID to type ID
    pub fn to_dynamic_t<T: 'static>(&self) -> Option<&TypeId> {
        self.to_dynamic(&StableTypeId::of::<T>())
    }
}

impl World {
    /// Casts the [`World`] to [`WorldSerialize`]
    pub fn as_serialize(&self) -> WorldSerialize {
        WorldSerialize { world: self }
    }
}

pub struct WorldSerialize<'w> {
    world: &'w World,
}

// TODO: consider collecting all the errors
impl<'w> serde::Serialize for WorldSerialize<'w> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let world = self.world;

        let mut state = serializer.serialize_struct("World", 1)?;

        // TODO: resources

        // TODO: entities

        // components
        state.serialize_field("comp", &ser::ComponentPoolMapSerialize { world })?;

        state.end()
    }
}
