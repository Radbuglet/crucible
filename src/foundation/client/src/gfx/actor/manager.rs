use bort::{cx, CompMut, Cx, Entity, HasGlobalManagedTag, Obj};
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

type MeshManagerUpdateCx<'a> = Cx<&'a mut MeshInstance>;
type MeshManagerRenderCx<'a> = Cx<&'a ActorMeshLayer, &'a Spatial>;

#[derive(Debug, Default)]
pub struct MeshManager {
	meshes: FxHashMap<Obj<ActorMeshLayer>, Vec<Obj<Spatial>>>,
}

impl MeshManager {
	#[clippy::dangerous(direct_mesh_management, reason = "spawn the actor instead")]
	pub fn register_instance(
		&mut self,
		target: &mut CompMut<MeshInstance>,
		target_spatial: Obj<Spatial>,
	) {
		let meshes = self.meshes.entry(target.mesh.obj()).or_default();
		target.mesh_index = meshes.len();
		meshes.push(target_spatial);
	}

	#[clippy::dangerous(direct_mesh_management, reason = "send an event instead")]
	pub fn set_instance_mesh(
		&mut self,
		cx: MeshManagerUpdateCx<'_>,
		target: &mut CompMut<MeshInstance>,
		target_spatial: Obj<Spatial>,
		mesh: Obj<ActorMeshLayer>,
	) {
		debug_assert_eq!(CompMut::owner(target).entity(), target_spatial.entity());

		// Remove the instance from its old vector
		self.unregister_instance(cx!(cx), target);

		// Add the instance to its target vector
		let meshes = self.meshes.entry(mesh).or_default();
		target.mesh = mesh.entity();
		target.mesh_index = meshes.len();
		meshes.push(target_spatial);
	}

	#[clippy::dangerous(direct_mesh_management, reason = "despawn the actor instead")]
	pub fn unregister_instance(
		&mut self,
		cx: MeshManagerUpdateCx<'_>,
		target: &mut CompMut<MeshInstance>,
	) {
		let meshes = self.meshes.get_mut(&target.mesh).unwrap();

		meshes.swap_remove(target.mesh_index);

		if let Some(moved) = meshes.get(target.mesh_index) {
			moved.entity().get_mut_s::<MeshInstance>(cx).mesh_index = target.mesh_index;
		}
	}

	pub fn render(
		&self,
		cx: MeshManagerRenderCx<'_>,
		gfx: &GfxContext,
		renderer: &mut ActorRenderer,
	) {
		for (mesh, instances) in &self.meshes {
			let mesh = mesh.get_s(cx!(cx));
			renderer.push_model(gfx, &mesh);

			for instance in instances {
				renderer.push_model_instance(
					gfx,
					Affine3A::from_translation(instance.get_s(cx!(cx)).pos.to_glam().as_vec3()),
				);
			}
		}
	}
}

#[derive(Debug)]
pub struct MeshInstance {
	mesh: Entity,
	mesh_index: usize,
}

impl HasGlobalManagedTag for MeshInstance {
	type Component = Self;
}

impl MeshInstance {
	pub fn new(mesh: Entity) -> Self {
		Self {
			mesh,
			mesh_index: 0,
		}
	}
}
