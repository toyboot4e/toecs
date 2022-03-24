use toecs::{
    world::{
        borrow::{AccessSet, BorrowWorld, GatBorrowWorld},
        comp::{Comp, CompMut, Component, ComponentPoolMap},
        ent::Entity,
        res::{Res, ResMut},
        ComponentSet,
    },
    World,
};

#[derive(Debug, Component)]
struct U(u32);

#[derive(Debug, Component)]
struct I(u32);

#[derive(GatBorrowWorld)]
pub struct CustomBorrow<'w> {
    res_u: Res<'w, U>,
    res_i: ResMut<'w, I>,
    comp_u: Comp<'w, U>,
    comp_i: CompMut<'w, I>,
}

#[test]
fn custom_borrow_access_set() {
    assert_eq!(
        <<CustomBorrow as GatBorrowWorld>::Borrow as BorrowWorld>::accesses(),
            <<(Res<U>,ResMut<I>,Comp<U>,CompMut<I>) as GatBorrowWorld>::Borrow as BorrowWorld>::accesses(),
    );

    let mut world = World::default();

    world.register_set::<(U, I)>();
    world.set_res_set((U(10), I(10)));
    world.spawn((U(20), I(20)));

    fn test_custom_borrow(c: CustomBorrow) {
        //
    }

    world.run(test_custom_borrow);
}

#[derive(ComponentSet)]
pub struct CustomComponentSet {
    u: U,
    i: I,
}

#[test]
fn custom_component_set_derive() {
    let mut world = World::default();

    world.register_set::<(U, I)>();
    let _entity = world.spawn(CustomComponentSet { u: U(10), i: I(20) });

    let u = world.borrow::<Comp<U>>();
    assert_eq!(u.as_slice().len(), 1);
    let i = world.borrow::<Comp<I>>();
    assert_eq!(i.as_slice().len(), 1);
}
