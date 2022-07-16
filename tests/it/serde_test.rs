use serde::{de::DeserializeSeed, Deserialize, Serialize};
use toecs::{prelude::*, serde::Registry};

#[derive(Debug, Component, Serialize, Deserialize)]
struct Pos {
    x: i32,
    y: i32,
}

#[derive(Debug, Component, Serialize, Deserialize)]
struct NonSerde(usize);

#[test]
fn test_serde_entity_id() {
    let mut world = World::default();
    let entity = world.spawn_empty();

    let ron = ron::to_string(&entity).unwrap();
    let e: Entity = ron::from_str(&ron).unwrap();

    assert_eq!(entity, e);
}

#[test]
fn test_serde_world() {
    let mut world = World::default();

    world.set_res(Registry::default());

    // TODO: make up some comfortable API for world / type registration
    world.register_set::<(Pos, NonSerde)>();
    {
        let mut reg = world.res_mut::<Registry>();
        reg.register::<Pos>();
        // FIXME: separate Resource/Component type
        // reg.register_res::<Pos>();
    }

    world.set_res(Pos { x: 100, y: 100 });

    let _e0 = world.spawn(Pos { x: 10, y: 10 });
    let _e1 = world.spawn((Pos { x: 11, y: 11 }, NonSerde(5)));

    let config = ron::ser::PrettyConfig::default()
        .decimal_floats(true)
        .indentor("  ".to_string())
        .new_line("\n".to_string());
    let ron = ron::ser::to_string_pretty(&world.as_serialize(), config).unwrap();

    println!("serialize: {}", ron);

    let mut deserializer = ron::de::Deserializer::from_str(&ron).unwrap();
    let mut world = world
        .res::<Registry>()
        .as_deserialize()
        .deserialize(&mut deserializer)
        .unwrap();

    println!("deserialize: {:?}", world.display());
}
