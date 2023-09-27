use bort::{alias, query, scope, BehaviorRegistry, Cx, GlobalTag, Obj, OwnedObj};
use crucible_foundation_client::{
	engine::{assets::AssetManager, io::gfx::GfxContext},
	gfx::actor::{
		manager::{ActorMeshInstance, ActorMeshManager, MeshRegistry},
		pipeline::ActorRenderingUniforms,
		renderer::{ActorMeshLayer, ActorRenderer},
	},
};
use crucible_foundation_shared::{
	actor::spatial::Spatial,
	math::{Aabb3, Color3},
};
use typed_glam::glam::Vec3;

use super::entry::{ActorSpawnedInGameBehavior, GameInitRegistry, GameSceneInitBehavior};

// === Behaviors === //

alias! {
	let asset_mgr: AssetManager;
	let gfx: GfxContext;
	let mesh_manager: ActorMeshManager;
}

pub fn register(bhv: &mut BehaviorRegistry) {
	bhv.register_combined(make_actor_spawn_handler());
}

fn make_actor_spawn_handler() -> ActorSpawnedInGameBehavior {
	ActorSpawnedInGameBehavior::new(|_bhv, s, on_spawned, scene| {
		scope! {
			use let s,
			access cx: Cx<&mut ActorMeshInstance>,
			inject { mut mesh_manager = scene }
		}

		query! {
			for (
				_ev in on_spawned;
				@me,
				omut instance in GlobalTag::<ActorMeshInstance>,
				slot spatial in GlobalTag::<Spatial>,
			) {
				mesh_manager.register_instance(&mut instance, Obj::from_raw_parts(me, spatial));
			}
		}
	})
}

pub fn push_plugins(pm: &mut GameInitRegistry) {
	pm.register(
		[],
		GameSceneInitBehavior::new(|_bhv, s, scene, engine| {
			scope! {
				use let s,
				inject { mut asset_mgr = engine, ref gfx = engine }
			}

			scene.add(ActorRenderingUniforms::new(asset_mgr, gfx));
			scene.add(ActorRenderer::default());
			scene.add(ActorMeshManager::default());

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
}
