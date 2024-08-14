use std::{cell::RefCell, fmt};

use crucible_utils::mem::DropBump;
use main_loop::GfxContext;
use typed_wgpu::{BufferAddress, GpuStruct};

use super::DynamicBuffer;

#[derive(Default)]
pub struct MultiPassDriver {
    bump: DropBump<'static>,
    offset_buff: RefCell<Vec<wgpu::BufferAddress>>,
}

impl fmt::Debug for MultiPassDriver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MultiPassDriver").finish_non_exhaustive()
    }
}

impl MultiPassDriver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn drive<'p>(
        &'p self,
        gfx: &GfxContext,
        pass: &mut wgpu::RenderPass<'p>,
        buffer: &mut DynamicBuffer,
        mut f: impl FnMut(&mut MultiPass<'_, 'p>),
    ) {
        let mut offset_buff = self
            .offset_buff
            .try_borrow_mut()
            .expect("cannot call `drive` reentrantly");

        offset_buff.clear();

        f(&mut MultiPass(MultiPassInner::Buffer {
            buffer,
            offsets: &mut offset_buff,
        }));

        f(&mut MultiPass(MultiPassInner::Pass {
            buffer: buffer.finish(gfx),
            bump: &self.bump,
            pass,
            offsets: &offset_buff,
        }));
    }
}

#[derive(Debug)]
pub struct MultiPass<'a, 'p>(MultiPassInner<'a, 'p>);

#[derive(Debug)]
enum MultiPassInner<'a, 'p> {
    Buffer {
        buffer: &'a mut DynamicBuffer,
        offsets: &'a mut Vec<wgpu::BufferAddress>,
    },
    Pass {
        buffer: &'a wgpu::Buffer,
        bump: &'p DropBump<'static>,
        pass: &'a mut wgpu::RenderPass<'p>,
        offsets: &'a [wgpu::BufferAddress],
    },
}

impl<'a, 'p> MultiPass<'a, 'p> {
    pub fn write(
        &mut self,
        f: impl FnOnce(&mut DynamicBuffer) -> wgpu::BufferAddress,
    ) -> wgpu::BufferAddress {
        match &mut self.0 {
            MultiPassInner::Buffer { buffer, offsets } => {
                let offset = f(buffer);
                offsets.push(offset);
                offset
            }
            MultiPassInner::Pass { offsets, .. } => {
                let (&first, new_offsets) = offsets.split_first().expect("write calls mismatched!");
                *offsets = new_offsets;
                first
            }
        }
    }

    pub fn write_typed<T: GpuStruct>(
        &mut self,
        gfx: &GfxContext,
        f: impl FnOnce() -> T::Pod,
    ) -> BufferAddress<T> {
        BufferAddress::wrap(self.write(|writer| {
            let align = gfx
                .requested_limits
                .min_uniform_buffer_offset_alignment
                .into();

            writer.align(align);
            let start = writer.len();
            writer.extend(bytemuck::bytes_of(&f()));
            writer.align(align);
            start
        }))
    }

    pub fn alloc<T: 'static>(&mut self, f: impl FnOnce(&wgpu::Buffer) -> T) -> Option<&'p T> {
        match &mut self.0 {
            MultiPassInner::Buffer { .. } => None,
            MultiPassInner::Pass { buffer, bump, .. } => Some(bump.alloc(f(buffer))),
        }
    }

    pub fn draw(&mut self, f: impl FnOnce(&mut wgpu::RenderPass<'p>)) {
        if let MultiPassInner::Pass { pass, .. } = &mut self.0 {
            f(pass);
        }
    }
}
