//! The only integration test "crate"

use toecs::{
    comp::{Comp, CompMut},
    query::Iter,
    res::Res,
    sys::System,
    World,
};

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

    {
        let us = world.comp::<usize>();
        assert_eq!((&us).iter().collect::<Vec<_>>(), [&10, &30]);
    }

    world.remove_many::<(usize, isize)>(e1);
    assert_eq!(world.comp::<usize>().get(e1), None);
    assert_eq!(world.comp::<isize>().get(e1), None);

    // $ cargo test -- --nocapture
    println!("{:#?}", world);
    println!("{:#?}", world.display());
}

#[test]
fn single_iter() {
    let mut world = World::default();

    world.register_many::<(usize, isize)>();

    world.set_res(10usize);

    let e1 = world.spawn((10usize, -10isize));
    let e2 = world.spawn((20usize, -20isize));
    let e3 = world.spawn((30usize, -30isize));

    fn add_system(mut us: CompMut<usize>, add: Res<usize>) {
        for u in (&mut us).iter() {
            *u += *add;
        }
    }

    unsafe {
        add_system.run(&mut world);
    }

    assert_eq!(
        world
            .comp::<usize>()
            .iter()
            .entities()
            .map(|(e, x)| (e, *x))
            .unzip(),
        (vec![e1, e2, e3], vec![10 + 10, 10 + 20, 10 + 30])
    );
}

#[test]
fn sparse_iter() {
    let mut world = World::default();

    world.register_many::<(usize, isize)>();

    world.set_res(10usize);

    let e1 = world.spawn((10usize, -10isize));
    let e2 = world.spawn((20usize, -20isize));
    let e3 = world.spawn((30usize, -30isize));

    fn add_system(mut us: CompMut<usize>, is: Comp<isize>, add: Res<usize>) {
        for (u, i) in (&mut us, &is).iter() {
            *u += (-*i) as usize + *add;
        }
    }

    unsafe {
        add_system.run(&mut world);
    }

    assert_eq!(
        world
            .comp::<usize>()
            .iter()
            .entities()
            .map(|(e, x)| (e, *x))
            .unzip(),
        (
            vec![e1, e2, e3],
            vec![10 + 10 + 10, 20 + 20 + 10, 30 + 30 + 10]
        )
    );

    world.register::<u32>();
    let e = world.spawn((10usize, 20isize, 30u32));

    fn triple(mut u: CompMut<usize>, i: Comp<isize>, u2: Comp<u32>) {
        for (u, i, u2) in (&mut u, &i, &u2).iter() {
            *u += *i as usize + *u2 as usize;
        }
    }

    unsafe {
        triple.run(&world);
    }

    assert_eq!(world.comp::<usize>().get(e), Some(&(10 + 20 + 30)));
}

#[test]
fn sparse_iter_holes() {
    let mut world = World::default();

    world.register_many::<(usize, isize, f32)>();

    let ui_ = world.spawn((10usize, 10isize));
    let u_f = world.spawn((20usize, 20.0f32));
    let uif = world.spawn((30usize, 30isize, 30.0f32));

    let u = world.comp::<usize>();
    let i = world.comp::<isize>();
    let f = world.comp::<f32>();

    // uif
    assert_eq!(
        (&u, &i, &f).iter().entities().collect::<Vec<_>>(),
        [(
            uif,
            (
                u.get(uif).unwrap(),
                i.get(uif).unwrap(),
                f.get(uif).unwrap(),
            )
        ),],
    );

    // ui_
    assert_eq!(
        (&u, &i).iter().entities().collect::<Vec<_>>(),
        [
            (ui_, (u.get(ui_).unwrap(), i.get(ui_).unwrap())),
            (uif, (u.get(uif).unwrap(), i.get(uif).unwrap())),
        ],
    );

    // u_f
    assert_eq!(
        (&u, &f).iter().entities().collect::<Vec<_>>(),
        [
            (u_f, (u.get(u_f).unwrap(), f.get(u_f).unwrap())),
            (uif, (u.get(uif).unwrap(), f.get(uif).unwrap())),
        ],
    );

    // _if
    assert_eq!(
        (&i, &f).iter().entities().collect::<Vec<_>>(),
        [(uif, (i.get(uif).unwrap(), f.get(uif).unwrap())),],
    );
}
