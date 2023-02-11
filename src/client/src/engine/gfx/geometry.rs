use crucible_common::voxel::math::{Axis3, BlockFace, Sign};
use typed_glam::{
	glam::Vec2,
	traits::{NumericVector2, NumericVector3},
};

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
pub const QUAD_UVS: [Vec2; 4] = [
	Vec2::new(0.0, 1.0),
	Vec2::new(1.0, 1.0),
	Vec2::new(1.0, 0.0),
	Vec2::new(0.0, 0.0),
];

pub fn quad_to_tris<V: Copy>([a, b, c, d]: [V; 4]) -> [[V; 3]; 2] {
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
	[[a, b, c], [a, c, d]]
}

pub fn flip_quad_winding<V>([a, b, c, d]: [V; 4]) -> [V; 4] {
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
	[b, a, d, c]
}

pub fn scaled_aabb_quad<V, S>(origin: V, size: S, facing: BlockFace) -> [V; 4]
where
	V: NumericVector3,
	S: Into<(V::Comp, V::Comp)>,
{
	let (axis, sign) = facing.decompose();
	let (w, h) = size.into();

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

			[origin + z, origin, origin + y, origin + y + z]
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

			[origin, origin + z, origin + x + z, origin + x]
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

			[origin, origin + x, origin + x + y, origin + y]
		}
	};

	// Flip the winding order if the quad is actually facing the positive direction:
	if sign == Sign::Positive {
		flip_quad_winding(quad)
	} else {
		quad
	}
}

pub fn face_size_given_volume<V, S>(volume: V, axis: Axis3) -> S
where
	V: NumericVector3,
	S: NumericVector2<Comp = V::Comp>,
{
	// We're essentially matching the conventions defined in `scaled_aabb_quad`.
	match axis {
		Axis3::X => S::new(volume.z(), volume.y()),
		Axis3::Y => S::new(volume.x(), volume.z()),
		Axis3::Z => S::new(volume.x(), volume.y()),
	}
}

pub fn aabb_quad<V: NumericVector3>(origin: V, facing: BlockFace) -> [V; 4] {
	let unit = V::ONE.x();
	scaled_aabb_quad(origin, (unit, unit), facing)
}
