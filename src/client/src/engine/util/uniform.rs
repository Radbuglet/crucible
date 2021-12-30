use crate::engine::context::GfxContext;
use crate::engine::util::gpu_align_ext::align_up;
use glsl_layout::Std140;
use std::mem::{align_of, swap};

pub struct UniformManager {
	head: usize,
	buffer_a: wgpu::Buffer,
	buffer_b: wgpu::Buffer,
	#[cfg(debug_assertions)]
	cap: usize,
	#[cfg(debug_assertions)]
	is_mapped: bool,
}

impl UniformManager {
	pub fn new(
		gfx: &GfxContext,
		label: wgpu::Label,
		usage: wgpu::BufferUsages,
		cap: usize,
	) -> Self {
		let usage = usage | wgpu::BufferUsages::MAP_WRITE | wgpu::BufferUsages::MAP_READ;

		// Create labels
		let label_a: String;
		let label_a_str = match label {
			Some(prefix) => {
				label_a = format!("{}_swap_1", prefix);
				Some(label_a.as_str())
			}
			None => None,
		};

		let label_b: String;
		let label_b_str = match label {
			Some(prefix) => {
				label_b = format!("{}_swap_2", prefix);
				Some(label_b.as_str())
			}
			None => None,
		};

		// Create buffers
		let buffer_a = gfx.device.create_buffer(&wgpu::BufferDescriptor {
			label: label_a_str,
			size: cap as _,
			usage,
			mapped_at_creation: false,
		});

		let buffer_b = gfx.device.create_buffer(&wgpu::BufferDescriptor {
			label: label_b_str,
			size: cap as _,
			usage,
			mapped_at_creation: false,
		});

		// Construct manager
		Self {
			head: 0,
			buffer_a,
			buffer_b,
			#[cfg(debug_assertions)]
			cap,
			#[cfg(debug_assertions)]
			is_mapped: false,
		}
	}

	pub async fn begin_frame(&mut self) -> Result<(), wgpu::BufferAsyncError> {
		// Validate call early on to catch bugs.
		#[cfg(debug_assertions)]
		{
			debug_assert!(!self.is_mapped);
			self.is_mapped = true;
		}

		self.head = 0;
		self.buffer_a
			.slice(..)
			.map_async(wgpu::MapMode::Write)
			.await
	}

	pub fn end_frame(&mut self) {
		// Validate call early on to catch bugs.
		#[cfg(debug_assertions)]
		{
			debug_assert!(self.is_mapped);
			self.is_mapped = false;
		}

		self.buffer_a.unmap();
		swap(&mut self.buffer_a, &mut self.buffer_b);
	}

	pub fn push<P: Std140>(&mut self, gfx: &GfxContext, object: &P) -> wgpu::BufferBinding {
		// Fetch bytes
		let bytes = object.as_raw();

		// Align head
		let head = align_up(
			self.head,
			(gfx.limits.min_uniform_buffer_offset_alignment as usize).max(align_of::<P>()),
		);
		debug_assert!(head < self.cap as usize);

		// Upload to GPU
		self.buffer_a.slice(..).get_mapped_range_mut()[self.head..][..bytes.len()]
			.clone_from_slice(bytes);

		// Create binding descriptor
		let binding = wgpu::BufferBinding {
			buffer: &self.buffer_a,
			offset: self.head as _,
			size: Some(wgpu::BufferSize::new(bytes.len() as _).unwrap()),
		};

		self.head = head + bytes.len();
		binding
	}

	pub fn buffer(&self) -> &wgpu::Buffer {
		&self.buffer_a
	}
}
