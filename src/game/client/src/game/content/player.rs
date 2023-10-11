use std::f64::consts::{PI, TAU};

use bort::{
	alias, cx, query, scope, storage, BehaviorRegistry, Cx, EventTarget, GlobalTag,
	HasGlobalManagedTag, OwnedEntity,
};

use crucible_foundation_client::{
	engine::gfx::camera::CameraSettings,
	gfx::{
		actor::manager::{ActorMeshInstance, MeshRegistry},
		ui::materials::sdf_rect::SdfRectImmBrushExt,
	},
};
use crucible_foundation_shared::{
	actor::{
		collider::{Collider, TrackedCollider},
		kinematic::KinematicObject,
		manager::{ActorManager, ActorSpawned},
		spatial::Spatial,
	},
	humanoid::{
		health::HealthState,
		inventory::{InventoryData, InventoryUpdated},
		item::{ItemMaterialRegistry, ItemStackBase},
	},
	math::{
		kinematic::{tick_friction_coef_to_coef_qty, MC_TICKS_TO_SECS, MC_TICKS_TO_SECS_SQUARED},
		Aabb2, Angle3D, Angle3DExt, BlockFace, Color4, EntityAabb, EntityVec,
	},
	voxel::{
		collision::{MaterialColliderDescriptor, RayCast},
		data::{
			Block, BlockMaterialId, BlockMaterialRegistry, ChunkVoxelData, EntityVoxelPointer,
			WorldVoxelData,
		},
	},
};
use crucible_util::{lang::iter::ContextualIter, use_generator};
use typed_glam::{
	glam::{DVec3, Vec2, Vec3, Vec3Swizzles},
	traits::NumericVector,
};
use winit::event::{MouseButton, VirtualKeyCode};

use crate::game::base::{
	behaviors::{
		ActorInputBehavior, ActorSpawnedInGameBehavior, CameraProviderBehavior, UiRenderHudBehavior,
	},
	item_data::BaseClientItemDescriptor,
};

// === Components === //

type BlockPlacementCx<'a> = Cx<&'a MaterialColliderDescriptor, &'a mut ChunkVoxelData>;

// See: https://web.archive.org/web/20230313061131/https://www.mcpk.wiki/wiki/Jumping
pub const GRAVITY: f64 = 0.08 * MC_TICKS_TO_SECS_SQUARED;
pub const GRAVITY_VEC: EntityVec = EntityVec::from_glam(DVec3::new(0.0, -GRAVITY, 0.0));

pub const PLAYER_SPEED: f64 = 0.13 * MC_TICKS_TO_SECS_SQUARED;
pub const PLAYER_AIR_FRICTION_COEF: f64 = 0.98;
pub const PLAYER_BLOCK_FRICTION_COEF: f64 = 0.91;

pub const PLAYER_JUMP_IMPULSE: f64 = 0.43 * MC_TICKS_TO_SECS;
pub const PLAYER_JUMP_COOL_DOWN: u64 = 30;

pub const PLAYER_WIDTH: f64 = 0.8;
pub const PLAYER_HEIGHT: f64 = 1.8;
pub const PLAYER_EYE_LEVEL: f64 = 1.6;

#[derive(Debug)]
pub struct LocalPlayer {
	pub facing: Angle3D,
	pub fly_mode: bool,
	pub jump_cool_down: u64,
	pub view_bob: f64,
	pub inventory_slot: usize,
}

impl HasGlobalManagedTag for LocalPlayer {
	type Component = Self;
}

impl LocalPlayer {
	pub fn eye_height(&self) -> f64 {
		PLAYER_EYE_LEVEL + 0.1 * self.view_bob.sin()
	}

	pub fn eye_pos(&self, spatial: &Spatial) -> EntityVec {
		spatial.pos() + EntityVec::Y * self.eye_height()
	}

	pub fn process_movement(&mut self, kinematic: &mut KinematicObject, inputs: LocalPlayerInputs) {
		// Compute heading
		let mut heading = Vec3::ZERO;

		if inputs.forward {
			heading += Vec3::Z;
		}

		if inputs.backward {
			heading -= Vec3::Z;
		}

		if inputs.left {
			heading -= Vec3::X;
		}

		if inputs.right {
			heading += Vec3::X;
		}

		// Normalize heading
		let heading = heading.normalize_or_zero();

		// Convert player-local heading to world space
		let heading = EntityVec::cast_from(
			self.facing
				.as_matrix_horizontal()
				.transform_vector3(heading),
		);

		// Accelerate in that direction
		kinematic.apply_acceleration(heading * PLAYER_SPEED);

		// Update view bob
		{
			let bob_speed = kinematic.velocity.as_glam().xz().length().sqrt() * 0.1;

			if bob_speed > 0.1 && kinematic.was_face_touching(BlockFace::NegativeY) {
				self.view_bob += bob_speed;
				self.view_bob %= TAU;
			} else {
				let closest_origin = if (self.view_bob - PI).abs() < PI / 2.0 {
					PI
				} else {
					0.0
				};

				let old_weight = 5.0;
				self.view_bob = (self.view_bob * old_weight + closest_origin) / (1.0 + old_weight);
			}

			if self.view_bob.is_subnormal() {
				self.view_bob = 0.0;
			}
		}

		// Handle jumps
		if !inputs.jump {
			self.jump_cool_down = 0;
		}

		if self.jump_cool_down > 0 {
			self.jump_cool_down -= 1;
		}

		if inputs.jump {
			if self.fly_mode {
				kinematic.apply_acceleration(-GRAVITY_VEC * 2.0);
			} else if self.jump_cool_down == 0 && kinematic.was_face_touching(BlockFace::NegativeY)
			{
				self.jump_cool_down = PLAYER_JUMP_COOL_DOWN;
				*kinematic.velocity.y_mut() = PLAYER_JUMP_IMPULSE;
			}
		}
	}

	pub fn place_block_where_looking(
		&self,
		cx: BlockPlacementCx<'_>,
		world: &mut WorldVoxelData,
		registry: &BlockMaterialRegistry,
		spatial: &Spatial,
		max_dist: f64,
	) {
		let mut ray = RayCast::new_at(
			EntityVoxelPointer::new(world, self.eye_pos(spatial)),
			self.facing.forward().cast(),
		);

		use_generator!(let ray[y] = ray.step_intersect_for(y, cx!(cx), storage(), max_dist));

		while let Some((mut isect, _meta)) = ray.next((world, registry)) {
			if isect
				.block
				.state(cx!(cx), world)
				.is_some_and(|v| v.is_not_air())
			{
				isect
					.block
					.at_neighbor(Some((cx!(cx), world)), isect.face)
					.set_state_or_warn(
						// Noalias: the generator only borrows these components while producing the
						// next element.
						cx!(noalias cx),
						world,
						Block::new(registry.find_by_name("crucible:proto").unwrap().id),
					);
				break;
			}
		}
	}

	pub fn break_block_where_looking(
		&self,
		cx: BlockPlacementCx<'_>,
		world: &mut WorldVoxelData,
		registry: &BlockMaterialRegistry,
		spatial: &Spatial,
		max_dist: f64,
	) {
		let mut ray = RayCast::new_at(
			EntityVoxelPointer::new(world, self.eye_pos(spatial)),
			self.facing.forward().cast(),
		);

		use_generator!(let ray[y] = ray.step_intersect_for(y, cx!(cx), storage(), max_dist));

		while let Some((mut isect, _meta)) = ray.next((world, registry)) {
			if isect
				.block
				.state(cx!(cx), world)
				.is_some_and(|v| v.is_not_air())
			{
				isect.block.set_state_or_warn(
					// Noalias: the generator only borrows these components while producing the
					// next element.
					cx!(noalias cx),
					world,
					Block::new(BlockMaterialId::AIR),
				);
				break;
			}
		}
	}
}

#[derive(Debug, Copy, Clone)]
pub struct LocalPlayerInputs {
	pub forward: bool,
	pub backward: bool,
	pub left: bool,
	pub right: bool,
	pub jump: bool,
}

// === Prefabs === //

pub fn spawn_local_player(
	actor_manager: &mut ActorManager,
	mesh_registry: &MeshRegistry,
	item_registry: &ItemMaterialRegistry,
	on_actor_spawned: &mut impl EventTarget<ActorSpawned>,
	on_inventory_changed: &mut impl EventTarget<InventoryUpdated>,
) -> OwnedEntity {
	// Create player
	let player = OwnedEntity::new().with_debug_label("local player");

	// Create inventory
	let mut inventory = InventoryData::new(4 * 9);
	let _ = inventory.insert_stack(
		player.entity(),
		on_inventory_changed,
		actor_manager.spawn(
			on_actor_spawned,
			OwnedEntity::new()
				.with_debug_label("stone item stack")
				.with(ItemStackBase {
					material: item_registry.find_by_name("crucible:stone").unwrap().id,
					count: 1,
				}),
		),
		|_, _| false,
	);

	// Attach components
	player
		.with_tagged(
			GlobalTag::<LocalPlayer>,
			LocalPlayer {
				facing: Angle3D::ZERO,
				fly_mode: false,
				jump_cool_down: 0,
				view_bob: 0.0,
				inventory_slot: 0,
			},
		)
		.with_tagged(GlobalTag::<Spatial>, Spatial::new(EntityVec::ZERO))
		.with_tagged(GlobalTag::<HealthState>, HealthState::new(20.0, 20.0))
		.with_tagged(GlobalTag::<InventoryData>, inventory)
		// Physics
		.with_tagged(
			GlobalTag::<Collider>,
			Collider::new(EntityAabb {
				origin: EntityVec::ZERO,
				size: EntityVec::new(PLAYER_WIDTH, PLAYER_HEIGHT, PLAYER_WIDTH),
			}),
		)
		.with_tagged(
			GlobalTag::<TrackedCollider>,
			TrackedCollider {
				origin_offset: EntityVec::new(PLAYER_WIDTH / 2.0, 0.0, PLAYER_WIDTH / 2.0),
			},
		)
		.with_tagged(
			GlobalTag::<KinematicObject>,
			KinematicObject::new(tick_friction_coef_to_coef_qty(
				EntityVec::new(
					PLAYER_AIR_FRICTION_COEF * PLAYER_BLOCK_FRICTION_COEF,
					PLAYER_AIR_FRICTION_COEF,
					PLAYER_AIR_FRICTION_COEF * PLAYER_BLOCK_FRICTION_COEF,
				),
				60.0,
			)),
		)
		// Rendering
		.with_tagged(
			GlobalTag::<ActorMeshInstance>,
			ActorMeshInstance::new(
				mesh_registry
					.find_by_name("crucible:glagglesnoy")
					.unwrap()
					.descriptor,
			),
		)
}

// === Behaviors === //

alias! {
	let actor_mgr: ActorManager;
	let block_registry: BlockMaterialRegistry;
	let item_registry: ItemMaterialRegistry;
	let world: WorldVoxelData;
}

pub fn register(bhv: &mut BehaviorRegistry) {
	bhv.register(make_spawn_behavior())
		.register(make_input_behavior())
		.register(make_hud_render_behavior())
		.register(make_camera_behavior());
}

fn make_spawn_behavior() -> ActorSpawnedInGameBehavior {
	ActorSpawnedInGameBehavior::new(|_bhv, s, events, _scene| {
		scope!(use let s);

		query! {
			for (_event in *events; @me) + [GlobalTag::<LocalPlayer>] {
				log::info!("Spawned player {me:?}");
			}
		}
	})
}

fn make_input_behavior() -> ActorInputBehavior {
	ActorInputBehavior::new(|_bhv, s, scene_root, actor_tag, inputs| {
		scope!(
			use let s,
			access cx: Cx<
				&mut LocalPlayer,
				&Spatial,
				&mut KinematicObject,
				&mut ChunkVoxelData,
				&MaterialColliderDescriptor,
			>,
			inject { mut world = scene_root, ref block_registry = scene_root }
		);

		query! {
			for (
				mut player in GlobalTag::<LocalPlayer>,
				ref spatial in GlobalTag::<Spatial>,
				mut kinematic in GlobalTag::<KinematicObject>,
			) + [actor_tag] {
				// Apply gravity
				kinematic.apply_acceleration(GRAVITY_VEC);

				// Process mouse look
				player.facing += inputs.mouse_delta() * f32::to_radians(0.4);
				player.facing = player.facing.clamp_y_90().wrap_x();

				// Process fly mode
				if inputs.key(VirtualKeyCode::F).recently_pressed() {
					player.fly_mode = !player.fly_mode;
				}

				// Process movement
				player.process_movement(kinematic, LocalPlayerInputs {
					forward: inputs.key(VirtualKeyCode::W).state(),
					backward: inputs.key(VirtualKeyCode::S).state(),
					left: inputs.key(VirtualKeyCode::A).state(),
					right: inputs.key(VirtualKeyCode::D).state(),
					jump: inputs.key(VirtualKeyCode::Space).state(),
				});

				for (i, k) in [
					VirtualKeyCode::Key1,
					VirtualKeyCode::Key2,
					VirtualKeyCode::Key3,
					VirtualKeyCode::Key4,
					VirtualKeyCode::Key5,
					VirtualKeyCode::Key6,
					VirtualKeyCode::Key7,
					VirtualKeyCode::Key8,
					VirtualKeyCode::Key9,
				].into_iter().enumerate() {
					if inputs.key(k).recently_pressed() {
						player.inventory_slot = i;
					}
				}

				// Handle block placement
				if inputs.button(MouseButton::Right).recently_pressed() {
					player.place_block_where_looking(cx!(cx), world, block_registry, spatial, 7.0);
				}

				if inputs.button(MouseButton::Left).recently_pressed() {
					player.break_block_where_looking(cx!(cx), world, block_registry, spatial, 7.0);
				}
			}
		}
	})
}

fn make_hud_render_behavior() -> UiRenderHudBehavior {
	UiRenderHudBehavior::new(|_bhv, s, brush, screen_size, scene| {
		scope!(
			use let s,
			access cx: Cx<
				&LocalPlayer,
				&HealthState,
				&InventoryData,
				&ItemStackBase,
				&BaseClientItemDescriptor,
			>,
			inject { ref actor_mgr = scene, ref item_registry = scene }
		);

		// Draw crosshair
		brush.fill_rect(
			Aabb2::from_origin_size(screen_size / 2.0, Vec2::splat(10.), Vec2::splat(0.5)),
			Color4::new(1.0, 0.0, 0.0, 0.5),
		);

		// Draw hotbar
		let total_w = screen_size.x * 0.6;
		let item_fw = total_w / 9.0;
		let item_vw = item_fw * 0.95;
		let total_w = total_w - (item_fw - item_vw);

		query! {
			for (
				ref player in GlobalTag::<LocalPlayer>,
				ref health in GlobalTag::<HealthState>,
				ref inventory in GlobalTag::<InventoryData>,
			) + [actor_mgr.tag()] {
				let start_x = screen_size.x / 2.0 - total_w / 2.0;

				// Draw items
				let mut x = start_x;
				for i in 0..9 {
					let item_aabb = Aabb2::new(x, screen_size.y - item_fw - 50.0, item_vw, item_vw);
					brush.fill_rect(
						item_aabb,
						if i == player.inventory_slot {
							Color4::new(0.4, 0.3, 1.0, 1.0)
						} else {
							Color4::new(0.0, 0.4, 1.0, 0.4)
						}
					);

					if let Some(stack) = inventory.slot(i) {
						let descriptor = item_registry
							.find_by_id(stack.get_s::<ItemStackBase>(cx!(cx)).material)
							.descriptor
							.get_s::<BaseClientItemDescriptor>(cx!(cx));

						brush.fill_rect(
							Aabb2::from_origin_size(
								item_aabb.at_percent(Vec2::splat(0.5)),
								item_aabb.size * 0.75,
								Vec2::splat(0.5),
							),
							descriptor.color,
						);
					}

					x += item_fw;
				}

				// Draw health
				brush.fill_rect(
					Aabb2::new(
						start_x,
						screen_size.y - item_vw - 50.0 - 20.0 - 10.0,
						total_w * health.health_percent(),
						10.0,
					),
					Color4::new(1.0, 0.0, 0.0, 1.0),
				);
				brush.fill_rect(
					Aabb2::new(
						start_x,
						screen_size.y - item_vw - 50.0 - 20.0 - 10.0,
						total_w * health.health_percent(),
						10.0,
					),
					Color4::new(0.0, 1.0, 0.0, 1.0),
				);
			}
		}
	})
}

fn make_camera_behavior() -> CameraProviderBehavior {
	CameraProviderBehavior::new(|_bhv, s, actor_tag, camera_mgr| {
		scope!(use let s, access _cx: Cx<&Spatial, &LocalPlayer>);

		query! {
			for (ref spatial in GlobalTag::<Spatial>, ref player in GlobalTag::<LocalPlayer>) + [actor_tag] {
				camera_mgr.set_pos_rot(
					player.eye_pos(spatial).to_glam().as_vec3(),
					player.facing,
					CameraSettings::Perspective { fov: 110f32.to_radians(), near: 0.1, far: 500.0 },
				);
			}
		}
	})
}
