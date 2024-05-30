use std::{
    collections::VecDeque,
    ops::ControlFlow,
    time::{Duration, Instant},
};

use bevy_autoken::{random_component, Obj, RandomAccess, RandomEntityExt};
use bevy_ecs::event::EventReader;

use super::{ChunkVoxelData, WorldChunkCreated, WorldVoxelData};

// === ChunkQueue === //

#[derive(Debug, Default)]
pub struct ChunkQueue {
    queue: VecDeque<(bool, Obj<ChunkVoxelData>)>,
}

impl ChunkQueue {
    pub fn push_many(&mut self, iter: impl IntoIterator<Item = Obj<ChunkVoxelData>>) {
        let my_color = self.queue.back().map_or(false, |last| !last.0);

        self.queue
            .extend(iter.into_iter().map(|chunk| (my_color, chunk)));
    }

    pub fn push(&mut self, chunk: Obj<ChunkVoxelData>) {
        self.push_many([chunk]);
    }

    pub fn process<B>(
        &mut self,
        limit: Option<Duration>,
        mut f: impl FnMut(Obj<ChunkVoxelData>) -> ControlFlow<B>,
    ) -> ControlFlow<B> {
        let Some(&(mut color, _)) = self.queue.front() else {
            return ControlFlow::Continue(());
        };

        let start = Instant::now();

        loop {
            while self
                .queue
                .front()
                .is_some_and(|&(other_color, _)| color == other_color)
            {
                f(self.queue.pop_front().unwrap().1)?;
            }

            color = !color;

            if limit.is_some_and(|limit| start.elapsed() > limit) {
                break;
            }
        }

        ControlFlow::Continue(())
    }
}

// === Components === //

#[derive(Debug, Default)]
pub struct ChunkLoadQueue(pub ChunkQueue);

random_component!(ChunkLoadQueue);

// === Systems === //

pub fn sys_add_new_chunks_to_load_queue(
    mut rand: RandomAccess<(&mut ChunkLoadQueue, &WorldVoxelData)>,
    mut events: EventReader<WorldChunkCreated>,
) {
    rand.provide(|| {
        for &event in events.read() {
            let Some(mut queue) = event.world.entity().try_get::<ChunkLoadQueue>() else {
                continue;
            };

            queue.0.push(event.chunk);
        }
    })
}