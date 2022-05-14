use geode::prelude::*;
use std::cell::Cell;

fn main() {
	let root = make_engine_root();

	root.get::<dyn TickHandler>()
		.on_tick(&mut ObjCx::with_root(&root));

	root.borrow_mut::<MyService>().count();
	root.borrow_mut::<MyService>().count();

	let mut st_example = StObj::new();
	st_example.add_rw(Cell::new(4u32));
	assert_eq!(*st_example.borrow_mut::<Cell<u32>>().get_mut(), 4);
}

fn make_engine_root() -> Obj {
	let mut root = Obj::new();

	root.add_rw(MyService::default());
	root.add_alias(
		|obj: &mut ObjCx| {
			obj.inject(|mut service: AMut<MyService>| {
				service.count();
				service.count();
			});

			dbg!(obj.path());
		},
		typed_key::<dyn TickHandler>(),
	);

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
	trait TickHandler::on_tick(&self, cx: &mut ObjCx);
}
