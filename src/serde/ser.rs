//! `Serialize` implementaitons

use serde::ser::SerializeMap;

use crate::{prelude::*, serde::Registry};

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

            let serialize = match reg.serialize_comp.get(&ty) {
                Some(f) => f,
                None => continue,
            };

            (serialize)(world, &mut |serialize| {
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

            let serialize = match reg.serialize_res.get(&ty) {
                Some(f) => f,
                None => continue,
            };

            (serialize)(world, &mut |serialize| {
                map.serialize_entry(&key, serialize).unwrap();
            });
        }

        map.end()
    }
}
