use cgmath::{Matrix3, Matrix4, Vector2, Vector3, Vector4};
use crucible_core::util::pod::{FixedPodSerializable, PodSubWriter, PodWriter};
use crucible_core::util::pointer::layout_from_size_and_align;
use crucible_core::util::wrapper::{new_wrapper, Wrapper};
use std::alloc::Layout;

new_wrapper! {
	/// A wrapper which marks a specific primitive as being serialized in the [std140] layout.
	///
	/// [std140]: https://github.com/sotrh/learn-wgpu/blob/ea77676365d123152c8f0afdf7df57a6b49ca859/docs/showcase/alignment/README.md
	pub Std140;
}

// === Scalars === //

macro scalar_derive(layout = $layout:expr, types = [$($path:path),*$(,)?]$(,)?) {$(
	impl FixedPodSerializable for Std140<$path> {
		const LAYOUT: Layout = $layout;

		fn write<T: ?Sized + PodWriter>(&self, writer: &mut PodSubWriter<T>) {
			// This is equivalent to the more direct `write_unaligned` form: https://godbolt.org/z/Tx5hqG9Kv
			writer.write(self.to_raw().to_ne_bytes().as_slice());
		}
	}
)*}

scalar_derive!(
	layout = layout_from_size_and_align(4, 4),
	types = [u32, i32, f32],
);

scalar_derive!(layout = layout_from_size_and_align(8, 8), types = [f64],);

impl FixedPodSerializable for Std140<bool> {
	const LAYOUT: Layout = layout_from_size_and_align(4, 4);

	fn write<T: ?Sized + PodWriter>(&self, writer: &mut PodSubWriter<T>) {
		writer.write(&Std140(if self.to_raw() { 1 } else { 0 }));
	}
}

// === Vectors === //

macro vec_derive(
	vec_ty = $vec:ident,
	comp_count = $comp_count:tt,
	layout = $layout:expr,
	generics = [$($generic:ty),*$(,)?]$(,)?
) {$(
	impl FixedPodSerializable for Std140<$vec<$generic>> {
		const LAYOUT: Layout = $layout;

		fn write<T: ?Sized + PodWriter>(&self, writer: &mut PodSubWriter<T>) {
			let comps: [$generic; $comp_count] = self.to_raw().into();
			for comp in comps {
				writer.write(&Std140(comp));
			}
		}
	}
)*}

vec_derive!(
	vec_ty = Vector2,
	comp_count = 2,
	layout = layout_from_size_and_align(8, 8),
	generics = [i32, u32, bool, f32],
);

vec_derive!(
	vec_ty = Vector3,
	comp_count = 3,
	layout = layout_from_size_and_align(12, 16),
	generics = [i32, u32, bool, f32],
);

vec_derive!(
	vec_ty = Vector4,
	comp_count = 4,
	layout = layout_from_size_and_align(16, 16),
	generics = [i32, u32, bool, f32],
);

vec_derive!(
	vec_ty = Vector2,
	comp_count = 2,
	layout = layout_from_size_and_align(16, 16),
	generics = [f64],
);

vec_derive!(
	vec_ty = Vector3,
	comp_count = 3,
	layout = layout_from_size_and_align(24, 32),
	generics = [f64],
);

vec_derive!(
	vec_ty = Vector4,
	comp_count = 4,
	layout = layout_from_size_and_align(32, 32),
	generics = [f64],
);

// === Matrices === //

impl FixedPodSerializable for Std140<Matrix3<f32>> {
	const LAYOUT: Layout = layout_from_size_and_align(3 * 16, 16);

	fn write<T: ?Sized + PodWriter>(&self, writer: &mut PodSubWriter<T>) {
		let raw = self.to_raw();
		writer.write(&Std140(raw.x));
		writer.write(&Std140(raw.y));
		writer.write(&Std140(raw.z));
	}
}

impl FixedPodSerializable for Std140<Matrix4<f32>> {
	const LAYOUT: Layout = layout_from_size_and_align(4 * 16, 16);

	fn write<T: ?Sized + PodWriter>(&self, writer: &mut PodSubWriter<T>) {
		let raw = self.to_raw();
		writer.write(&Std140(raw.x));
		writer.write(&Std140(raw.y));
		writer.write(&Std140(raw.z));
		writer.write(&Std140(raw.w));
	}
}
