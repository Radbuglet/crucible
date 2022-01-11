//! Utilities to marshall between vector types.

use cgmath::{Vector1, Vector2, Vector3, Vector4};
use winit::dpi::{LogicalSize, PhysicalSize};

pub trait VecConvert: Sized {
	type Vector;

	fn to_vec(self) -> Self::Vector;
	fn from_vec(vec: Self::Vector) -> Self;
	fn convert_vec<T: VecConvert<Vector = Self::Vector>>(self) -> T {
		T::from_vec(self.to_vec())
	}
}

// VectorN <-> VectorN (allows blind `into` without having to worry about no-op special case)
macro vec_convert_n($target:ident) {
	impl<T> VecConvert for $target<T> {
		type Vector = $target<T>;

		fn to_vec(self) -> Self::Vector {
			self
		}

		fn from_vec(vec: Self::Vector) -> Self {
			vec
		}
	}
}

vec_convert_n!(Vector1);
vec_convert_n!(Vector2);
vec_convert_n!(Vector3);
vec_convert_n!(Vector4);

// LogicalSize <-> Vector2
impl<T> VecConvert for LogicalSize<T> {
	type Vector = Vector2<T>;

	fn to_vec(self) -> Self::Vector {
		Vector2::new(self.width, self.height)
	}

	fn from_vec(vec: Self::Vector) -> Self {
		LogicalSize::new(vec.x, vec.y)
	}
}

// PhysicalSize <-> Vector2
impl<T> VecConvert for PhysicalSize<T> {
	type Vector = Vector2<T>;

	fn to_vec(self) -> Self::Vector {
		Vector2::new(self.width, self.height)
	}

	fn from_vec(vec: Self::Vector) -> Self {
		PhysicalSize::new(vec.x, vec.y)
	}
}

// wgpu::Extent3 <-> Vector3
impl VecConvert for wgpu::Extent3d {
	type Vector = Vector3<u32>;

	fn to_vec(self) -> Self::Vector {
		Vector3::new(self.width, self.height, self.depth_or_array_layers)
	}

	fn from_vec(vec: Self::Vector) -> Self {
		wgpu::Extent3d {
			width: vec.x,
			height: vec.y,
			depth_or_array_layers: vec.z,
		}
	}
}
