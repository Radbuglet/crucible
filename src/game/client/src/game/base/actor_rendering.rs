use bort::{alias, cx, query, scope, BehaviorRegistry, Cx, GlobalTag, Obj, OwnedObj, VecEventList};
use crucible_foundation_client::{
	engine::{assets::AssetManager, io::gfx::GfxContext},
	gfx::actor::{
		manager::{MeshInstance, MeshManager, MeshRegistry},
		pipeline::ActorRenderingUniforms,
		renderer::{ActorMeshLayer, ActorRenderer},
	},
};
use crucible_foundation_shared::{
	actor::{manager::ActorSpawned, spatial::Spatial},
	math::{Aabb3, Color3},
};
use crucible_util::debug::type_id::NamedTypeId;
use typed_glam::glam::Vec3;

use super::behaviors::{InitGame, UpdateHandleEarlyEvents};

alias! {
	let asset_mgr: AssetManager;
	let gfx: GfxContext;
	let mesh_manager: MeshManager;
}

pub fn register(bhv: &mut BehaviorRegistry) {
	bhv.register_cx(
		[],
		InitGame::new(|_bhv, s, scene, engine| {
			scope! {
				use let s,
				inject { mut asset_mgr = engine, ref gfx = engine }
			}

			scene.add(ActorRenderingUniforms::new(asset_mgr, gfx));
			scene.add(ActorRenderer::default());
			scene.add(MeshManager::default());

			// Create a registry for all our meshes
			let mut registry = MeshRegistry::default();

			let mesh_offset = Vec3::Z * -10.0;
			let mesh = ActorMeshLayer::new()
				.with_cube(
					Aabb3::from_origin_size(
						Vec3::X * -0.3,
						Vec3::new(0.45, 0.95, 0.45),
						Vec3::new(0.5, 0.0, 0.5),
					)
					.offset_by(mesh_offset),
					Color3::new(0.5, 0.5, 0.5),
				)
				.with_cube(
					Aabb3::from_origin_size(
						Vec3::X * 0.3,
						Vec3::new(0.45, 0.95, 0.45),
						Vec3::new(0.5, 0.0, 0.5),
					)
					.offset_by(mesh_offset),
					Color3::new(0.5, 0.5, 0.5),
				)
				.with_cube(
					Aabb3::from_origin_size(
						Vec3::Y * 0.95,
						Vec3::splat(1.2),
						Vec3::new(0.5, 0.0, 0.5),
					)
					.offset_by(mesh_offset),
					Color3::new(0.5, 0.5, 0.5),
				)
				.with_cube(
					Aabb3::from_origin_size(
						Vec3::new(-0.5, 0.95 + 0.6, 0.6),
						Vec3::splat(0.3),
						Vec3::new(0.5, 0.5, 0.5),
					)
					.offset_by(mesh_offset),
					Color3::new(0.1, 0.1, 1.0),
				)
				.with_cube(
					Aabb3::from_origin_size(
						Vec3::new(0.5, 0.95 + 0.6, 0.6),
						Vec3::splat(0.3),
						Vec3::new(0.5, 0.5, 0.5),
					)
					.offset_by(mesh_offset),
					Color3::new(0.1, 0.1, 1.0),
				);

			registry.register(
				"crucible:glagglesnoy",
				OwnedObj::new(mesh)
					.with_debug_label("glagglesnoy mesh")
					.owned_entity(),
			);

			scene.add(registry);
		}),
	);

	bhv.register_cx(
		([NamedTypeId::of::<ActorSpawned>()], []),
		UpdateHandleEarlyEvents::new(|_bhv, s, events, scene| {
			scope! {
				use let s,
				access cx: Cx<&mut MeshInstance, &VecEventList<ActorSpawned>>,
				inject { mut mesh_manager = scene }
			}

			query! {
				for (
					_ev in events.get_s::<ActorSpawned>(cx!(cx));
					@me,
					omut instance in GlobalTag::<MeshInstance>,
					slot spatial in GlobalTag::<Spatial>,
				) {
					#[clippy::accept_danger(direct_mesh_management, reason = "this is that system!")]
					mesh_manager.register_instance(&mut instance, Obj::from_raw_parts(me, spatial));
				}
			}
		}),
	);
}
