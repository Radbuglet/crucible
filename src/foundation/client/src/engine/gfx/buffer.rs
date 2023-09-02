use std::sync::{mpsc, Arc};

use crevice::std430::AsStd430;
use crucible_util::{
	debug::label::{DebugLabel, ReifiedDebugLabel},
	lang::polyfill::OptionPoly,
};

use crate::engine::io::gfx::GfxContext;

// === MappableBufferPool === //

#[derive(Debug)]
pub struct MappableBufferPool {
	mappable_sender: mpsc::Sender<Arc<wgpu::Buffer>>,
	mappable_receiver: mpsc::Receiver<Arc<wgpu::Buffer>>,
	open_buffers: Vec<Arc<wgpu::Buffer>>,
}

impl Default for MappableBufferPool {
	fn default() -> Self {
		let (mappable_sender, mappable_receiver) = mpsc::channel();
		Self {
			mappable_sender,
			mappable_receiver,
			open_buffers: Vec::new(),
		}
	}
}

impl MappableBufferPool {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn acquire(
		&mut self,
		gfx: &GfxContext,
		request: &mut impl MappableBufferRequest,
	) -> Arc<wgpu::Buffer> {
		self.receive_mappables(|buffer| request.is_compatible(buffer));

		if let Some(buffer) = self.open_buffers.pop() {
			buffer
		} else {
			Arc::new(request.create_new_buffer(gfx))
		}
	}

	pub fn release(&mut self, buffer: Arc<wgpu::Buffer>) {
		let mappable_sender = self.mappable_sender.clone();
		Arc::clone(&buffer)
			.slice(..)
			.map_async(wgpu::MapMode::Write, move |_| {
				let _ = mappable_sender.send(buffer);
			});
	}

	fn receive_mappables(&mut self, mut is_compatible: impl FnMut(&wgpu::Buffer) -> bool) {
		self.open_buffers.retain(|buffer| is_compatible(buffer));

		while let Ok(buffer) = self.mappable_receiver.try_recv() {
			if is_compatible(&buffer) {
				self.open_buffers.push(buffer);
			}
		}
	}
}

pub trait MappableBufferRequest {
	fn create_new_buffer(&mut self, gfx: &GfxContext) -> wgpu::Buffer;

	fn is_compatible(&mut self, buffer: &wgpu::Buffer) -> bool;
}

#[derive(Debug, Copy, Clone)]
pub struct StaticBufferRequest<'a> {
	pub label: Option<&'a str>,
	pub size: wgpu::BufferAddress,
	pub usage: wgpu::BufferUsages,
}

impl MappableBufferRequest for StaticBufferRequest<'_> {
	fn create_new_buffer(&mut self, gfx: &GfxContext) -> wgpu::Buffer {
		gfx.device.create_buffer(&wgpu::BufferDescriptor {
			label: self.label,
			size: self.size,
			usage: self.usage,
			mapped_at_creation: true,
		})
	}

	fn is_compatible(&mut self, buffer: &wgpu::Buffer) -> bool {
		debug_assert_eq!(buffer.size(), self.size);
		debug_assert_eq!(buffer.usage(), self.usage);
		true
	}
}

// === DynamicBuffer === //

#[derive(Debug)]
pub struct DynamicBuffer {
	// Buffer state
	label: ReifiedDebugLabel,
	usage: wgpu::BufferUsages,
	buffer: Option<wgpu::Buffer>,

	// Staging state
	chunk_pool: MappableBufferPool,
	written_chunks: Vec<Arc<wgpu::Buffer>>,
	curr_chunk_len: usize,
	chunk_size: usize,
}

impl DynamicBuffer {
	pub fn new(label: impl DebugLabel, usage: wgpu::BufferUsages, chunk_size: usize) -> Self {
		let chunk_size = chunk_size & !(wgpu::COPY_BUFFER_ALIGNMENT as usize - 1);
		let chunk_size = chunk_size.max(wgpu::COPY_BUFFER_ALIGNMENT as usize);
		let usage = usage | wgpu::BufferUsages::COPY_DST;

		Self {
			label: label.reify(),
			usage,
			buffer: None,
			chunk_pool: MappableBufferPool::new(),
			written_chunks: Vec::new(),
			curr_chunk_len: chunk_size,
			chunk_size,
		}
	}

	pub fn push(&mut self, gfx: &GfxContext, data: &[u8]) {
		// Split off the data that can be pushed into the most recent chunk
		let first_chunk_data_len = self.chunk_size - self.curr_chunk_len;
		let first_chunk_data_len = first_chunk_data_len.min(data.len());
		let (first_chunk_data, remainder_data) = data.split_at(first_chunk_data_len);

		// If that chunk is not full, write to it.
		if !first_chunk_data.is_empty() {
			self.written_chunks
				.last()
				.unwrap()
				.slice(..)
				.get_mapped_range_mut()[self.curr_chunk_len..][..first_chunk_data_len]
				.copy_from_slice(first_chunk_data);

			self.curr_chunk_len += first_chunk_data_len;
		}

		// For the remaining chunks...
		for chunk in remainder_data.chunks(self.chunk_size as usize) {
			// Acquire a buffer to store it.
			let chunk_buf = self.chunk_pool.acquire(
				gfx,
				&mut StaticBufferRequest {
					label: Some("DynamicBuffer staging buffer"),
					size: self.chunk_size as wgpu::BufferAddress,
					usage: wgpu::BufferUsages::MAP_WRITE | wgpu::BufferUsages::COPY_SRC,
				},
			);

			// Write data to it
			chunk_buf.slice(..).get_mapped_range_mut()[..chunk.len()].copy_from_slice(chunk);

			// Make it the most active chunk
			self.written_chunks.push(chunk_buf);

			// And update the current chunk length so we can continue off on this chunk.
			self.curr_chunk_len = chunk.len();
		}
	}

	pub fn ensure_appropriate_buffer(&mut self, gfx: &GfxContext) {
		let chunk_size = self.chunk_size as wgpu::BufferAddress;

		// Determine the size required to store all chunks, including their trailing bytes.
		let size = chunk_size * self.written_chunks.len() as wgpu::BufferAddress;

		// Reuse the existing buffer if it has an appropriate size or recreate it if necessary.
		if self
			.buffer
			.as_ref()
			.is_none_or(|b| b.size() < size || b.size() > size * 2)
		{
			self.buffer = Some(gfx.device.create_buffer(&wgpu::BufferDescriptor {
				label: self.label.as_deref(),
				size,
				usage: self.usage,
				mapped_at_creation: false,
			}));
		}
	}

	pub fn buffer(&self) -> &wgpu::Buffer {
		self.buffer.as_ref().unwrap()
	}

	pub fn len(&self) -> wgpu::BufferAddress {
		if self.written_chunks.is_empty() {
			0
		} else {
			(self.written_chunks.len() - 1) as wgpu::BufferAddress
				* self.chunk_size as wgpu::BufferAddress
				+ self.curr_chunk_len as wgpu::BufferAddress
		}
	}

	pub fn upload(&mut self, gfx: &GfxContext, cb: &mut wgpu::CommandEncoder) {
		// Ensure that our buffer is big enough
		self.ensure_appropriate_buffer(gfx);

		let chunk_size = self.chunk_size as wgpu::BufferAddress;
		let buffer = self.buffer.as_ref().unwrap();

		// Copy every chunk into the destination buffer in their entirety.
		let mut dst_offset = 0;

		for chunk in &self.written_chunks {
			chunk.unmap();
			cb.copy_buffer_to_buffer(&chunk, 0, &buffer, dst_offset, chunk_size);
			dst_offset += chunk_size;
		}
	}

	pub fn reset_and_release(&mut self) {
		for chunk in self.written_chunks.drain(..) {
			self.chunk_pool.release(chunk);
		}
		self.curr_chunk_len = self.chunk_size;
	}
}

pub fn buffer_len_to_count<S: AsStd430>(buffer_len: wgpu::BufferAddress) -> u32 {
	(buffer_len / S::std430_size_static() as wgpu::BufferAddress) as u32
}
