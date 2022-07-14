//! `Serialize` implementaitons

use serde::ser::{SerializeMap, SerializeSeq};

use crate::{prelude::*, serde::Registry};

// --------------------------------------------------------------------------------
// Typed serialize
// --------------------------------------------------------------------------------

pub struct ComponentPoolSerialize<'w, T> {
    pub comps: &'w ComponentPool<T>,
}

impl<'w, T: Component + serde::Serialize + 'static> serde::Serialize
    for ComponentPoolSerialize<'w, T>
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut seq = serializer.serialize_seq(None)?;

        for comp in self.comps.iter() {
            seq.serialize_element(comp)?;
        }

        seq.end()
    }
}

// --------------------------------------------------------------------------------
// Type-erased collection's serialize
// --------------------------------------------------------------------------------

pub struct ComponentPoolMapSerialize<'w> {
    pub world: &'w World,
}

impl<'w> serde::Serialize for ComponentPoolMapSerialize<'w> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let world = self.world;
        let reg = world.res::<Registry>();

        let mut map = serializer.serialize_map(None)?;

        for comps in world.comp.any_iter() {
            let ty = comps.info.ty;

            let key = match reg.to_stable().get(&ty) {
                Some(key) => key,
                None => continue,
            };

            let fetch = match reg.serialize_fetch.get(&ty) {
                Some(f) => f,
                None => continue,
            };

            (fetch)(world, &mut |serialize| {
                map.serialize_entry(&key, serialize).unwrap();
            });
        }

        map.end()
    }
}

pub struct ResourceMapSerialize<'w> {
    pub world: &'w World,
}

impl<'w> serde::Serialize for ResourceMapSerialize<'w> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let world = self.world;
        let reg = world.res::<Registry>();

        let mut map = serializer.serialize_map(None)?;

        for res in world.res.any_iter() {
            let ty = res.info.ty;

            let key = match reg.to_stable().get(&ty) {
                Some(key) => key,
                None => continue,
            };

            let fetch = match reg.serialize_fetch.get(&ty) {
                Some(f) => f,
                None => continue,
            };

            (fetch)(world, &mut |serialize| {
                map.serialize_entry(&key, serialize).unwrap();
            });
        }

        map.end()
    }
}
