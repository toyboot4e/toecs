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

#[test]
fn sparse_set() {
    use crate::sparse::*;

    let mut set = SparseSet::<usize>::default();

    // Indices are allocatead manually:
    let i0 = SparseIndex::initial(RawSparseIndex(0));
    let i1 = SparseIndex::initial(RawSparseIndex(1));
    let i2 = SparseIndex::initial(RawSparseIndex(2));

    assert_eq!(set.insert(i0, 0), None);
    assert_eq!(set.insert(i1, 1), None);
    assert_eq!(set.insert(i2, 2), None);

    assert_eq!(set.get(i0), Some(&0));
    assert_eq!(set.get(i1), Some(&1));
    assert_eq!(set.get(i2), Some(&2));

    let i1_new = i1.increment_generation();
    assert_eq!(set.insert(i1_new, 100), Some(1));

    assert_eq!(set.get(i0), Some(&0));
    // old index is invalidated
    assert_eq!(set.get(i1), None);
    assert_eq!(set.get(i1_new), Some(&100));
    assert_eq!(set.get(i2), Some(&2));

    assert_eq!(set.swap_remove(i0), Some(0));

    for (i, x) in set.iter_with_index() {
        match i {
            _ if *i == i1_new => assert_eq!(x, &100),
            _ if *i == i2 => assert_eq!(x, &2),
            _ => unreachable!(),
        }
    }
}
