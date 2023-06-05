use bort::{Entity, OwnedEntity};
use crucible_foundation_client::{
	engine::{
		gfx::texture::FullScreenTexture,
		io::{gfx::GfxContext, input::InputManager, main_loop::MainLoop},
		scene::{SceneRenderHandler, SceneUpdateHandler},
	},
	gfx::voxel::mesh::{ChunkVoxelMesh, WorldVoxelMesh},
};
use crucible_foundation_shared::{
	bort::delegate::ComponentInjector,
	voxel::{
		data::{ChunkVoxelData, WorldVoxelData},
		loader::{LoadedChunk, WorldChunkFactory, WorldLoader},
	},
};
use crucible_shared::world::{WorldManagedData, WorldManager};
use winit::event::VirtualKeyCode;

#[derive(Debug)]
pub struct GameSceneRoot {
	engine: Entity,
	viewport: Entity,
}

impl GameSceneRoot {
	pub fn spawn(engine: Entity, viewport: Entity) -> OwnedEntity {
		let root = OwnedEntity::new()
			.with_debug_label("game root")
			.with(Self { engine, viewport })
			.with(WorldManager::default())
			.with(SceneUpdateHandler::new_method_mut(
				ComponentInjector,
				Self::update,
			))
			.with(SceneRenderHandler::new_method_mut(
				ComponentInjector,
				Self::render,
			));

		// Create the main world
		root.get_mut::<WorldManager>().create_world(
			"overworld",
			OwnedEntity::new()
				.with(WorldManagedData::default())
				.with(WorldVoxelData::default())
				.with(WorldVoxelMesh::default())
				.with(WorldLoader::new(WorldChunkFactory::new(|_, pos| {
					OwnedEntity::new()
						.with_debug_label(format_args!("chunk at {pos:?}"))
						.with(ChunkVoxelData::default())
						.with(ChunkVoxelMesh::default())
						.with(LoadedChunk::default())
						.into_obj()
				})))
				.into_obj(),
		);

		root
	}

	pub fn update(&mut self, _me: Entity, main_loop: &mut MainLoop) {
		// Handle escape-to-exit
		if self
			.viewport
			.get::<InputManager>()
			.key(VirtualKeyCode::Escape)
			.recently_pressed()
		{
			main_loop.exit();
		}
	}

	pub fn render(&mut self, _me: Entity, viewport: Entity, frame: &mut wgpu::SurfaceTexture) {
		// Acquire context
		let gfx = &*self.engine.get::<GfxContext>();
		let viewport_depth = &mut *viewport.get_mut::<FullScreenTexture>();

		// Render a black screen
		let frame_view = frame
			.texture
			.create_view(&wgpu::TextureViewDescriptor::default());

		let mut cb = gfx
			.device
			.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

		let mut pass = cb.begin_render_pass(&wgpu::RenderPassDescriptor {
			label: None,
			color_attachments: &[Some(wgpu::RenderPassColorAttachment {
				view: &frame_view,
				resolve_target: None,
				ops: wgpu::Operations {
					load: wgpu::LoadOp::Clear(wgpu::Color {
						r: 0.1,
						g: 0.1,
						b: 0.1,
						a: 1.0,
					}),
					store: true,
				},
			})],
			depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
				view: viewport_depth.acquire_view(&gfx, &*viewport.get()),
				depth_ops: Some(wgpu::Operations {
					load: wgpu::LoadOp::Clear(1.0),
					store: true,
				}),
				stencil_ops: None,
			}),
		});

		drop(pass);

		gfx.queue.submit([cb.finish()]);
	}
}
