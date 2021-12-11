use crate::{
    comp::ComponentPoolMap,
    ent::EntityPool,
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

#[test]
fn entity_pool() {
    let mut pool = EntityPool::default();
    let e0 = pool.alloc();
    let e1 = pool.alloc();
    let e2 = pool.alloc();

    assert_eq!(pool.iter().collect::<Vec<_>>(), [&e0, &e1, &e2]);

    // deallocation at the boundary
    assert!(pool.dealloc(e2));
    assert!(!pool.dealloc(e2));
    assert_eq!(pool.iter().collect::<Vec<_>>(), [&e0, &e1]);

    // make sure the slot is recycled:
    let e2_new = pool.alloc();
    assert_eq!(e2_new.generation().to_usize(), 2);

    // deallocation at non-boundary
    assert!(pool.dealloc(e1));
    assert!(!pool.dealloc(e1));
    assert_eq!(pool.iter().collect::<Vec<_>>(), [&e0, &e2_new]);
}

#[test]
fn component_pool_map() {
    let mut world = World::default();

    assert!(!world.comp.is_registered::<usize>());
    assert!(!world.comp.register::<usize>());
    assert!(world.comp.is_registered::<usize>());
    assert!(!world.comp.register::<isize>());

    let e0 = world.ents.alloc();
    let e1 = world.ents.alloc();
    let e2 = world.ents.alloc();

    let mut us = world.comp.borrow_mut::<usize>().unwrap();
    assert_eq!(us.insert(e0, 100), None);
    assert_eq!(us.insert(e0, 0), Some(100));
    assert_eq!(us.insert(e1, 1), None);
    assert_eq!(us.insert(e2, 2), None);

    let mut is = world.comp.borrow_mut::<isize>().unwrap();
    assert_eq!(is.insert(e0, -0), None);
    assert_eq!(is.insert(e1, -1), None);
    assert_eq!(is.insert(e2, -2), None);

    assert_eq!(is.swap_remove(e0), Some(-0));
    assert_eq!(is.get(e1), Some(&-1));
    assert_eq!(is.get(e2), Some(&-2));
}

#[test]
fn component_safe() {
    let mut comp = ComponentPoolMap::default();
    comp.register::<usize>();
    let _u1 = comp.borrow::<usize>().unwrap();
    let _u2 = comp.borrow::<usize>().unwrap();
}

#[test]
#[should_panic]
fn component_panic() {
    let mut comp = ComponentPoolMap::default();
    comp.register::<usize>();
    let _u1 = comp.borrow_mut::<usize>().unwrap();
    let _u2 = comp.borrow::<usize>().unwrap();
}

#[test]
fn pointer_stability_after_display() {
    let mut world = World::default();

    world.comp.register::<usize>();
    world.comp.register::<isize>();
    let _e0 = world.ents.alloc();
    let _e1 = world.ents.alloc();

    let res = &world.comp as *const _;
    let ents = &world.ents as *const _;
    let comp = &world.comp as *const _;

    format!("{:?}", world.display());

    let res2 = &world.comp as *const _;
    let ents2 = &world.ents as *const _;
    let comp2 = &world.comp as *const _;

    assert_eq!(res, res2);
    assert_eq!(ents, ents2);
    assert_eq!(comp, comp2);
}
