// Nothing from `typed-glam` needs to be imported here. These behave as if they truly were their own
// types.
pub use decl::{ChunkPos, WorldPos, WorldPosExt};

mod decl {
	use typed_glam::{FlavorCastFrom, TypedVector, VecFlavor};

	// `TypedVector` takes a dummy struct implementing `VecFlavor` (also called a "flavor") and produces
	// a strongly typed wrapper around the flavor's `Backing` vector. Newtyping a vector takes 7 lines
	// instead of just under 700.
	pub type WorldPos = TypedVector<WorldPosFlavor>;

	pub struct WorldPosFlavor;

	impl VecFlavor for WorldPosFlavor {
		type Backing = glam::Vec3;
	}

	pub type ChunkPos = TypedVector<ChunkPosFlavor>;

	pub struct ChunkPosFlavor;

	impl VecFlavor for ChunkPosFlavor {
		type Backing = glam::Vec3;
	}

	// Users can still implement extension methods on their vector "newtypes."
	pub trait WorldPosExt {
		fn do_something(&self);
	}

	impl WorldPosExt for WorldPos {
		fn do_something(&self) {
			println!("Something! {self}");
		}
	}

	// Users can define custom casts from one newtype vector flavor to another. This works because
	// we're defining the conversion on the flavor rather than on the vector directly.
	impl FlavorCastFrom<ChunkPosFlavor> for WorldPosFlavor {
		fn vec_from(vec: ChunkPos) -> WorldPos {
			(vec * 16.).raw_cast()
		}
	}
}

fn main() {
	// The interface of newtyped vectors and regular `glam` vectors are almost identical. The only
	// exceptions to this parity are:
	// - Swizzling is disabled because it doesn't really make sense in this context.
	// - Users cannot use the bare struct constructor because we need to introduce a `PhantomData`.
	//   This is also the reason for which we introduced a newtype wrapper instead of just adding the
	//   flavor parameter directly to the vector types.
	let mut world_pos = WorldPos::new(1., 1., 1.);
	world_pos += 0.4 * (WorldPos::X + WorldPos::NEG_Y).normalize();

	let mut chunk_pos = ChunkPos::new(3., 2., 4.);

	// Raw vectors can always be used with newtyped vectors.
	chunk_pos *= 3. * glam::Vec3::X;
	chunk_pos -= ChunkPos::new(1., 3., 4.);

	// Oh look, contextual extension methods.
	world_pos.do_something();

	// Users can `cast` flavors directly if they need to.
	chunk_pos += world_pos.raw_cast();

	// But they really should be using the safe casts provided by the flavor.
	chunk_pos.cast::<WorldPos>().do_something();
}
