use std::ops::ControlFlow;

use bevy_autoken::{random_component, Obj, ObjOwner, RandomAccess, RandomEntityExt};
use bevy_ecs::removal_detection::RemovedComponents;
use crucible_math::EntityAabb;
use rustc_hash::FxHashSet;

// === Components === //

#[derive(Debug)]
pub struct AabbStore {
    // TODO: Use an actual implementation
    colliders: FxHashSet<Obj<AabbHolder>>,
}

random_component!(AabbStore);

impl AabbStore {
    pub fn register(mut self: Obj<Self>, mut aabb: Obj<AabbHolder>) {
        aabb.store = Some(self);
        self.colliders.insert(aabb);
    }

    pub fn scan<B>(
        &self,
        aabb: EntityAabb,
        mut f: impl FnMut(Obj<AabbHolder>) -> ControlFlow<B>,
    ) -> ControlFlow<B> {
        for &collider in &self.colliders {
            if collider.aabb.intersects(aabb) {
                f(collider)?;
            }
        }
        ControlFlow::Continue(())
    }
}

pub struct AabbHolder {
    store: Option<Obj<AabbStore>>,
    aabb: EntityAabb,
}

random_component!(AabbHolder);

impl AabbHolder {
    pub fn aabb(&self) -> EntityAabb {
        self.aabb
    }

    pub fn set_aabb(mut self: Obj<Self>, aabb: EntityAabb) {
        self.aabb = aabb;
    }

    pub fn remove(mut self: Obj<Self>) {
        let Some(mut store) = self.store.take() else {
            return;
        };

        store.colliders.remove(&self);
    }
}

// === Systems === //

pub fn sys_unregister_dead_aabbs(
    mut rand: RandomAccess<(&mut AabbStore, &mut AabbHolder)>,
    mut query: RemovedComponents<ObjOwner<AabbHolder>>,
) {
    rand.provide(|| {
        for entity in query.read() {
            entity.get::<AabbHolder>().remove();
        }
    });
}
