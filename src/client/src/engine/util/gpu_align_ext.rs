use glsl_layout::{Std140, Uniform};
use std::mem;

pub fn align_up(offset: usize, align: usize) -> usize {
	(offset + align) & !align
}

pub fn convert_slice<T: Uniform>(values: &[T]) -> Vec<u8> {
	let mut collector = Vec::with_capacity(values.len() * mem::size_of::<T::Std140>());
	for value in values {
		for byte in value.std140().as_raw() {
			collector.push(*byte);
		}
	}
	collector
}
