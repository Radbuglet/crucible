use bort::{access_cx, CompMut, Entity, HasGlobalManagedTag, Obj};
use crucible_foundation_shared::{
	actor::spatial::Spatial,
	material::{MaterialId, MaterialInfo, MaterialMarker, MaterialRegistry},
};
use crucible_util::mem::hash::FxHashMap;
use typed_glam::glam::Affine3A;

use crate::engine::io::gfx::GfxContext;

use super::renderer::{ActorMeshLayer, ActorRenderer};

// === MeshRegistry === //

#[non_exhaustive]
pub struct MeshMaterialMarker;

impl MaterialMarker for MeshMaterialMarker {}

pub type MeshRegistry = MaterialRegistry<MeshMaterialMarker>;
pub type MeshId = MaterialId<MeshMaterialMarker>;
pub type MeshInfo = MaterialInfo<MeshMaterialMarker>;

// === ActorMeshManager === //

access_cx! {
	pub trait ActorManagerUpdateCx = mut ActorMeshInstance;
	pub trait ActorManagerRenderCx = ref ActorMeshLayer, ref Spatial;
}

#[derive(Debug, Default)]
pub struct ActorMeshManager {
	meshes: FxHashMap<Obj<ActorMeshLayer>, Vec<Obj<Spatial>>>,
}

impl ActorMeshManager {
	pub fn register_instance(
		&mut self,
		target: &mut CompMut<ActorMeshInstance>,
		target_spatial: Obj<Spatial>,
	) {
		let meshes = self.meshes.entry(target.mesh.obj()).or_default();
		target.mesh_index = meshes.len();
		meshes.push(target_spatial);
	}

	pub fn set_instance_mesh(
		&mut self,
		cx: &impl ActorManagerUpdateCx,
		target: &mut CompMut<ActorMeshInstance>,
		target_spatial: Obj<Spatial>,
		mesh: Obj<ActorMeshLayer>,
	) {
		debug_assert_eq!(CompMut::owner(target).entity(), target_spatial.entity());

		// Remove the instance from its old vector
		self.unregister_instance(cx, target);

		// Add the instance to its target vector
		let meshes = self.meshes.entry(mesh).or_default();
		target.mesh = mesh.entity();
		target.mesh_index = meshes.len();
		meshes.push(target_spatial);
	}

	pub fn unregister_instance(
		&mut self,
		cx: &impl ActorManagerUpdateCx,
		target: &mut CompMut<ActorMeshInstance>,
	) {
		let meshes = self.meshes.get_mut(&target.mesh).unwrap();

		meshes.swap_remove(target.mesh_index);

		if let Some(moved) = meshes.get(target.mesh_index) {
			moved.entity().get_mut_s::<ActorMeshInstance>(cx).mesh_index = target.mesh_index;
		}
	}

	pub fn render(
		&self,
		cx: &impl ActorManagerRenderCx,
		gfx: &GfxContext,
		renderer: &mut ActorRenderer,
	) {
		for (mesh, instances) in &self.meshes {
			let mesh = mesh.get_s(cx);
			renderer.push_model(gfx, &mesh);

			for instance in instances {
				renderer.push_model_instance(
					gfx,
					Affine3A::from_translation(instance.get_s(cx).pos().to_glam().as_vec3()),
				);
			}
		}
	}
}

#[derive(Debug)]
pub struct ActorMeshInstance {
	mesh: Entity,
	mesh_index: usize,
}

impl HasGlobalManagedTag for ActorMeshInstance {
	type Component = Self;
}

impl ActorMeshInstance {
	pub fn new(mesh: Entity) -> Self {
		Self {
			mesh,
			mesh_index: 0,
		}
	}
}
