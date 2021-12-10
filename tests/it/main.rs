//! The only integration test "crate"

use toecs::World;

#[test]
fn world_api() {
    let mut world = World::default();

    assert_eq!(world.set_res(1usize), None);
    assert_eq!(world.set_res(100usize), Some(1));
    world.set_res(-100isize);

    world.register::<usize>();
    world.register::<isize>();

    let e1 = world.spawn();
    let e2 = world.spawn();
    let e3 = world.spawn();

    // TODO: insert components with component set on spawn
    world.insert(e1, 10usize);
    world.insert(e1, -10isize);

    world.insert(e2, 20usize);
    world.insert(e2, -20isize);

    world.insert(e3, 30usize);
    world.insert(e3, -30isize);

    assert_eq!(world.remove::<isize>(e1), Some(-10));

    // TODO: despawn `e2`
    // assert!(world.despawn(e2));
    // assert!(!world.despawn(e2));
    // let e2 = world.spawn();
    // assert_eq!(world.entities().collect::<Vec<_>>(), [&e1, &e3, &e2]);

    // TODO: iterate through components

    // $ cargo test -- --nocapture
    println!("{:#?}", world);
}
