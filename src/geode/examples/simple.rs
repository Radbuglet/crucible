use geode::exec::obj::{event_trait, typed_key, Obj, ObjLike};

fn main() {
	let root = make_engine_root();
	let mut value = 42;

	root.get::<dyn TickHandler<u32, i32>>()
		.on_tick(&&root, &mut value);

	dbg!(value);

	root.borrow_mut::<MyService>().count();
	root.borrow_mut::<MyService>().count();
}

fn make_engine_root() -> Obj {
	let mut root = Obj::new();

	root.add_as(
		typed_key(),
		|obj: &&Obj, value: &mut u32| {
			dbg!(obj.try_get::<dyn TickHandler<u32, i32>>().is_ok());
			dbg!(obj.try_get::<dyn TickHandler<i32, i32>>().is_err());
			dbg!(*value);
			*value = 12;
		},
		typed_key::<dyn TickHandler<u32, i32>>(),
	);

	root.add_rw(MyService::default());

	root
}

#[derive(Debug, Default)]
struct MyService {
	counter: u32,
}

impl MyService {
	fn count(&mut self) {
		self.counter += 1;
		dbg!(self.counter);
	}
}

event_trait! {
	trait TickHandler::<T, A>::on_tick<'a, 'b>(&self, obj: &'a &'b Obj, value: &mut T);
}
