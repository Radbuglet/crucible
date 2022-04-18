use geode::exec::event::event_type;
use geode::exec::obj::Obj;

fn main() {
	let root = make_engine_root();
	let mut data = 21;
	dbg!(data);
	root.fire_event::<MyEvent>(MyEvent {
		some_data: &mut data,
	});
	dbg!(data);
}

fn make_engine_root() -> Obj {
	let mut root = Obj::new();

	root.add_event_handler::<MyEvent>(|event| {
		*event.some_data = 42;
	});

	root
}

event_type! {
	struct MyEvent<'a> {
		some_data: &'a mut u32,
	}
}
