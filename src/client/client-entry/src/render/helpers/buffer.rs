use std::sync::{mpsc, Arc};

use crucible_utils::polyfill::OptionExt;
use main_loop::GfxContext;
use wgpu::util::DeviceExt;

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
pub struct ExactBufferRequest<'a> {
    pub label: Option<&'a str>,
    pub size: wgpu::BufferAddress,
    pub usage: wgpu::BufferUsages,
}

impl MappableBufferRequest for ExactBufferRequest<'_> {
    fn create_new_buffer(&mut self, gfx: &GfxContext) -> wgpu::Buffer {
        gfx.device.create_buffer(&wgpu::BufferDescriptor {
            label: self.label,
            size: self.size,
            usage: self.usage,
            mapped_at_creation: true,
        })
    }

    fn is_compatible(&mut self, buffer: &wgpu::Buffer) -> bool {
        buffer.size() == self.size && buffer.usage() == self.usage
    }
}

// === DynamicBuffer === //

#[derive(Debug)]
pub struct DynamicBuffer {
    label: Option<String>,
    usage: wgpu::BufferUsages,
    buffer: Option<wgpu::Buffer>,
    data: Vec<u8>,
}

impl DynamicBuffer {
    pub fn new(label: Option<impl Into<String>>, usage: wgpu::BufferUsages) -> Self {
        Self {
            label: label.map(|v| v.into()),
            usage,
            buffer: None,
            data: Vec::new(),
        }
    }

    pub fn data(&mut self) -> &mut Vec<u8> {
        &mut self.data
    }

    pub fn finish(&mut self, gfx: &GfxContext) -> &wgpu::Buffer {
        let buffer = if self.buffer.is_none_or(|b| b.size() < self.len()) {
            self.buffer.insert(
                gfx.device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: self.label.as_deref(),
                        usage: self.usage,
                        contents: &self.data,
                    }),
            )
        } else {
            // Using unwrap because match unfortunately can't express conditions on only half of the
            // binding (i.e. `None | (Some(v) if <condition involving `v`>)` is malformed).
            let buffer = self.buffer.as_mut().unwrap();
            gfx.queue.write_buffer(buffer, 0, &self.data);
            buffer
        };
        self.data.clear();
        buffer
    }

    // === Helpers === //

    pub fn len(&self) -> wgpu::BufferAddress {
        self.data.len() as _
    }

    pub fn extend_from_slice(&mut self, data: &[u8]) {
        self.data.extend_from_slice(data);
    }

    pub fn pad(&mut self, amount: wgpu::BufferAddress) {
        self.extend((0..amount).map(|_| 0u8));
    }

    pub fn align(&mut self, align: wgpu::BufferAddress) {
        assert!(align.is_power_of_two());

        let remaining = align - (self.len() & (align - 1));
        if remaining == align {
            return;
        }

        self.pad(remaining);
    }
}

impl Extend<u8> for DynamicBuffer {
    fn extend<T: IntoIterator<Item = u8>>(&mut self, iter: T) {
        self.data.extend(iter);
    }
}

impl<'a> Extend<&'a u8> for DynamicBuffer {
    fn extend<T: IntoIterator<Item = &'a u8>>(&mut self, iter: T) {
        self.data.extend(iter.into_iter().copied());
    }
}
