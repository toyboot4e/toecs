use toecs::{
    world::{
        borrow::{AccessSet, AutoFetchImpl, AutoFetch},
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

#[derive(AutoFetch)]
pub struct CustomFetch<'w> {
    _res_u: Res<'w, U>,
    _res_i: ResMut<'w, I>,
    _comp_u: Comp<'w, U>,
    _comp_i: CompMut<'w, I>,
}

#[test]
fn custom_borrow_access_set() {
    assert_eq!(
        <<CustomFetch as AutoFetch>::Fetch as AutoFetchImpl>::accesses(),
            <<(Res<U>,ResMut<I>,Comp<U>,CompMut<I>) as AutoFetch>::Fetch as AutoFetchImpl>::accesses(),
    );

    let mut world = World::default();

    world.register_set::<(U, I)>();
    world.set_res_set((U(10), I(10)));
    world.spawn((U(20), I(20)));

    fn test_custom_borrow(_c: CustomFetch) {
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

    let u = world.fetch::<Comp<U>>();
    assert_eq!(u.as_slice().len(), 1);
    let i = world.fetch::<Comp<I>>();
    assert_eq!(i.as_slice().len(), 1);
}
