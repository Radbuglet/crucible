use crate::GfxContext;
use crucible_core::foundation::prelude::*;
use std::ops::Range;
use wgpu::MapMode;

pub struct ContigMesh {
	buffer: wgpu::Buffer,
	ranges: Storage<Range<usize>>,
	entities: Vec<Entity>,
	len: usize,
}

impl ContigMesh {
	pub fn new(gfx: &GfxContext) -> Self {
		let buffer = gfx.device.create_buffer(&wgpu::BufferDescriptor {
			label: Some("memory allocator at home"),
			size: 16_000_000, // Casual 16mb heap. Don't worry about it.
			usage: wgpu::BufferUsages::MAP_WRITE
				| wgpu::BufferUsages::MAP_READ
				| wgpu::BufferUsages::VERTEX,
			mapped_at_creation: false,
		});

		Self {
			buffer,
			ranges: Storage::new(),
			entities: Vec::new(),
			len: 0,
		}
	}

	pub async fn begin_updating(&mut self) -> Result<(), wgpu::BufferAsyncError> {
		self.buffer.slice(..).map_async(MapMode::Write).await
	}

	pub fn end_updating(&mut self) {
		self.buffer.unmap();
	}

	pub fn add(&mut self, world: &World, entity: Entity, data: &[u8]) {
		// Remove any existing entries so we can replace it.
		if self.ranges.try_get(world, entity).is_some() {
			self.remove(world, entity);
		}

		// Get mapped buffer
		let mut mapped = self.buffer.slice(..).get_mapped_range_mut();

		// Determine the range of affected bytes
		let write_range = self.len..(self.len.checked_add(data.len()).unwrap());

		// Modify the buffer
		mapped[write_range.clone()].copy_from_slice(data);

		// Update length
		self.len += data.len();

		// Update mirror
		self.ranges.insert(world, entity, write_range);
		self.entities.push(entity);
	}

	pub fn remove(&mut self, _world: &World, _entity: Entity) {
		todo!()

		// // Get mapped buffer
		// let mut mapped = self.buffer.slice(..).get_mapped_range_mut();
		//
		// // Determine the range of affected bytes
		// let write_range = *self.ranges.get(world, entry);
		//
		// // Modify the buffer
		// mapped.copy_within(write_range.end..self.len, write_range.start);
		//
		// // Update length
		// self.len -= write_range.len();
		//
		// //
		//
	}

	pub fn len_bytes(&self) -> usize {
		self.len
	}

	pub fn len_entries(&self) -> usize {
		self.entities.len()
	}

	pub fn buffer(&self) -> &wgpu::Buffer {
		&self.buffer
	}
}
