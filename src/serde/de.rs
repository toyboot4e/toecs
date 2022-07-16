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

/// See [`Registry::as_deserialize`]
pub struct WorldDeserialize<'a> {
    pub reg: &'a Registry,
}

impl<'a, 'de> serde::de::DeserializeSeed<'de> for WorldDeserialize<'a> {
    type Value = World;

    fn deserialize<D>(self, mut deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // --------------------------------------------------------------------------------
        // Field visitor
        // --------------------------------------------------------------------------------

        enum Field {
            Comp,
        }

        const FIELDS: &'static [&'static str] = &["comp"];

        impl<'de> serde::de::Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> Result<Field, D::Error>
            where
                D: serde::de::Deserializer<'de>,
            {
                struct FieldVisitor;

                impl<'de> serde::de::Visitor<'de> for FieldVisitor {
                    type Value = Field;

                    fn expecting(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
                        write!(fmt, "one of {:?}", FIELDS)
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Field, E>
                    where
                        E: serde::de::Error,
                    {
                        match value {
                            "comp" => Ok(Field::Comp),
                            _ => Err(serde::de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }

                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        // --------------------------------------------------------------------------------
        // World visitor
        // --------------------------------------------------------------------------------

        pub struct WorldVisitor<'r> {
            pub reg: &'r Registry,
        }

        impl<'r, 'de> serde::de::Visitor<'de> for WorldVisitor<'r> {
            type Value = World;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("World")
            }

            fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
            where
                V: serde::de::MapAccess<'de>,
            {
                let mut world = World::default();

                while let Some(key) = map.next_key()? {
                    // TODO: disable duplicate fields

                    match key {
                        Field::Comp => {
                            world.comp =
                                map.next_value_seed(ComponentPoolDeserialize { reg: self.reg })?;
                        }
                    }
                }

                Ok(world)
            }
        }

        // run
        deserializer.deserialize_struct("World", FIELDS, WorldVisitor { reg: self.reg })
    }
}

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
