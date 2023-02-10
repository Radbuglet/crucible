use anyhow::Context;
use std::fmt::Display;
use winit::window::Window;

use super::features::{FeatureDescriptor, FeatureList, FeatureScore};

#[derive(Debug)]
pub struct GfxContext {
	/// Our WebGPU instance from which everything was derived.
	pub instance: wgpu::Instance,

	/// Our WebGPU device.
	pub device: wgpu::Device,

	/// Our WebGPU queue.
	pub queue: wgpu::Queue,

	/// Our WebGPU adapter from which our device and queue were derived.
	pub adapter: wgpu::Adapter,

	/// Our WebGPU adapter's real features and limits.
	pub adapter_info: AdapterInfoBundle,

	/// Our device's requested feature set.
	pub requested_features: wgpu::Features,

	/// Our device's requested limits.
	pub requested_limits: wgpu::Limits,
}

impl GfxContext {
	pub async fn init<D: ?Sized + GfxFeatureDetector>(
		main_window: &Window,
		compat_detector: &mut D,
	) -> anyhow::Result<(Self, D::Table, wgpu::Surface)> {
		let backends = wgpu::Backends::PRIMARY;
		let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
			backends,
			dx12_shader_compiler: wgpu::Dx12Compiler::Dxc {
				dxil_path: None,
				dxc_path: None,
			},
		});
		let main_surface = unsafe {
			// FIXME: Windows can still be destroyed unexpectedly, potentially causing UB. We need
			// tighter integration between the viewport manager and our `GfxContext`.
			instance
				.create_surface(main_window)
				.context("failed to create main surface")?
		};

		struct ValidatedAdapter<'a, T> {
			adapter: wgpu::Adapter,
			adapter_info: AdapterInfoBundle,
			descriptor: wgpu::DeviceDescriptor<'a>,
			compat_table: T,
			score: f32,
		}

		let req = instance
			.enumerate_adapters(backends)
			.filter_map(|adapter| {
				// Get info about the adapter
				let adapter_info = AdapterInfoBundle::new_for(&adapter);

				// Query support and config
				let mut descriptor = wgpu::DeviceDescriptor::default();
				let (features, compat_table) = compat_detector.query_compat(&mut CompatQueryInfo {
					descriptor: &mut descriptor,
					instance: &instance,
					main_surface: &main_surface,
					adapter: &adapter,
					adapter_info: &adapter_info,
				});

				assert_eq!(features.did_pass(), compat_table.is_some());

				// Log info
				let wgpu::AdapterInfo { name, backend, .. } = &adapter_info.info;
				let score = features.score();
				log::info!(
					"Found adapter {name:?} using backend {backend:?}. Score: {}",
					match &score {
						Some(score) => score as &dyn Display,
						None => &"missing mandatory features" as &dyn Display,
					}
				);
				log::info!("Feature table: {:#?}", features);

				compat_table.map(|compat_table| ValidatedAdapter {
					adapter,
					adapter_info,
					descriptor,
					compat_table,
					score: features.score().unwrap(),
				})
			})
			.max_by(|a, b| a.score.total_cmp(&b.score))
			.context("no adapters satisfy the application's minimum requirements")?;

		let (device, queue) = req
			.adapter
			.request_device(&req.descriptor, None)
			.await
			.context("failed to acquire wgpu device")?;

		Ok((
			Self {
				instance,
				device,
				queue,
				adapter: req.adapter,
				adapter_info: req.adapter_info,
				requested_features: req.descriptor.features,
				requested_limits: req.descriptor.limits,
			},
			req.compat_table,
			main_surface,
		))
	}
}

#[derive(Debug, Clone)]
pub struct AdapterInfoBundle {
	pub info: wgpu::AdapterInfo,
	pub limits: wgpu::Limits,
	pub features: wgpu::Features,
}

impl AdapterInfoBundle {
	pub fn new_for(adapter: &wgpu::Adapter) -> Self {
		Self {
			info: adapter.get_info(),
			limits: adapter.limits(),
			features: adapter.features(),
		}
	}

	pub fn device_type(&self) -> wgpu::DeviceType {
		self.info.device_type
	}
}

#[derive(Debug)]
pub struct CompatQueryInfo<'a, 'l> {
	/// The descriptor being modified by the [GfxFeatureDetector] to define our requested device.
	pub descriptor: &'a mut wgpu::DeviceDescriptor<'l>,

	/// Our WebGPU instance.
	pub instance: &'a wgpu::Instance,

	/// The main surface against which we're creating our [GfxContext].
	pub main_surface: &'a wgpu::Surface,

	/// The adapter against which we're trying to create our [GfxContext].
	pub adapter: &'a wgpu::Adapter,

	/// The adapter's limits and features.
	pub adapter_info: &'a AdapterInfoBundle,
}

pub trait GfxFeatureDetector {
	/// A userdata table describing the features the rest of the engine can depend upon.
	type Table;

	/// Queries the provided adapter for compatibility. Produces a [FeatureList] describing the
	/// justification for which a given configuration is supported, partially supported, or rejected
	/// as well as a userland table of type [GfxFeatureDetector::Table], which describes the actual
	/// set of logical engine features which are supported is also returned. The userland table of
	/// type [GfxFeatureDetector::Table] is `Some` if and only if the [FeatureList] passes
	/// (i.e. the adapter is declared valid).
	fn query_compat(&mut self, info: &mut CompatQueryInfo) -> (FeatureList, Option<Self::Table>);
}

/// Asserts that the adapter must be capable of presenting to the primary surface.
pub struct GfxFeatureNeedsScreen;

impl GfxFeatureDetector for GfxFeatureNeedsScreen {
	type Table = ();

	fn query_compat(&mut self, info: &mut CompatQueryInfo) -> (FeatureList, Option<Self::Table>) {
		let mut features = FeatureList::default();

		features.mandatory_feature(
			FeatureDescriptor {
				name: "Can display to screen",
				description: "The specified driver must be capable of rendering to the main window",
			},
			if info.adapter.is_surface_supported(info.main_surface) {
				FeatureScore::BinaryPass
			} else {
				FeatureScore::BinaryFail {
					reason: "main surface is unsupported by adapter".to_string(),
				}
			},
		);

		features.wrap_user_table(())
	}
}

/// Asserts that the adapter must be have the proper power preference. This can be turned into a soft
/// requirement by nesting `FeatureLists`.
#[derive(Debug, Clone)]
pub struct GfxFeaturePowerPreference(pub wgpu::PowerPreference);

impl GfxFeatureDetector for GfxFeaturePowerPreference {
	type Table = ();

	fn query_compat(&mut self, info: &mut CompatQueryInfo) -> (FeatureList, Option<Self::Table>) {
		use wgpu::{DeviceType::*, PowerPreference::*};

		let mut features = FeatureList::default();

		let mode = info.adapter_info.device_type();
		let pref = self.0;
		let matches = match mode {
			Other => true,
			IntegratedGpu => pref == LowPower,
			DiscreteGpu => pref == HighPerformance,
			VirtualGpu => pref == LowPower,
			Cpu => pref == LowPower,
		};

		features.mandatory_feature(
			FeatureDescriptor {
				name: format_args!("GPU power preference"),
				description: format_args!("For best performance, GPU must be {pref:?}."),
			},
			if matches {
				FeatureScore::BinaryPass
			} else {
				FeatureScore::BinaryFail {
					reason: format!(
						"expected GPU with {pref:?} power preference; got {mode:?} adapter type, which \
					 	 has the opposite power preference"
					),
				}
			},
		);

		features.wrap_user_table(())
	}
}
