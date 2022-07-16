//! `Deserialize` implementations

use std::fmt;

use serde::de;

use crate::{
    prelude::*,
    serde::Registry,
    world::{
        comp::{AnyComponentPool, ComponentPoolMap},
        res::{AnyResource, ResourceMap},
    },
};

pub struct ComponentPoolDeserialize<'a> {
    pub reg: &'a Registry,
}

impl<'a, 'de> de::DeserializeSeed<'de> for ComponentPoolDeserialize<'a> {
    type Value = ComponentPoolMap;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct ComponentPoolMapVisitor<'a> {
            reg: &'a Registry,
        }

        impl<'a, 'de> de::Visitor<'de> for ComponentPoolMapVisitor<'a> {
            type Value = ComponentPoolMap;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("ComponentPoolMap")
            }

            fn visit_map<A>(self, mut access: A) -> Result<Self::Value, A::Error>
            where
                A: de::MapAccess<'de>,
            {
                let mut pools = ComponentPoolMap::default();

                let Self { reg } = self;

                while let Some(raw_key) = access.next_key::<String>()? {
                    let info = match reg.intern(&raw_key) {
                        Some(x) => x.clone(),
                        None => continue,
                    };

                    let deserialize_comp_pool = match reg.deserialize_comp_pool.get(&info.ty) {
                        Some(f) => f,
                        None => continue,
                    };

                    let any = access.next_value_seed(deserialize_comp_pool)?;

                    pools.erased_register(info.ty, || AnyComponentPool {
                        info: info.clone(),
                        any,
                    });
                }

                Ok(pools)
            }
        }

        deserializer.deserialize_map(ComponentPoolMapVisitor { reg: self.reg })
    }
}
