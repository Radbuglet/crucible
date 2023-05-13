use std::cell::{Ref, RefMut};

use bort::Entity;
use crucible_util::lang::delegate::{FuncMethodInjectorMut, FuncMethodInjectorRef};

#[derive(Debug, Copy, Clone, Default)]
pub struct ComponentInjector;

impl<T: 'static> FuncMethodInjectorRef<T> for ComponentInjector {
	type Guard<'a> = Ref<'static, T>;
	type Injector = for<'a> fn(&'a (), &mut Entity) -> Self::Guard<'a>;

	const INJECTOR: Self::Injector = |_, me| me.get();
}

impl<T: 'static> FuncMethodInjectorMut<T> for ComponentInjector {
	type Guard<'a> = RefMut<'static, T>;
	type Injector = for<'a> fn(&'a (), &mut Entity) -> Self::Guard<'a>;

	const INJECTOR: Self::Injector = |_, me| me.get_mut();
}
