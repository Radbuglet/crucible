use crate::engine::context::GfxContext;
use crucible_core::foundation::prelude::*;
use std::fmt::{Display, Formatter};

pub struct ContigMesh {
	buffer: wgpu::Buffer,
	is_mapped: bool,
	mirror: Vec<MeshEntry>,
	len: usize,
}

#[derive(Copy, Clone)]
struct MeshEntry {
	entity: Entity,
	size: usize,
}

impl ContigMesh {
	pub fn new(gfx: &GfxContext) -> Self {
		let buffer = gfx.device.create_buffer(&wgpu::BufferDescriptor {
			label: Some("memory allocator at home"),
			size: 64_000_000, // Casual 64mb heap. Don't worry about it.
			usage: wgpu::BufferUsages::MAP_WRITE
				| wgpu::BufferUsages::MAP_READ
				| wgpu::BufferUsages::VERTEX,
			mapped_at_creation: false,
		});

		Self {
			buffer,
			is_mapped: false,
			mirror: Vec::new(),
			len: 0,
		}
	}

	pub async fn ensure_mapped(&mut self) {
		if !self.is_mapped {
			self.buffer
				.slice(..)
				.map_async(wgpu::MapMode::Write)
				.await
				.unwrap();
			self.is_mapped = true;
		}
	}

	pub fn end_updating(&mut self) {
		if self.is_mapped {
			self.buffer.unmap();
			self.is_mapped = false;
		}
	}

	pub async fn add(&mut self, world: &World, entity: Entity, data: &[u8]) {
		// Remove any existing entries so we can replace it.
		let _ = self.try_remove(world, entity);

		// Lazily map the buffer. This ensures that we don't have to create multiple instances of the
		// same buffer (wasting space) or block on frames where mesh data is not being updated (wasting
		// time and limiting FPS).
		self.ensure_mapped().await;

		// Determine the range of affected bytes
		let write_range = self.len..(self.len.checked_add(data.len()).unwrap());

		if !write_range.is_empty() {
			// Get mapped buffer
			let mut mapped = self
				.buffer
				.slice((write_range.start as u64)..(write_range.end as u64))
				.get_mapped_range_mut();

			// Modify the buffer
			mapped.copy_from_slice(data);
		}

		// Update length
		self.len += data.len();

		// Update mirror
		self.mirror.push(MeshEntry {
			entity,
			size: data.len(),
		});
	}

	pub async fn try_remove(
		&mut self,
		_world: &World,
		entity: Entity,
	) -> Result<(), MissingEntityError> {
		// Scan for target entity and update mirror
		let mut offset = 0;
		let mut entry_size = None;
		self.mirror.retain(|entry| {
			// TODO: Cleanup dead entries while we're at it.

			// We can only remove one element.
			if entry_size.is_some() {
				return true;
			}

			// Are we at the target?
			if entity == entry.entity {
				entry_size = Some(entry.size);
				return false;
			}

			// Otherwise, sum offset and preserve
			offset += entry.size;
			true
		});

		let entry_size = entry_size.ok_or(MissingEntityError)?;

		// Lazily map the buffer. See reasoning for why we do this in [Self::add].
		self.ensure_mapped().await;

		// Get mapped buffer
		let mut mapped = self.buffer.slice(..).get_mapped_range_mut();

		// Modify the buffer
		mapped.copy_within((offset + entry_size)..self.len, offset);
		self.len -= entry_size;

		Ok(())
	}

	pub async fn remove(&mut self, world: &World, entity: Entity) {
		self.try_remove(world, entity).await.unwrap()
	}

	pub fn len_bytes(&self) -> usize {
		self.len
	}

	pub fn len_entries(&self) -> usize {
		self.mirror.len()
	}

	pub fn buffer(&self) -> &wgpu::Buffer {
		&self.buffer
	}
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct MissingEntityError;

impl Display for MissingEntityError {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		write!(
			f,
			"attempted to remove a mesh entry from an entity which did not exist"
		)
	}
}
