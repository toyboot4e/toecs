//! The only integration test "crate"

use toecs::World;

#[test]
fn world_api() {
    let mut world = World::default();

    assert_eq!(world.set_res(1usize), None);
    assert_eq!(world.set_res(100usize), Some(1));
    world.set_res(-100isize);

    world.register_many::<(usize, isize)>();

    let e1 = world.spawn_empty();
    world.insert_many(e1, (10usize, -10isize));
    let e2 = world.spawn((20usize, -20isize));
    let e3 = world.spawn((30usize, -30isize));

    assert_eq!(world.remove::<isize>(e1), Some(-10));

    assert!(world.despawn(e2));
    assert!(!world.despawn(e2));

    let e2 = world.spawn_empty();
    assert_eq!(world.entities().iter().collect::<Vec<_>>(), [&e1, &e3, &e2]);

    // TODO: iterate through components
    world.remove_many::<(usize, isize)>(e1);
    assert_eq!(world.comp::<usize>().get(e1), None);
    assert_eq!(world.comp::<isize>().get(e1), None);

    // $ cargo test -- --nocapture
    println!("{:#?}", world);
    println!("{:#?}", world.display());
}
