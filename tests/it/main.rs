//! The only integration test "crate"

use toecs::{
    query::Iter,
    sys::System,
    world::{
        comp::{Comp, CompMut, Component},
        res::{Res, ResMut},
    },
    World,
};

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct U(usize);
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct U32(u32);
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct I(isize);
#[derive(Component, Debug, Clone, Copy, PartialEq, PartialOrd)]
struct F(f32);

#[test]
fn world_api() {
    let mut world = World::default();

    // resource
    assert_eq!(world.set_res(U(1)), None);
    assert_eq!(world.set_res(U(100)), Some(U(1)));
    world.set_res(I(-100));

    // components
    world.register_set::<(U, I)>();

    let e1 = world.spawn_empty();
    world.insert_set(e1, (U(10), I(-10)));
    let e2 = world.spawn((U(20), I(-20)));
    let e3 = world.spawn((U(30), I(-30)));

    assert_eq!(world.remove::<I>(e1), Some(I(-10)));

    assert!(world.despawn(e2));
    assert!(!world.despawn(e2));

    let e2 = world.spawn_empty();
    assert_eq!(world.entities().iter().collect::<Vec<_>>(), [&e1, &e3, &e2]);

    {
        let us = world.comp::<U>();
        assert_eq!((&us).iter().collect::<Vec<_>>(), [&U(10), &U(30)]);
    }

    world.remove_set::<(U, I)>(e1);
    assert_eq!(world.comp::<U>().get(e1), None);
    assert_eq!(world.comp::<I>().get(e1), None);

    // $ cargo test -- --nocapture
    println!("{:#?}", world);
    println!("{:#?}", world.display());
}

#[test]
fn single_iter() {
    let mut world = World::default();

    world.register_set::<(U, I)>();

    let e1 = world.spawn((U(10), I(-10)));
    let e2 = world.spawn((U(20), I(-20)));
    let e3 = world.spawn((U(30), I(-30)));

    fn add_system(mut us: CompMut<U>) {
        for u in (&mut us).iter() {
            u.0 += 10;
        }
    }

    unsafe {
        add_system.run(&mut world);
    }

    assert_eq!(
        world
            .comp::<U>()
            .iter()
            .entities()
            .map(|(e, x)| (e, *x))
            .unzip(),
        (vec![e1, e2, e3], vec![U(10 + 10), U(10 + 20), U(10 + 30)])
    );
}

#[test]
fn sparse_iter() {
    let mut world = World::default();

    world.register_set::<(U, I)>();

    let e1 = world.spawn((U(10), I(-10)));
    let e2 = world.spawn((U(20), I(-20)));
    let e3 = world.spawn((U(30), I(-30)));

    fn add_system(mut us: CompMut<U>, is: Comp<I>) {
        for (u, i) in (&mut us, &is).iter() {
            u.0 += -i.0 as usize + 10;
        }
    }

    unsafe {
        add_system.run(&mut world);
    }

    assert_eq!(
        world
            .comp::<U>()
            .iter()
            .entities()
            .map(|(e, x)| (e, *x))
            .unzip(),
        (
            vec![e1, e2, e3],
            vec![U(10 + 10 + 10), U(20 + 20 + 10), U(30 + 30 + 10)]
        )
    );

    world.register::<U32>();
    let e = world.spawn((U(10), I(20), U32(30)));

    fn triple(mut u: CompMut<U>, i: Comp<I>, u2: Comp<U32>) {
        for (u, i, u2) in (&mut u, &i, &u2).iter() {
            u.0 += i.0 as usize + u2.0 as usize;
        }
    }

    unsafe {
        triple.run(&world);
    }

    assert_eq!(world.comp::<U>().get(e), Some(&(U(10 + 20 + 30))));
}

#[test]
fn sparse_iter_holes() {
    let mut world = World::default();

    world.register_set::<(U, I, F)>();

    let ui_ = world.spawn((U(10), I(10)));
    let u_f = world.spawn((U(20), F(20.0)));
    let uif = world.spawn((U(30), I(30), F(30.0)));

    let u = world.comp::<U>();
    let i = world.comp::<I>();
    let f = world.comp::<F>();

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

#[test]
fn borrow_type_inference() {
    let mut world = World::default();

    world.set_res_set((U(0), I(0)));
    world.register_set::<(U, I)>();

    {
        let _: Res<U> = world.borrow();
        let _: ResMut<I> = world.borrow();
    }

    {
        let _: Comp<U> = world.borrow();
        let _: CompMut<I> = world.borrow();
    }

    let (_, _, _, _): (Res<U>, Res<I>, Comp<U>, CompMut<I>) = world.borrow();
}

#[test]
fn run_exclusive() {
    let mut world = World::default();
    world.set_res_set((U(0), I(0), F(0.0)));

    fn sys(_u: Res<U>, _i: Res<I>, _f: Res<F>) {}
    fn ex_sys(_world: &mut World) {}

    world.run_ex(sys);
    world.run_ex(|_: &mut World| {});
    world.run_ex(ex_sys);

    use toecs::sys::erased::{ExclusiveResultSystem, ResultSystem};
    unsafe {
        let _ = (|_: &mut World| {}).run_as_result_ex(&mut world);
        let _ = sys.run_as_result(&world);
        let _ = sys.run_as_result_ex(&mut world);
        let _ = ex_sys.run_as_result_ex(&mut world);
    }
}

#[test]
fn run_with_args() {
    let mut world = World::default();
    world.set_res_set((U(0), I(0), F(0.0)));

    fn sys(arg: u32, _u: Res<U>, _i: Res<I>, _f: Res<F>) -> u32 {
        arg
    }

    assert_eq!(world.run_arg(10u32, sys), 10);
}

#[test]
fn component_set_definition() {
    let mut world = World::default();
    world.register_set::<(U, I, F)>();

    let entity = world.spawn_empty();

    // a component is a component set
    world.insert_set(entity, U(0));

    // set of components is a component set
    world.insert_set(entity, (I(1), U(1)));

    // set of component set is a component set
    world.insert_set(entity, ((U(2), I(2), F(2.2)), (U(3), I(3))));
}

// #[test]
// fn parallel() -> SystemResult {
//     use toecs::res::ResMut;
//
//     let mut world = World::default();
//
//     // resources
//
//     world.set_res_set((10usize, 20isize, 30.0f32));
//
//     fn res_ui_(u: Res<usize>, mut i: ResMut<isize>) {
//         *i += *u as isize;
//     }
//
//     fn res_u_f(u: Res<usize>, mut f: ResMut<f32>) {
//         *f += *u as f32;
//     }
//
//     world.run_par(res_ui_)?;
//
//     // components
//
//     world.set_res_set((0usize, 0isize, 0.0f32));
//
//     world.register_set::<(usize, isize, f32)>();
//
//     fn sum_u(mut res: ResMut<usize>, u: Comp<usize>) {
//         *res = u.iter().sum();
//     }
//
//     fn sum_i(mut res: ResMut<isize>, u: Comp<isize>) {
//         *res = u.iter().sum();
//     }
//
//     fn sum_f(mut res: ResMut<f32>, u: Comp<f32>) {
//         *res = u.iter().sum();
//     }
//
//     world.spawn((0usize, 0isize, 0.0f32));
//     world.spawn((1usize, 1isize, 1.0f32));
//     world.spawn((2usize, 2isize, 2.0f32));
//
//     // world.run_par(sum_u);
//
//     Ok(())
// }
