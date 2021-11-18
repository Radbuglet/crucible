use crate::engine::context::GfxContext;
use std::mem::swap;

pub struct UniformManager {
	head: usize,
	is_mapped: bool,
	buffer_a: wgpu::Buffer,
	buffer_b: wgpu::Buffer,
}

impl UniformManager {
	pub fn new(
		gfx: &GfxContext,
		label: wgpu::Label,
		usage: wgpu::BufferUsages,
		size: wgpu::BufferAddress,
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
			size,
			usage,
			mapped_at_creation: false,
		});

		let buffer_b = gfx.device.create_buffer(&wgpu::BufferDescriptor {
			label: label_b_str,
			size,
			usage,
			mapped_at_creation: false,
		});

		// Construct manager
		Self {
			head: 0,
			is_mapped: false,
			buffer_a,
			buffer_b,
		}
	}

	pub async fn begin_frame(&mut self) -> Result<(), wgpu::BufferAsyncError> {
		assert!(!self.is_mapped);
		self.is_mapped = true;
		self.head = 0;
		self.buffer_a
			.slice(..)
			.map_async(wgpu::MapMode::Write)
			.await
	}

	pub fn end_frame(&mut self) {
		assert!(self.is_mapped);
		self.is_mapped = false;
		self.buffer_a.unmap();
		swap(&mut self.buffer_a, &mut self.buffer_b);
	}

	// TODO: We might need to implement an alignment system (maybe we should take a GpuPod object?)
	pub fn push(&mut self, bytes: &[u8]) -> wgpu::BufferBinding {
		assert!(bytes.len() > 0);
		self.buffer_a.slice(..).get_mapped_range_mut()[self.head..(self.head + bytes.len())]
			.clone_from_slice(bytes);

		let binding = wgpu::BufferBinding {
			buffer: &self.buffer_a,
			offset: self.head as _,
			size: Some(wgpu::BufferSize::new(bytes.len() as _).unwrap()),
		};

		self.head += bytes.len();
		binding
	}

	pub fn buffer(&self) -> &wgpu::Buffer {
		&self.buffer_a
	}
}
