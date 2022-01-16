use crate::engine::context::GfxContext;
use crucible_core::util::pod::{PodWriter, VecWriter};

#[derive(Debug)]
pub struct UniformManager {
	// FIXME: Check capacity
	writer: VecWriter,
	buffer: wgpu::Buffer,
}

impl UniformManager {
	pub fn new(
		gfx: &GfxContext,
		label: wgpu::Label,
		usage: wgpu::BufferUsages,
		cap: usize,
	) -> Self {
		let buffer = gfx.device.create_buffer(&wgpu::BufferDescriptor {
			label,
			size: cap as _,
			usage: usage | wgpu::BufferUsages::COPY_DST,
			mapped_at_creation: false,
		});

		Self {
			writer: VecWriter::new(),
			buffer,
		}
	}

	pub fn flush(&mut self, gfx: &GfxContext) {
		gfx.queue.write_buffer(&self.buffer, 0, self.writer.bytes());
		self.writer.bytes.clear();
	}

	// TODO: Immediate mode writer
	pub fn push(&mut self, align: usize, bytes: &[u8]) -> wgpu::BufferBinding {
		self.writer.align_to(align);
		let head = self.writer.location();
		self.writer.write(bytes);

		wgpu::BufferBinding {
			buffer: &self.buffer,
			offset: head as _,
			size: Some(wgpu::BufferSize::new(bytes.len() as _).unwrap()),
		}
	}

	pub fn buffer(&self) -> &wgpu::Buffer {
		&self.buffer
	}
}
