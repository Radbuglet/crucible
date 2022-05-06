use crate::util::features::{FeatureDescriptor, FeatureList, FeatureScore};
use crate::util::num::OrdF32;
use anyhow::Context;
use std::fmt::Display;
use winit::window::Window;

#[derive(Debug)]
pub struct GfxContext {
	pub instance: wgpu::Instance,
	pub device: wgpu::Device,
	pub queue: wgpu::Queue,
	pub adapter: wgpu::Adapter,
	pub adapter_info: AdapterInfoBundle,
	pub features_req: wgpu::Features,
	pub limits_req: wgpu::Limits,
}

impl GfxContext {
	pub async fn init<D: ?Sized + GfxFeatureDetector>(
		main_window: &Window,
		compat_detector: &mut D,
	) -> anyhow::Result<(Self, D::Table, wgpu::Surface)> {
		let backends = wgpu::Backends::PRIMARY;
		let instance = wgpu::Instance::new(backends);
		let main_surface = unsafe { instance.create_surface(main_window) };

		struct ValidatedAdapter<'a, T> {
			adapter: wgpu::Adapter,
			adapter_info: AdapterInfoBundle,
			descriptor: wgpu::DeviceDescriptor<'a>,
			compat_table: T,
			score: OrdF32,
		}

		let req = instance
			.enumerate_adapters(backends)
			.filter_map(|adapter| {
				// Get info about the adapter
				let adapter_info = AdapterInfoBundle::get(&adapter);

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
					"Found adapter \"{name}\" using backend {backend:?}. Score: {}",
					match &score {
						Some(score) => score as &dyn Display,
						None => &"missing mandatory features" as &dyn Display,
					}
				);
				log::info!("Feature table: {:#?}", features);

				if let Some(compat_table) = compat_table {
					Some(ValidatedAdapter {
						adapter,
						adapter_info,
						descriptor,
						compat_table,
						score: features.score().unwrap(),
					})
				} else {
					// The adapter did not pass.
					None
				}
			})
			.max_by(|a, b| a.score.cmp(&b.score))
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
				features_req: req.descriptor.features,
				limits_req: req.descriptor.limits,
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
	pub fn get(adapter: &wgpu::Adapter) -> Self {
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
	pub descriptor: &'a mut wgpu::DeviceDescriptor<'l>,
	pub instance: &'a wgpu::Instance,
	pub main_surface: &'a wgpu::Surface,
	pub adapter: &'a wgpu::Adapter,
	pub adapter_info: &'a AdapterInfoBundle,
}

pub trait GfxFeatureDetector {
	type Table;

	fn query_compat(&mut self, info: &mut CompatQueryInfo) -> (FeatureList, Option<Self::Table>);
}

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
			if info.adapter.is_surface_supported(&info.main_surface) {
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
