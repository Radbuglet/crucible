use crucible_common::voxel::math::{Axis3, BlockFace, Sign};
use typed_glam::{glam::Vec2, traits::NumericVector3};

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

pub fn aabb_quad<V: NumericVector3>(origin: V, facing: BlockFace) -> [V; 4] {
	let (axis, sign) = facing.decompose();

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
			//    d  |
			//    |  b 0
			//    | /         ---> -x
			//    |/
			//    a +z
			//
			[origin + V::Z, origin, origin + V::Y, origin + V::Y + V::Z]
		}
		Axis3::Y => {
			// A quad facing the negative y direction looks like this:
			//
			//  +x        0
			//    d------a       |
			//   /      /        |
			//  /      /         ↓ -y
			// c------b
			//         +z
			//
			[origin, origin + V::Z, origin + V::X + V::Z, origin + V::X]
		}
		Axis3::Z => {
			// A quad facing the negative z direction looks like this:
			//
			//              +y
			//      c------d
			//      |      |     ^ -z
			//      |      |    /
			//      b------a   /
			//    +x        0
			//
			[origin, origin + V::X, origin + V::X + V::Y, origin + V::Y]
		}
	};

	// Flip the winding order if the quad is actually facing the positive direction:
	if sign == Sign::Positive {
		flip_quad_winding(quad)
	} else {
		quad
	}
}
