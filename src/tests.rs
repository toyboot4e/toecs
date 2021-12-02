use crate::{
    res::{Res, ResMut, ResourceMap},
    World,
};

#[test]
fn resource_map() {
    let mut res = ResourceMap::default();
    res.insert(30usize);
    res.insert(-30isize);

    // mutably borrow two resources
    {
        let mut u = res.borrow_mut::<usize>().unwrap();
        let mut i = res.borrow_mut::<isize>().unwrap();
        *u += 5;
        *i += 5;
        assert_eq!(*u, 30 + 5);
        assert_eq!(*i, -30 + 5);
    }

    // insert, remove
    assert_eq!(res.insert(2usize), Some(30 + 5));
    assert_eq!(res.remove::<usize>(), Some(2usize));
}

#[test]
#[should_panic]
fn resource_panic() {
    let mut res = ResourceMap::default();
    res.insert(0usize);
    let _u1 = res.borrow_mut::<usize>().unwrap();
    let _u2 = res.borrow::<usize>().unwrap();
}

#[test]
fn resource_safe() {
    let mut res = ResourceMap::default();
    res.insert(0usize);
    let _u1 = res.borrow::<usize>().unwrap();
    let _u2 = res.borrow::<usize>().unwrap();
}

#[test]
fn resource_system() {
    use crate::sys::System;

    fn system(x: Res<usize>, mut y: ResMut<isize>) {
        *y = *x as isize + *y;
    }

    let mut world = World::default();
    world.res.insert(10usize);
    world.res.insert(30isize);

    unsafe {
        system.run(&world);
    }
    assert_eq!(*world.res.borrow::<isize>().unwrap(), 10 + 30);
}
