use std::sync::Arc;

use anyhow::Context;
use fmt_util::DisplayFromFn;
use winit::window::Window;

#[derive(Debug)]
pub struct GfxContext {
    pub instance: wgpu::Instance,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub adapter: wgpu::Adapter,
    pub adapter_info: AdapterInfoBundle,

    // These need to be queried fairly regularly so it's best to just store them even if you can just
    // fetch them from the device.
    pub requested_features: wgpu::Features,
    pub requested_limits: wgpu::Limits,
}

impl GfxContext {
    pub async fn new<T>(
        main_window: Arc<Window>,
        mut compat_detector: impl FnMut(&mut CompatQueryInfo) -> (Judgement, T),
    ) -> anyhow::Result<(Self, wgpu::Surface<'static>, T)> {
        let backends = wgpu::Backends::PRIMARY;
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends,
            dx12_shader_compiler: wgpu::Dx12Compiler::Dxc {
                dxil_path: None,
                dxc_path: None,
            },
            flags: wgpu::InstanceFlags::empty(),
            gles_minor_version: wgpu::Gles3MinorVersion::Automatic,
        });

        let main_surface = instance
            .create_surface(main_window)
            .context("failed to create main surface")?;

        struct ValidatedAdapter<'a, T> {
            adapter: wgpu::Adapter,
            adapter_info: AdapterInfoBundle,
            descriptor: wgpu::DeviceDescriptor<'a>,
            compat_table: T,
            score: f64,
        }

        let req = instance
            .enumerate_adapters(backends)
            .into_iter()
            .filter_map(|adapter| {
                // Get info about the adapter
                let adapter_info = AdapterInfoBundle::new_for(&adapter);

                // Query support and config
                let mut descriptor = wgpu::DeviceDescriptor::default();
                let (judgement, compat_table) = (compat_detector)(&mut CompatQueryInfo {
                    descriptor: &mut descriptor,
                    instance: &instance,
                    main_surface: &main_surface,
                    adapter: &adapter,
                    adapter_info: &adapter_info,
                });

                // Log info
                let wgpu::AdapterInfo { name, backend, .. } = &adapter_info.info;

                tracing::info!(
                    "Found adapter {name:?} using backend {backend:?}. Score: {}",
                    DisplayFromFn(|f| {
                        match judgement.kind {
                            JudgementKind::Ok => f.write_str("perfect"),
                            JudgementKind::Penalized(penalty) => {
                                write!(f, "penalized: {penalty}")
                            }
                            JudgementKind::Err => f.write_str("incompatible"),
                        }
                    })
                );
                tracing::info!("Feature table: {:#?}", judgement);

                judgement.did_pass().then_some(ValidatedAdapter {
                    adapter,
                    adapter_info,
                    descriptor,
                    compat_table,
                    score: judgement.score(),
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
                requested_features: req.descriptor.required_features,
                requested_limits: req.descriptor.required_limits,
            },
            main_surface,
            req.compat_table,
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
    pub descriptor: &'a mut wgpu::DeviceDescriptor<'l>,
    pub instance: &'a wgpu::Instance,
    pub main_surface: &'a wgpu::Surface<'static>,
    pub adapter: &'a wgpu::Adapter,
    pub adapter_info: &'a AdapterInfoBundle,
}

// === Judgement === //

#[derive(Debug)]
pub struct Judgement {
    pub name: String,
    pub kind: JudgementKind,
    pub reason: Option<anyhow::Error>,
    pub subs: Vec<Judgement>,
}

#[derive(Debug, Copy, Clone)]
pub enum JudgementKind {
    Ok,
    Err,
    Penalized(f64),
}

impl Judgement {
    // === Constructors === //

    pub fn new_ok(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: JudgementKind::Ok,
            reason: None,
            subs: Vec::new(),
        }
    }

    pub fn new_err(name: impl Into<String>, reason: anyhow::Error) -> Self {
        Self {
            name: name.into(),
            kind: JudgementKind::Err,
            reason: Some(reason),
            subs: Vec::new(),
        }
    }

    pub fn from_result(name: impl Into<String>, result: anyhow::Result<()>) -> Self {
        match result {
            Ok(()) => Self::new_ok(name),
            Err(reason) => Self::new_err(name, reason),
        }
    }

    pub fn new_penalty(name: impl Into<String>, reason: anyhow::Error, penalty: f64) -> Self {
        Self {
            name: name.into(),
            kind: JudgementKind::Penalized(penalty),
            reason: Some(reason),
            subs: Vec::new(),
        }
    }

    pub fn make_soft_error(self, penalty: f64) -> Self {
        Self {
            name: self.name,
            kind: match self.kind {
                JudgementKind::Ok => JudgementKind::Ok,
                _ => JudgementKind::Penalized(penalty),
            },
            reason: self.reason,
            subs: self.subs,
        }
    }

    pub fn push_sub(&mut self, judgement: Judgement) {
        // Merge kinds
        match judgement.kind {
            JudgementKind::Ok => { /* no-op */ }
            JudgementKind::Err => {
                self.kind = JudgementKind::Err;
            }
            JudgementKind::Penalized(penalty) => match &mut self.kind {
                me @ JudgementKind::Ok => *me = JudgementKind::Penalized(penalty),
                JudgementKind::Err => { /* no-op */ }
                JudgementKind::Penalized(cumulative) => {
                    *cumulative += penalty;
                }
            },
        }

        // Push sub
        self.subs.push(judgement);
    }

    pub fn with_sub(mut self, judgement: Judgement) -> Self {
        self.push_sub(judgement);
        self
    }

    // === Decoding === //

    pub fn did_pass(&self) -> bool {
        matches!(self.kind, JudgementKind::Ok | JudgementKind::Penalized(_))
    }

    pub fn score(&self) -> f64 {
        match self.kind {
            JudgementKind::Ok => 0.,
            JudgementKind::Err => f64::NEG_INFINITY,
            JudgementKind::Penalized(penalty) => -penalty,
        }
    }

    // === Shorthand === //

    pub fn with_table<T>(self, table: T) -> (Self, T) {
        (self, table)
    }
}

// === Foundational Feature Judgements === //

pub fn feat_requires_screen(info: &mut CompatQueryInfo) -> (Judgement, ()) {
    Judgement::from_result(
        "The main window can be drawn to",
        if info.adapter.is_surface_supported(info.main_surface) {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "the main window is not supported by the adapter"
            ))
        },
    )
    .with_table(())
}

pub fn feat_requires_power_pref(
    pref: wgpu::PowerPreference,
) -> impl FnMut(&mut CompatQueryInfo) -> (Judgement, ()) {
    move |info: &mut CompatQueryInfo| {
        let mode = info.adapter_info.device_type();
        let matches = match mode {
            wgpu::DeviceType::Other => true,
            wgpu::DeviceType::IntegratedGpu => pref == wgpu::PowerPreference::LowPower,
            wgpu::DeviceType::DiscreteGpu => pref == wgpu::PowerPreference::HighPerformance,
            wgpu::DeviceType::VirtualGpu => pref == wgpu::PowerPreference::LowPower,
            wgpu::DeviceType::Cpu => pref == wgpu::PowerPreference::LowPower,
        };

        Judgement::from_result(
            format!("GPU has {pref:?} power preference"),
            if matches {
                Ok(())
            } else {
                Err(anyhow::format_err!(
					"expected GPU with {pref:?} power preference; got {mode:?} adapter type, which \
					  has the opposite power preference"
				))
            },
        )
        .make_soft_error(10.)
        .with_table(())
    }
}
