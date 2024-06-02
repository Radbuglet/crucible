use std::time::Instant;

use bevy_app::{App, Startup};
use bevy_autoken::{random_component, RandomAccess, RandomAppExt, RandomEntityExt};
use bevy_ecs::system::Commands;

pub struct MyCounter(u64);

random_component!(MyCounter);

fn main() {
    let mut app = App::new();
    app.add_random_component::<MyCounter>();
    app.add_systems(Startup, sys_run);

    app.update();
}

fn sys_run(mut rand: RandomAccess<&mut MyCounter>, mut cmd: Commands) {
    rand.provide(|| {
        let entity = cmd.spawn(()).id();
        let mut entity = entity.insert(MyCounter(0));

        let start = Instant::now();

        for _ in 0..1_000_000 {
            entity.0 += 1;
        }

        dbg!(start.elapsed());
    })
}
