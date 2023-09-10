use bort::{access_cx, CompMut, Entity, Obj};
use crucible_foundation_shared::actor::spatial::Spatial;
use crucible_util::mem::hash::FxHashMap;
use typed_glam::glam::Affine3A;

use crate::engine::io::gfx::GfxContext;

use super::renderer::{ActorMeshLayer, ActorRenderer};

access_cx! {
	pub trait ActorManagerUpdateCx = mut ActorMeshInstance;
	pub trait ActorManagerRenderCx = ref ActorMeshLayer, ref Spatial;
}

#[derive(Debug, Default)]
pub struct ActorMeshManager {
	meshes: FxHashMap<Obj<ActorMeshLayer>, Vec<Obj<Spatial>>>,
}

impl ActorMeshManager {
	pub fn set_instance_mesh(
		&mut self,
		cx: &impl ActorManagerUpdateCx,
		target: &mut CompMut<ActorMeshInstance>,
		target_spatial: Obj<Spatial>,
		mesh: Obj<ActorMeshLayer>,
	) {
		debug_assert_eq!(CompMut::owner(target).entity(), target_spatial.entity());

		// Remove the instance from its old vector
		self.remove_instance(cx, target);

		// Add the instance to its target vector
		let meshes = self.meshes.entry(mesh).or_default();
		target.mesh = Some(mesh.entity());
		target.mesh_index = meshes.len();
		meshes.push(target_spatial);
	}

	pub fn remove_instance(
		&mut self,
		cx: &impl ActorManagerUpdateCx,
		target: &mut CompMut<ActorMeshInstance>,
	) {
		let meshes = self.meshes.get_mut(&target.mesh.take().unwrap()).unwrap();

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
	mesh: Option<Entity>,
	mesh_index: usize,
}
