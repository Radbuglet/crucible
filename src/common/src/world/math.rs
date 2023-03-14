//! ## Coordinate System
//!
//! Crucible uses a **y-up right-handed** coordinate system. Thus, our axes look like this:
//!
//! ```plain_text
//!     +y
//!      |
//! +x---|
//!     /
//!   +z
//! ```
//!
//! This coordinate system is nice because it works well with graphics conventions. For example,
//! because object depth increases along the positive `z` direction, camera view matrices transform
//! positive `z` local-space vectors into the forward direction. Thus, it is fair to call:
//!
//! ```plain_text
//!              +y (up)
//!              | +z (forward)
//!              |/
//! (left) -x----|---+x (right)
//!             /|
//!            / |
//!    (back) -z  -y (down)
//! ```
//!
//! ...which feels pretty intuitive.
//!
//! ## Relation to Voxels
//!
//! There are four major typed vector types to represent Crucible's various coordinate systems:
//!
//! 1. [`WorldVec`]: a block vector in world-space i.e. global block coordinates.
//! 2. [`ChunkVec`]: a chunk position vector i.e. the coordinate of a chunk.
//! 3. [`BlockVec`]: a block vector in chunk-relative-space i.e. the coordinate of a block relative
//!    to a chunk.
//! 4. [`EntityVec`]: an entity vector in world-space i.e. global entity coordinates.
//!
//! ##### `EntityVec` and `WorldVec`
//!
//! A voxel takes up the size `EntityVec::ONE`. For a given voxel at `(x, y, z)` in world-vec
//! coordinates, the corresponding entity-vec at `(x, y, z)` will be positioned at the bottom most-
//! negative corner of the block:
//!
//! ```plain_text
//!              <1>
//!          *---------*    +y
//!         /         /|    |  +z
//!        / |       / |    | /
//!       *---------|  |    |/
//!       |  /- - - | -*    *-----+x
//!   <1> | /       | /
//!       |/        |/    <----- voxel at (x, y, z)
//!       #---------*
//!       ^   <1>
//!       |---- point at (x, y, z)
//! ```
//!
//! ##### `BlockVec`, `ChunkVec`, and `WorldVec`
//!
//! A chunk is a cubic section of `(CHUNK_EDGE, CHUNK_EDGE, CHUNK_EDGE)` blocks. Like entity
//! coordinates on a block, chunk coordinates, when converted to world block vectors, correspond to
//! the negative-most corner of the chunk. `BlockVec` measure block positions relative to that point.
//!
//! A valid `BlockVec` is comprised of components from 0 to `CHUNK_EDGE` upper exclusive.
//!
//! ## Block Faces
//!
//! Block faces are axis-aligned, and are enumerated by the `BlockFace` enum.

use std::f32::consts::{PI, TAU};

use num_traits::Signed;
use typed_glam::{
	ext::VecExt,
	glam::{self, DVec3, IVec2, IVec3, Mat4, Vec2, Vec3},
	traits::{NumericVector2, NumericVector3, SignedNumericVector3},
	typed::{FlavorCastFrom, TypedVector, VecFlavor},
};

use crucible_util::{
	lang::iter::VolumetricIter,
	mem::{
		array::{map_arr, zip_arr},
		c_enum::{c_enum, CEnum},
	},
};

// === Coordinate Systems === //

pub const CHUNK_EDGE: i32 = 16;
pub const CHUNK_LAYER: i32 = CHUNK_EDGE.pow(2);
pub const CHUNK_VOLUME: i32 = CHUNK_EDGE.pow(3);

// === `WorldVec` === //

pub type WorldVec = TypedVector<WorldVecFlavor>;

#[non_exhaustive]
pub struct WorldVecFlavor;

impl VecFlavor for WorldVecFlavor {
	type Backing = glam::IVec3;

	const DEBUG_NAME: &'static str = "WorldVec";
}

impl FlavorCastFrom<EntityVec> for WorldVecFlavor {
	fn cast_from(vec: EntityVec) -> TypedVector<Self> {
		vec.block_pos()
	}
}

impl FlavorCastFrom<glam::IVec3> for WorldVecFlavor {
	fn cast_from(v: glam::IVec3) -> WorldVec {
		WorldVec::from_glam(v)
	}
}

impl FlavorCastFrom<i32> for WorldVecFlavor {
	fn cast_from(v: i32) -> WorldVec {
		WorldVec::splat(v)
	}
}

pub trait WorldVecExt: Sized {
	fn compose(chunk: ChunkVec, block: BlockVec) -> Self;
	fn decompose(self) -> (ChunkVec, BlockVec);
	fn chunk(self) -> ChunkVec;
	fn block(self) -> BlockVec;
	fn negative_most_corner(self) -> EntityVec;
	fn block_interface_layer(self, face: BlockFace) -> f64;
}

impl WorldVecExt for WorldVec {
	fn compose(chunk: ChunkVec, block: BlockVec) -> Self {
		debug_assert!(chunk.is_valid());
		debug_assert!(block.is_valid());
		Self::from_glam(chunk.to_glam() * CHUNK_EDGE + block.to_glam())
	}

	fn decompose(self) -> (ChunkVec, BlockVec) {
		(self.chunk(), self.block())
	}

	fn chunk(self) -> ChunkVec {
		ChunkVec::new(
			self.x().div_euclid(CHUNK_EDGE),
			self.y().div_euclid(CHUNK_EDGE),
			self.z().div_euclid(CHUNK_EDGE),
		)
	}

	fn block(self) -> BlockVec {
		BlockVec::new(
			self.x().rem_euclid(CHUNK_EDGE),
			self.y().rem_euclid(CHUNK_EDGE),
			self.z().rem_euclid(CHUNK_EDGE),
		)
	}

	fn negative_most_corner(self) -> EntityVec {
		self.map_glam(|raw| raw.as_dvec3())
	}

	fn block_interface_layer(self, face: BlockFace) -> f64 {
		let corner = self.negative_most_corner();
		let (axis, sign) = face.decompose();

		if sign == Sign::Positive {
			corner.comp(axis) + 1.
		} else {
			corner.comp(axis)
		}
	}
}

// === `ChunkVec` === //

pub type ChunkVec = TypedVector<ChunkVecFlavor>;

#[non_exhaustive]
pub struct ChunkVecFlavor;

impl VecFlavor for ChunkVecFlavor {
	type Backing = glam::IVec3;

	const DEBUG_NAME: &'static str = "ChunkVec";
}

impl FlavorCastFrom<glam::IVec3> for ChunkVecFlavor {
	fn cast_from(v: glam::IVec3) -> ChunkVec {
		ChunkVec::from_glam(v)
	}
}

impl FlavorCastFrom<i32> for ChunkVecFlavor {
	fn cast_from(v: i32) -> ChunkVec {
		ChunkVec::splat(v)
	}
}

pub trait ChunkVecExt: Sized {
	fn is_valid(&self) -> bool;
}

impl ChunkVecExt for ChunkVec {
	fn is_valid(&self) -> bool {
		self.all(|comp| comp.checked_mul(CHUNK_EDGE).is_some())
	}
}

// === `BlockVec` === //

pub type BlockVec = TypedVector<BlockVecFlavor>;

#[non_exhaustive]
pub struct BlockVecFlavor;

impl VecFlavor for BlockVecFlavor {
	type Backing = glam::IVec3;

	const DEBUG_NAME: &'static str = "BlockVec";
}

impl FlavorCastFrom<glam::IVec3> for BlockVecFlavor {
	fn cast_from(v: glam::IVec3) -> BlockVec {
		BlockVec::from_glam(v)
	}
}

impl FlavorCastFrom<i32> for BlockVecFlavor {
	fn cast_from(v: i32) -> BlockVec {
		BlockVec::splat(v)
	}
}

pub trait BlockVecExt: Sized {
	fn is_valid(&self) -> bool;
	fn wrap(self) -> Self;
	fn iter() -> BlockPosIter;

	fn to_index(self) -> usize;
	fn try_from_index(index: usize) -> Option<Self>;
	fn from_index(index: usize) -> Self;
	fn is_valid_index(index: usize) -> bool;
}

impl BlockVecExt for BlockVec {
	fn is_valid(&self) -> bool {
		self.all(|comp| (0..CHUNK_EDGE).contains(&comp))
	}

	fn wrap(self) -> Self {
		self.map(|comp| comp.rem_euclid(CHUNK_EDGE))
	}

	fn iter() -> BlockPosIter {
		BlockPosIter(0)
	}

	fn to_index(self) -> usize {
		debug_assert!(self.is_valid());
		(self.x() + self.y() * CHUNK_EDGE + self.z() * CHUNK_LAYER) as usize
	}

	fn try_from_index(index: usize) -> Option<Self> {
		if Self::is_valid_index(index) {
			Some(Self::from_index(index))
		} else {
			None
		}
	}

	fn from_index(index: usize) -> Self {
		debug_assert!(Self::is_valid_index(index));

		let mut index = index as i32;
		let x = index % CHUNK_EDGE;
		index /= CHUNK_EDGE;
		let y = index % CHUNK_EDGE;
		index /= CHUNK_EDGE;
		let z = index % CHUNK_EDGE;

		Self::new(x, y, z)
	}

	fn is_valid_index(index: usize) -> bool {
		index < CHUNK_VOLUME as usize
	}
}

#[derive(Debug)]
pub struct BlockPosIter(usize);

impl Iterator for BlockPosIter {
	type Item = BlockVec;

	fn next(&mut self) -> Option<Self::Item> {
		let pos = BlockVec::try_from_index(self.0)?;
		self.0 += 1;
		Some(pos)
	}
}

// === `EntityVec` === //

/// A vector in the logical vector-space of valid entity positions. This is a double precision float
/// vector because we need all world positions to be encodable as entity positions.
pub type EntityVec = TypedVector<EntityVecFlavor>;

#[non_exhaustive]
pub struct EntityVecFlavor;

impl VecFlavor for EntityVecFlavor {
	type Backing = DVec3;

	const DEBUG_NAME: &'static str = "EntityVec";
}

impl FlavorCastFrom<glam::DVec3> for EntityVecFlavor {
	fn cast_from(v: glam::DVec3) -> EntityVec {
		EntityVec::from_glam(v)
	}
}

impl FlavorCastFrom<f64> for EntityVecFlavor {
	fn cast_from(v: f64) -> EntityVec {
		EntityVec::splat(v)
	}
}

impl FlavorCastFrom<WorldVec> for EntityVecFlavor {
	fn cast_from(v: WorldVec) -> EntityVec {
		v.negative_most_corner()
	}
}

pub trait EntityVecExt {
	const HORIZONTAL: Self;

	fn block_pos(self) -> WorldVec;
}

impl EntityVecExt for EntityVec {
	const HORIZONTAL: Self = Self::from_glam(DVec3::new(1.0, 0.0, 1.0));

	fn block_pos(self) -> WorldVec {
		self.map_glam(|raw| raw.floor().as_ivec3())
	}
}

// === Enums === //

c_enum! {
	pub enum BlockFace {
		PositiveX,
		NegativeX,
		PositiveY,
		NegativeY,
		PositiveZ,
		NegativeZ,
	}

	pub enum Axis2 {
		X,
		Y,
	}

	pub enum Axis3 {
		X,
		Y,
		Z,
	}

	pub enum Sign {
		Positive,
		Negative,
	}
}

// BlockFace
impl BlockFace {
	pub const TOP: Self = Self::PositiveY;

	pub const BOTTOM: Self = Self::NegativeY;

	pub const SIDES: [Self; 4] = [
		Self::PositiveX,
		Self::NegativeZ,
		Self::NegativeX,
		Self::PositiveZ,
	];

	pub fn from_vec(vec: IVec3) -> Option<Self> {
		let mut choice = None;

		for axis in Axis3::variants() {
			let comp = vec.comp(axis);
			if comp.abs() == 1 {
				if choice.is_some() {
					return None;
				}

				choice = Some(BlockFace::compose(axis, Sign::of(comp).unwrap()));
			}
		}

		choice
	}

	pub fn compose(axis: Axis3, sign: Sign) -> Self {
		use Axis3::*;
		use BlockFace::*;
		use Sign::*;

		match (axis, sign) {
			(X, Positive) => PositiveX,
			(X, Negative) => NegativeX,
			(Y, Positive) => PositiveY,
			(Y, Negative) => NegativeY,
			(Z, Positive) => PositiveZ,
			(Z, Negative) => NegativeZ,
		}
	}

	pub fn decompose(self) -> (Axis3, Sign) {
		(self.axis(), self.sign())
	}

	pub fn axis(self) -> Axis3 {
		use Axis3::*;
		use BlockFace::*;

		match self {
			PositiveX => X,
			NegativeX => X,
			PositiveY => Y,
			NegativeY => Y,
			PositiveZ => Z,
			NegativeZ => Z,
		}
	}

	pub fn sign(self) -> Sign {
		use BlockFace::*;
		use Sign::*;

		match self {
			PositiveX => Positive,
			NegativeX => Negative,
			PositiveY => Positive,
			NegativeY => Negative,
			PositiveZ => Positive,
			NegativeZ => Negative,
		}
	}

	pub fn invert(self) -> Self {
		Self::compose(self.axis(), self.sign().invert())
	}

	pub fn unit(self) -> IVec3 {
		self.unit_typed()
	}

	pub fn unit_typed<V>(self) -> V
	where
		V: SignedNumericVector3,
	{
		let v = self.axis().unit_typed::<V>();
		if self.sign() == Sign::Negative {
			-v
		} else {
			v
		}
	}
}

// Axis2
impl Axis2 {
	pub fn unit(self) -> IVec2 {
		self.unit_typed()
	}

	pub fn unit_typed<V: NumericVector2>(self) -> V {
		use Axis2::*;

		match self {
			X => V::X,
			Y => V::Y,
		}
	}
}

// Axis3
impl Axis3 {
	pub fn unit(self) -> IVec3 {
		self.unit_typed()
	}

	pub fn unit_typed<V: NumericVector3>(self) -> V {
		use Axis3::*;

		match self {
			X => V::X,
			Y => V::Y,
			Z => V::Z,
		}
	}

	pub fn ortho_hv(self) -> (Axis3, Axis3) {
		match self {
			// As a reminder, our coordinate system is y-up right-handed and looks like this:
			//
			//     +y
			//      |
			// +x---|
			//     /
			//   +z
			//
			Self::X => {
				// A quad facing the negative x direction looks like this:
				//
				//       c +y
				//      /|
				//     / |
				//    d  |
				//    |  b 0
				//    | /     ---> -x
				//    |/
				//    a +z
				//
				(Self::Z, Self::Y)
			}
			Self::Y => {
				// A quad facing the negative y direction looks like this:
				//
				//  +x        0
				//    d------a    |
				//   /      /     |
				//  /      /      ↓ -y
				// c------b
				//         +z
				(Self::X, Self::Z)
			}
			Self::Z => {
				// A quad facing the negative z direction looks like this:
				//
				//              +y
				//      c------d
				//      |      |     ^ -z
				//      |      |    /
				//      b------a   /
				//    +x        0
				//
				(Self::X, Self::Y)
			}
		}
	}

	pub fn extrude_volume_hv<V: NumericVector3>(
		self,
		size: impl Into<(V::Comp, V::Comp)>,
		perp: V::Comp,
	) -> V {
		let (ha, va) = self.ortho_hv();
		let (hm, vm) = size.into();

		let mut target = V::ZERO;
		*target.comp_mut(ha) = hm;
		*target.comp_mut(va) = vm;
		*target.comp_mut(self) = perp;

		target
	}

	pub fn plane_intersect(self, layer: f64, line: Line3) -> (f64, EntityVec) {
		let lerp = lerp_percent_at(layer, line.start.comp(self), line.end.comp(self));
		(lerp, line.start.lerp(line.end, lerp))
	}
}

// Sign
impl Sign {
	pub fn of<T: Signed>(val: T) -> Option<Self> {
		if val.is_positive() {
			Some(Self::Positive)
		} else if val.is_negative() {
			Some(Self::Negative)
		} else {
			None
		}
	}

	pub fn invert(self) -> Self {
		use Sign::*;

		match self {
			Positive => Negative,
			Negative => Positive,
		}
	}

	pub fn unit<T: Signed>(self) -> T {
		use Sign::*;

		match self {
			Positive => T::one(),
			Negative => -T::one(),
		}
	}
}

// === Line3 === //

#[derive(Debug, Copy, Clone)]
pub struct Line3 {
	pub start: EntityVec,
	pub end: EntityVec,
}

impl Line3 {
	pub fn new(start: EntityVec, end: EntityVec) -> Self {
		Self { start, end }
	}

	pub fn new_origin_delta(start: EntityVec, delta: EntityVec) -> Self {
		Self {
			start,
			end: start + delta,
		}
	}
}

// === Misc Math === //

pub fn lerp_percent_at(val: f64, start: f64, end: f64) -> f64 {
	// start + (end - start) * percent = val
	// (val - start) / (end - start) = percent
	(val - start) / (end - start)
}

// === Vector extensions === //

pub trait Vec3Ext: NumericVector3 {
	fn comp(&self, axis: Axis3) -> Self::Comp;

	fn comp_mut(&mut self, axis: Axis3) -> &mut Self::Comp;
}

impl<V: NumericVector3> Vec3Ext for V {
	fn comp(&self, axis: Axis3) -> Self::Comp {
		self[axis.index()]
	}

	fn comp_mut(&mut self, axis: Axis3) -> &mut Self::Comp {
		&mut self[axis.index()]
	}
}

// === Angle3D === //

pub const HALF_PI: f32 = PI / 2.0;

pub type Angle3D = TypedVector<Angle3DFlavor>;

pub struct Angle3DFlavor;

impl VecFlavor for Angle3DFlavor {
	type Backing = Vec2;

	const DEBUG_NAME: &'static str = "Angle3D";
}

impl FlavorCastFrom<Vec2> for Angle3DFlavor {
	fn cast_from(vec: Vec2) -> TypedVector<Self>
	where
		Self: VecFlavor,
	{
		TypedVector::from_glam(vec)
	}
}

pub trait Angle3DExt {
	fn new_deg(yaw: f32, pitch: f32) -> Self;

	fn as_matrix(&self) -> Mat4;

	fn as_matrix_horizontal(&self) -> Mat4;

	fn as_matrix_vertical(&self) -> Mat4;

	fn forward(&self) -> Vec3;

	fn wrap(&self) -> Self;

	fn wrap_x(&self) -> Self;

	fn wrap_y(&self) -> Self;

	fn clamp_y(&self, min: f32, max: f32) -> Self;

	fn clamp_y_90(&self) -> Self;
}

impl Angle3DExt for Angle3D {
	fn new_deg(yaw: f32, pitch: f32) -> Self {
		Self::new(yaw.to_radians(), pitch.to_radians())
	}

	fn as_matrix(&self) -> Mat4 {
		self.as_matrix_horizontal() * self.as_matrix_vertical()
	}

	fn as_matrix_horizontal(&self) -> Mat4 {
		Mat4::from_rotation_y(self.x())
	}

	fn as_matrix_vertical(&self) -> Mat4 {
		Mat4::from_rotation_x(self.y())
	}

	fn forward(&self) -> Vec3 {
		self.as_matrix().transform_vector3(Vec3::Z)
	}

	fn wrap(&self) -> Self {
		self.wrap_x().wrap_y()
	}

	fn wrap_x(&self) -> Self {
		Self::new(self.x().rem_euclid(TAU), self.y())
	}

	fn wrap_y(&self) -> Self {
		Self::new(self.x(), self.y().rem_euclid(TAU))
	}

	fn clamp_y(&self, min: f32, max: f32) -> Self {
		Self::new(self.x(), self.y().clamp(min, max))
	}

	fn clamp_y_90(&self) -> Self {
		self.clamp_y(-HALF_PI, HALF_PI)
	}
}

// === Quad === //

#[derive(Debug, Copy, Clone)]
pub struct AaQuad<V: NumericVector3> {
	pub origin: V,
	pub face: BlockFace,
	pub size: (V::Comp, V::Comp),
}

impl<V: NumericVector3> AaQuad<V> {
	pub fn new_given_volume(origin: V, face: BlockFace, volume: V) -> Self {
		let (h, v) = face.axis().ortho_hv();
		let size = (volume.comp(h), volume.comp(v));

		Self { origin, face, size }
	}

	pub fn new_unit(origin: V, face: BlockFace) -> Self {
		let one = V::ONE.x();

		Self {
			origin,
			face,
			size: (one, one),
		}
	}

	pub fn size_deltas(&self) -> (V, V) {
		let (h, v) = self.face.axis().ortho_hv();
		let (sh, sv) = self.size;
		(
			h.unit_typed::<V>() * V::splat(sh),
			v.unit_typed::<V>() * V::splat(sv),
		)
	}

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

	pub fn extrude_hv(self, delta: V::Comp) -> Aabb3<V>
	where
		V: SignedNumericVector3,
		V::Comp: Signed,
	{
		Aabb3 {
			origin: if self.face.sign() == Sign::Negative {
				self.origin - self.face.axis().unit_typed::<V>() * V::splat(delta)
			} else {
				self.origin
			},
			size: self.face.axis().extrude_volume_hv(self.size, delta),
		}
	}
}

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

	pub fn to_tris(self) -> [[V; 3]; 2]
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
		[[a, b, c], [a, c, d]]
	}

	pub fn zip<R>(self, rhs: Quad<R>) -> Quad<(V, R)> {
		Quad(zip_arr(self.0, rhs.0))
	}

	pub fn map<R>(self, f: impl FnMut(V) -> R) -> Quad<R> {
		Quad(map_arr(self.0, f))
	}
}

// === Aabb3 === //

pub type EntityAabb = Aabb3<EntityVec>;
pub type WorldAabb = Aabb3<WorldVec>;

#[derive(Debug, Copy, Clone)]
pub struct Aabb3<V> {
	pub origin: V,
	pub size: V,
}

impl<V: SignedNumericVector3> Aabb3<V> {
	pub fn translated(&self, by: V) -> Self {
		Self {
			origin: self.origin + by,
			size: self.size,
		}
	}

	pub fn positive_corner(&self) -> V {
		self.origin + self.size
	}

	pub fn grow(self, by: V) -> Self {
		Self {
			origin: self.origin - by,
			size: self.size + by + by,
		}
	}

	pub fn quad(self, face: BlockFace) -> AaQuad<V> {
		let origin = self.origin;
		let origin = if face.sign() == Sign::Positive {
			origin + face.unit_typed::<V>() * V::splat(self.size.comp(face.axis()))
		} else {
			origin
		};

		AaQuad::new_given_volume(origin, face, self.size)
	}
}

impl EntityAabb {
	pub fn as_blocks(&self) -> WorldAabb {
		Aabb3::from_blocks_corners(self.origin.block_pos(), self.positive_corner().block_pos())
	}
}

impl WorldAabb {
	pub fn from_blocks_corners(a: WorldVec, b: WorldVec) -> Self {
		let origin = a.min(b);
		let size = (b - a).abs() + WorldVec::ONE;

		Self { origin, size }
	}

	pub fn iter_blocks(self) -> impl Iterator<Item = WorldVec> {
		VolumetricIter::new_exclusive_iter([
			self.size.x() as u32,
			self.size.y() as u32,
			self.size.z() as u32,
		])
		.map(move |[x, y, z]| self.origin + WorldVec::new(x as i32, y as i32, z as i32))
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
