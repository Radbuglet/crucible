use std::ops::ControlFlow;

use bevy_autoken::{random_component, Obj, RandomAccess, RandomEntityExt};
use bevy_ecs::removal_detection::RemovedComponents;
use crucible_math::EntityAabb;
use rustc_hash::FxHashSet;

use super::ColliderMaterial;

// === Components === //

#[derive(Debug, Default)]
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
    material: ColliderMaterial,
}

random_component!(AabbHolder);

impl AabbHolder {
    pub fn new(aabb: EntityAabb, material: ColliderMaterial) -> Self {
        Self {
            store: None,
            aabb,
            material,
        }
    }

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

    pub fn material(&self) -> ColliderMaterial {
        self.material
    }

    pub fn set_material(&mut self, material: ColliderMaterial) {
        self.material = material;
    }
}

// === Systems === //

pub fn sys_unregister_dead_aabbs(
    mut rand: RandomAccess<(&mut AabbStore, &mut AabbHolder)>,
    mut query: RemovedComponents<Obj<AabbHolder>>,
) {
    rand.provide(|| {
        for entity in query.read() {
            entity.get::<AabbHolder>().remove();
        }
    });
}
