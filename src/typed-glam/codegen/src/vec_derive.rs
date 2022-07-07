use genco::prelude::*;

// === Entry === //

pub fn derive_entry_all() -> rust::Tokens {
	derive_entry_one(&rust::import("glam::i32", "IVec3"), CompType::I32, 3)
}

// === Session types === //

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
enum CompType {
	U32,
	I32,
	F32,
	F64,
}

impl CompType {
	fn prim_ty(self) -> &'static str {
		match self {
			CompType::U32 => "u32",
			CompType::I32 => "i32",
			CompType::F32 => "f32",
			CompType::F64 => "f64",
		}
	}

	fn is_signed(self) -> bool {
		match self {
			CompType::U32 => false,
			CompType::I32 => true,
			CompType::F32 => true,
			CompType::F64 => true,
		}
	}

	fn is_twos_compliment(self) -> bool {
		match self {
			CompType::U32 => true,
			CompType::I32 => true,
			CompType::F32 => false,
			CompType::F64 => false,
		}
	}
}

struct VecDeriveSession<'a> {
	// Config parameters
	backing: &'a rust::Import,
	comp_type: CompType,
	dim: usize,

	// Imports
	backing_vec_trait: &'a rust::Import,
	backing_vec_sealed: &'a rust::Import,
	vec_flavor: &'a rust::Import,
	typed_vector: &'a rust::Import,
	typed_vector_impl: &'a rust::Import,
}

// === Main derivation logic === //

fn derive_entry_one(backing: &rust::Import, comp_type: CompType, dim: usize) -> rust::Tokens {
	let sess = VecDeriveSession {
		// Config parameters
		backing,
		comp_type,
		dim,

		// Imports
		backing_vec_trait: &rust::import("crate", "BackingVec"),
		backing_vec_sealed: &rust::import("crate::backing_vec", "Sealed"),
		vec_flavor: &rust::import("crate", "VecFlavor"),
		typed_vector: &rust::import("crate", "TypedVector").direct(),
		typed_vector_impl: &rust::import("crate", "TypedVectorImpl").direct(),
	};

	let backing_vec_impl = derive_backing_vec_marker(&sess);
	let op_forwards = derive_op_forwards(&sess);

	quote! {
		$backing_vec_impl

		$op_forwards
	}
}

fn derive_backing_vec_marker(sess: &VecDeriveSession) -> rust::Tokens {
	let backing = sess.backing;
	let backing_vec_trait = sess.backing_vec_trait;
	let backing_vec_sealed = sess.backing_vec_sealed;

	quote! {
		impl $backing_vec_trait for $backing {}
		impl $backing_vec_sealed for $backing {}
	}
}

fn derive_op_forwards(sess: &VecDeriveSession) -> rust::Tokens {
	// Hoisted
	let backing = sess.backing;
	let vec_flavor = sess.vec_flavor;
	let typed_vector_impl = sess.typed_vector_impl;

	// Generation
	let mut bin_traits = rust::Tokens::new();

	derive_bin_op_forward(sess, "Add", false).format_into(&mut bin_traits);
	derive_bin_op_forward(sess, "Sub", false).format_into(&mut bin_traits);
	derive_bin_op_forward(sess, "Mul", false).format_into(&mut bin_traits);
	derive_bin_op_forward(sess, "Div", false).format_into(&mut bin_traits);

	if sess.comp_type.is_twos_compliment() {
		derive_bin_op_forward(sess, "BitAnd", true).format_into(&mut bin_traits);
		derive_bin_op_forward(sess, "BitOr", true).format_into(&mut bin_traits);
		derive_bin_op_forward(sess, "BitXor", true).format_into(&mut bin_traits);

		let op_not = &rust::import("core::ops", "Not");

		quote! {
			impl<M> $op_not for $typed_vector_impl<$backing, M>
			where
				M: ?Sized + $vec_flavor<Backing = $backing>,
			{
				type Output = Self;

				fn not(self) -> Self {
					self.map_raw(|v| !v)
				}
			}
		}
		.format_into(&mut bin_traits);
	}

	if sess.comp_type.is_signed() {
		let op_neg = &rust::import("core::ops", "Neg");

		quote! {
			impl<M> $op_neg for $typed_vector_impl<$backing, M>
			where
				M: ?Sized + $vec_flavor<Backing = $backing>,
			{
				type Output = Self;

				fn neg(self) -> Self {
					self.map_raw(|v| -v)
				}
			}
		}
		.format_into(&mut bin_traits);
	}

	bin_traits
}

fn derive_bin_op_forward(
	sess: &VecDeriveSession,
	trait_name: &str,
	is_bit_op: bool,
) -> rust::Tokens {
	// Imports
	let vec_flavor = sess.vec_flavor;
	let typed_vector_impl = sess.typed_vector_impl;

	// Hoists
	let backing = sess.backing;
	let scalar = sess.comp_type.prim_ty();

	// Trait name derivations
	let op_trait = &rust::import("core::ops", trait_name);
	let op_trait_assign = &rust::import("core::ops", format!("{trait_name}Assign"));

	let fn_name = &trait_name.to_lowercase();
	let fn_name_assign = &format!("{fn_name}_assign");

	// Quasiquoting
	quote! {
		$(format_args!("// `{trait_name}` operation forwarding"))
		// vec + vec
		impl<M> $op_trait for $typed_vector_impl<$backing, M>
		where
			M: ?Sized + $vec_flavor<Backing = $backing>,
		{
			type Output = Self;

			fn $fn_name(self, rhs: Self) -> Self {
				self.map_raw(|lhs| $op_trait::$fn_name(lhs, rhs.into_raw()))
			}
		}

		// vec + raw vec
		impl<M> $op_trait<$backing> for $typed_vector_impl<$backing, M>
		where
			M: ?Sized + $vec_flavor<Backing = $backing>,
		{
			type Output = Self;

			fn $fn_name(self, rhs: $backing) -> Self {
				self.map_raw(|lhs| $op_trait::$fn_name(lhs, rhs))
			}
		}

		// vec + scalar
		impl<M> $op_trait<$scalar> for $typed_vector_impl<$backing, M>
		where
			M: ?Sized + $vec_flavor<Backing = $backing>,
		{
			type Output = Self;

			fn $fn_name(self, rhs: $scalar) -> Self {
				self.map_raw(|lhs| $op_trait::$fn_name(lhs, rhs))
			}
		}

		$(if !is_bit_op =>
			// scalar + vec
			impl<M> $op_trait<$typed_vector_impl<$backing, M>> for $scalar
			where
				M: ?Sized + $vec_flavor<Backing = $backing>,
			{
				type Output = $typed_vector_impl<$backing, M>;

				fn $fn_name(self, rhs: $typed_vector_impl<$backing, M>) -> $typed_vector_impl<$backing, M> {
					rhs.map_raw(|rhs| $op_trait::$fn_name(self, rhs))
				}
			}

			// vec += vec
			impl<M> $op_trait_assign for $typed_vector_impl<$backing, M>
			where
				M: ?Sized + $vec_flavor<Backing = $backing>,
			{
				fn $fn_name_assign(&mut self, rhs: Self) {
					$op_trait_assign::$fn_name_assign(self.raw_mut(), rhs.into_raw())
				}
			}

			// vec += raw vec
			impl<M> $op_trait_assign<$backing> for $typed_vector_impl<$backing, M>
			where
				M: ?Sized + $vec_flavor<Backing = $backing>,
			{
				fn $fn_name_assign(&mut self, rhs: $backing) {
					$op_trait_assign::$fn_name_assign(self.raw_mut(), rhs)
				}
			}
		)

		$['\n']

		// TODO: scalar stuff
	}
}
