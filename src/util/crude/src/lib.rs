mod storage;
pub use storage::*;

mod universe;
pub use universe::*;

mod demo {
    use crate::{Component, EntityAllocator, StorageRand, StorageView, StorageViewModify};

    pub struct Position([f32; 3]);

    impl Component for Position {
        type Storage = StorageRand<Self>;
    }

    pub struct Velocity([f32; 3]);

    impl Component for Velocity {
        type Storage = StorageRand<Self>;
    }

    fn spawner_system(
        entities: &mut EntityAllocator,
        positions: &mut StorageView<Position>,
        velocities: &mut StorageView<Velocity>,
    ) {
        let (pos, vel) = (&mut *positions, &mut *velocities).spawn(
            entities,
            "demo object",
            (Position([1., 2., 3.]), Velocity([0., 0., 0.])),
        );

        positions.get_mut(pos).0[1] += 0.1;
        velocities.get_mut(vel).0[1] += 0.1;
    }
}
