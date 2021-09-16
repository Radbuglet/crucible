use core::foundation::provider::{
	Component, LazyComponent, LazyProviderExt, MultiProvider, ProviderExt,
};

fn main() {
	let engine = MultiProvider((
		MultiProvider::<(LazyComponent<u32>, LazyComponent<i32>)>::default(),
		MultiProvider((Component(4.2f32),)),
	));

	println!("f32: {:?}", engine.get::<f32>());
	println!("u32: {:?}", engine.try_get::<u32>());
	engine.init(42u32);
	println!("u32: {:?}", engine.try_get::<u32>());
}
