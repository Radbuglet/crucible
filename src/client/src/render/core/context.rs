use anyhow::Context;
use winit::window::Window;

pub struct GfxContext {
	pub instance: wgpu::Instance,
	pub adapter: wgpu::Adapter,
	pub device: wgpu::Device,
	pub queue: wgpu::Queue,
}

impl GfxContext {
	pub async fn no_window() -> anyhow::Result<Self> {
		Self::new(None).await.map(|(cx, _)| cx)
	}

	pub async fn with_window(window: &Window) -> anyhow::Result<(Self, wgpu::Surface)> {
		Self::new(Some(window))
			.await
			.map(|(cx, surface)| (cx, surface.unwrap()))
	}

	pub async fn new(window: Option<&Window>) -> anyhow::Result<(Self, Option<wgpu::Surface>)> {
		let instance = wgpu::Instance::new(wgpu::Backends::PRIMARY);
		let surface = window.map(|window| unsafe { instance.create_surface(window) });

		// Create a default adapter
		// TODO: Allow users to request features
		let adapter = instance
			.request_adapter(&wgpu::RequestAdapterOptions {
				// Ensure that the device supports the main window's presentation engine.
				compatible_surface: surface.as_ref(),
				// Prioritize external GPUs
				power_preference: wgpu::PowerPreference::HighPerformance,
			})
			.await
			.context("Failed to find a device adapter.")?;

		let info = adapter.get_info();
		println!(
			"Using backend {:?} and physical device {}",
			info.backend, info.name
		);

		// Construct a logical device and fetch its queue(s).
		let (device, queue) = adapter
			.request_device(
				&wgpu::DeviceDescriptor {
					label: Some("Device"),
					limits: Default::default(),
					features: Default::default(),
				},
				None,
			)
			.await?;

		Ok((
			Self {
				instance,
				adapter,
				device,
				queue,
			},
			surface,
		))
	}
}
