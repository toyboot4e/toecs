use toecs::{
    comp::{Comp, CompMut, Component},
    res::{Res, ResMut},
    sys::{AccessSet, BorrowWorld, GatBorrowWorld},
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

    world.register_many::<(U, I)>();
    world.set_res_many((U(10), I(10)));
    world.spawn((U(20), I(20)));

    fn test_custom_borrow(c: CustomBorrow) {
        //
    }

    world.run(test_custom_borrow);
}
