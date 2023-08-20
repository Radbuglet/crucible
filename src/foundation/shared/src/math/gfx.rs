use crucible_util::mem::array::{map_arr, zip_arr};
use typed_glam::{
	glam::{self, Vec2},
	traits::SignedNumericVector3,
	typed::{FlavorCastFrom, TypedVector, VecFlavor},
};

use super::{AaQuad, Axis3, Sign};

// === Quad === //

#[derive(Debug, Copy, Clone)]
pub struct Quad<V>(pub [V; 4]);

// Quads, from a front-view, are laid out as follows:
//
//      d---c
//      |   |
//      a---b
//
// Textures, meanwhile, are laid out as follows:
//
// (0,0)     (1,0)
//      *---*
//      |   |
//      *---*
// (0,1)     (1,1)
//
// Hence:
pub const QUAD_UVS: Quad<Vec2> = Quad([
	Vec2::new(0.0, 1.0),
	Vec2::new(1.0, 1.0),
	Vec2::new(1.0, 0.0),
	Vec2::new(0.0, 0.0),
]);

impl<V> Quad<V> {
	pub fn flip_winding(self) -> Self {
		let [a, b, c, d] = self.0;

		// If we have a quad like this facing us:
		//
		// d---c
		// |   |
		// a---b
		//
		// From the other side, it looks like this:
		//
		// c---d
		// |   |
		// b---a
		//
		// If we preserve quad UV rules, we get the new ordering:
		Self([b, a, d, c])
	}

	pub fn to_tris(self) -> [Tri<V>; 2]
	where
		V: Copy,
	{
		let [a, b, c, d] = self.0;

		// If we have a quad like this facing us:
		//
		// d---c
		// |   |
		// a---b
		//
		// We can split it up into triangles preserving the winding order like so:
		//
		//       3
		//      c
		//     /|
		//    / |
		//   /  |
		//  a---b
		// 1     2
		//
		// ...and:
		//
		// 3     2
		//  d---c
		//  |  /
		//  | /
		//  |/
		//  a
		// 1
		[Tri([a, b, c]), Tri([a, c, d])]
	}

	pub fn zip<R>(self, rhs: Quad<R>) -> Quad<(V, R)> {
		Quad(zip_arr(self.0, rhs.0))
	}

	pub fn map<R>(self, f: impl FnMut(V) -> R) -> Quad<R> {
		Quad(map_arr(self.0, f))
	}
}

// === Tri === //

#[derive(Debug, Copy, Clone)]
pub struct Tri<V>(pub [V; 3]);

// === AaQuad Extensions === //

impl<V: SignedNumericVector3> AaQuad<V> {
	pub fn as_quad_ccw(&self) -> Quad<V> {
		let (axis, sign) = self.face.decompose();
		let (w, h) = self.size;
		let origin = self.origin;

		// Build the quad with a winding order assumed to be for a negative facing quad.
		let quad = match axis {
			// As a reminder, our coordinate system is y-up right-handed and looks like this:
			//
			//     +y
			//      |
			// +x---|
			//     /
			//   +z
			//
			Axis3::X => {
				// A quad facing the negative x direction looks like this:
				//
				//       c +y
				//      /|
				//     / |
				//    d  |                --
				//    |  b 0              /
				//    | /     ---> -x    / size.x
				//    |/                /
				//    a +z            --
				//
				let z = V::Z * V::splat(w);
				let y = V::Y * V::splat(h);

				Quad([origin + z, origin, origin + y, origin + y + z])
			}
			Axis3::Y => {
				// A quad facing the negative y direction looks like this:
				//
				//  +x        0
				//    d------a       |        --
				//   /      /        |        / size.x
				//  /      /         ↓ -y    /
				// c------b                --
				//         +z
				//
				// |______| size.y
				//
				let x = V::X * V::splat(w);
				let z = V::Z * V::splat(h);

				Quad([origin, origin + z, origin + x + z, origin + x])
			}
			Axis3::Z => {
				// A quad facing the negative z direction looks like this:
				//
				//              +y
				//      c------d            --
				//      |      |     ^ -z    | size.y
				//      |      |    /        |
				//      b------a   /        --
				//    +x        0
				//
				//      |______| size.x
				//
				let x = V::X * V::splat(w);
				let y = V::Y * V::splat(h);

				Quad([origin, origin + x, origin + x + y, origin + y])
			}
		};

		// Flip the winding order if the quad is actually facing the positive direction:
		if sign == Sign::Positive {
			quad.flip_winding()
		} else {
			quad
		}
	}
}

// === Color3 === //

pub type Color3 = TypedVector<Color3Flavor>;

#[non_exhaustive]
pub struct Color3Flavor;

impl VecFlavor for Color3Flavor {
	type Backing = glam::Vec3;

	const DEBUG_NAME: &'static str = "Color3";
}

impl FlavorCastFrom<f32> for Color3Flavor {
	fn cast_from(vec: f32) -> Color3
	where
		Self: VecFlavor,
	{
		Color3::splat(vec)
	}
}

impl FlavorCastFrom<glam::Vec3> for Color3Flavor {
	fn cast_from(v: glam::Vec3) -> Color3 {
		Color3::from_glam(v)
	}
}

// === Color4 === //

pub type Color4 = TypedVector<Color4Flavor>;

#[non_exhaustive]
pub struct Color4Flavor;

impl VecFlavor for Color4Flavor {
	type Backing = glam::Vec4;

	const DEBUG_NAME: &'static str = "Color4";
}

impl FlavorCastFrom<f32> for Color4Flavor {
	fn cast_from(vec: f32) -> Color4
	where
		Self: VecFlavor,
	{
		Color4::splat(vec)
	}
}

impl FlavorCastFrom<glam::Vec4> for Color4Flavor {
	fn cast_from(v: glam::Vec4) -> Color4 {
		Color4::from_glam(v)
	}
}
