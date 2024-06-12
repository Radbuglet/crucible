use crucible_utils::newtypes::define_index;

use crate::mesh::{QuadMeshLayer, VolumetricMeshLayer};

define_index! {
    pub struct ColliderMaterialId: u32;
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct ColliderMaterial {
    pub id: ColliderMaterialId,
    pub meta: u32,
}

#[derive(Debug, Clone)]
pub enum Collider {
    Transparent,
    Opaque(ColliderMaterial),
    Mesh {
        volumes: VolumetricMeshLayer<ColliderMaterial>,
        extra_quads: QuadMeshLayer<ColliderMaterial>,
    },
}

impl Collider {
    pub fn from_volumes(volumes: VolumetricMeshLayer<ColliderMaterial>) -> Self {
        Self::Mesh {
            volumes,
            extra_quads: QuadMeshLayer::default(),
        }
    }
}
