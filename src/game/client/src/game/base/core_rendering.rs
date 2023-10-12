use bort::{alias, scope, BehaviorRegistry};
use crucible_foundation_client::{
	engine::{assets::AssetManager, gfx::camera::CameraManager, io::gfx::GfxContext},
	gfx::skybox::pipeline::SkyboxUniforms,
};
use crucible_util::debug::error::ResultExt;
use wgpu::util::DeviceExt;

use super::behaviors::InitGame;

alias! {
	let gfx: GfxContext;
	let asset_mgr: AssetManager;
}

pub fn register(bhv: &mut BehaviorRegistry) {
	bhv.register_cx(
		[],
		InitGame::new(|_bhv, s, scene, engine| {
			scope!(use let s, inject { mut asset_mgr = engine, ref gfx = engine });

			// Create camera manager
			scene.add(CameraManager::default());

			// Create skybox renderer
			let skybox = image::load_from_memory(include_bytes!("../res/skybox.png"))
				.unwrap_pretty()
				.into_rgba8();

			let skybox = gfx.device.create_texture_with_data(
				&gfx.queue,
				&wgpu::TextureDescriptor {
					label: Some("Skybox panorama"),
					size: wgpu::Extent3d {
						width: skybox.width(),
						height: skybox.height(),
						depth_or_array_layers: 1,
					},
					mip_level_count: 1,
					sample_count: 1,
					dimension: wgpu::TextureDimension::D2,
					format: wgpu::TextureFormat::Rgba8Unorm,
					usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
					view_formats: &[],
				},
				&skybox,
			);

			let skybox = skybox.create_view(&wgpu::TextureViewDescriptor::default());

			scene.add(SkyboxUniforms::new(asset_mgr, gfx, &skybox));
		}),
	);
}
