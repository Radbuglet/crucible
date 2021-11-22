use anyhow::Context;
use crucible_core::util::error::AnyResult;
use winit::window::Window;

pub struct GfxContext {
	// Singletons
	pub instance: wgpu::Instance,
	pub adapter: wgpu::Adapter,
	pub device: wgpu::Device,
	pub queue: wgpu::Queue,
	// Caches
	pub limits: wgpu::Limits,
	pub features: wgpu::Features,
}

impl GfxContext {
	pub async fn no_window(features: wgpu::Features) -> AnyResult<Self> {
		Self::new(None, features).await.map(|(cx, _)| cx)
	}

	pub async fn with_window(
		window: &Window,
		features: wgpu::Features,
	) -> AnyResult<(Self, wgpu::Surface)> {
		Self::new(Some(window), features)
			.await
			.map(|(cx, surface)| (cx, surface.unwrap()))
	}

	// TODO: Prioritize adapters and devices which work best with the requested features & limits.
	pub async fn new(
		window: Option<&Window>,
		features: wgpu::Features,
	) -> AnyResult<(Self, Option<wgpu::Surface>)> {
		let instance = wgpu::Instance::new(wgpu::Backends::PRIMARY);
		let surface = window.map(|window| unsafe { instance.create_surface(window) });

		// Create a default adapter
		let adapter = instance
			.request_adapter(&wgpu::RequestAdapterOptions {
				// Ensure that the device supports the main window's presentation engine.
				compatible_surface: surface.as_ref(),
				// Prioritize external GPUs
				power_preference: wgpu::PowerPreference::HighPerformance,
				// Don't force software rendering
				force_fallback_adapter: false,
			})
			.await
			.context("Failed to find a device adapter.")?;

		let info = adapter.get_info();
		log::info!(
			"Using backend {:?} and physical device {}",
			info.backend,
			info.name,
		);

		// Construct a logical device and fetch its queue(s).
		let (device, queue) = adapter
			.request_device(
				&wgpu::DeviceDescriptor {
					label: Some("Device"),
					limits: Default::default(),
					features,
				},
				None,
			)
			.await?;

		let limits = device.limits();
		let features = device.features();

		Ok((
			Self {
				instance,
				adapter,
				device,
				queue,
				limits,
				features,
			},
			surface,
		))
	}
}
