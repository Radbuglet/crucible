//! Utilities to marshall between Vector types.

use cgmath::{Vector1, Vector2, Vector3, Vector4};
use winit::dpi::{LogicalSize, PhysicalSize};

pub trait VecConvert {
	type Vector;

	fn to_vec(self) -> Self::Vector;
	fn from_vec(vec: Self::Vector) -> Self;
}

pub trait VecConvertExt: VecConvert {
	fn into<T: VecConvert<Vector = Self::Vector>>(self) -> T;
}

impl<O: VecConvert> VecConvertExt for O {
	fn into<T: VecConvert<Vector = Self::Vector>>(self) -> T {
		T::from_vec(self.to_vec())
	}
}

// VectorN <-> VectorN (allows blind into without having to worry about identity conversions)
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
