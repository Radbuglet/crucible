use crate::util::{FmtIntoOwnedExt, FmtIterExt};
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

	pub fn unit_zero(self) -> &'static str {
		match self {
			CompType::U32 => "0",
			CompType::I32 => "0",
			CompType::F32 => "0.0",
			CompType::F64 => "0.0",
		}
	}

	pub fn unit_one(self) -> &'static str {
		match self {
			CompType::U32 => "1",
			CompType::I32 => "1",
			CompType::F32 => "1.0",
			CompType::F64 => "1.0",
		}
	}

	pub fn unit_neg_one(self) -> &'static str {
		match self {
			CompType::U32 => "-1",
			CompType::I32 => "-1",
			CompType::F32 => "-1.0",
			CompType::F64 => "-1.0",
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
	typed_vector_impl: &'a rust::Import,
	as_mut: &'a rust::Import,
	as_ref: &'a rust::Import,
	from: &'a rust::Import,
	index: &'a rust::Import,
	index_mut: &'a rust::Import,

	// Reused items
	self_owned: &'a rust::Tokens,
	self_ref: &'a rust::Tokens,
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
		typed_vector_impl: &rust::import("crate", "TypedVectorImpl").direct(),
		as_mut: &rust::import("core::convert", "AsMut"),
		as_ref: &rust::import("core::convert", "AsRef"),
		from: &rust::import("core::convert", "From"),
		index: &rust::import("core::ops", "Index"),
		index_mut: &rust::import("core::ops", "IndexMut"),

		// Reused items
		self_owned: &quote! { Self },
		self_ref: &quote! { &Self },
	};

	quote! {
		$(derive_method_forwards(&sess))
		$(derive_misc_traits(&sess))

		// TODO: `Shl` and `Shr`
		// TODO: Ensure that the formatting pre-rust-fmt isn't too bad.

		$(derive_op_forwards(&sess))
	}
}

fn derive_method_forwards(sess: &VecDeriveSession) -> rust::Tokens {
	// Hoisted
	let backing = sess.backing;
	let vec_flavor = sess.vec_flavor;
	let typed_vector_impl = sess.typed_vector_impl;
	let comp_ty = sess.comp_type.prim_ty();
	let unit_zero = sess.comp_type.unit_zero();
	let unit_one = sess.comp_type.unit_one();
	let unit_neg_one = sess.comp_type.unit_neg_one();
	let dim = sess.dim;

	// Method forwarding generation
	let mut forwarded_methods = Tokens::new();

	derive_method_forward_stub(
		sess,
		"new",
		true,
		SelfTy::Static,
		&[
			// FIXME
			("x", ForwardedType::Exact(&comp_ty.fmt_to_tokens())),
			("y", ForwardedType::Exact(&comp_ty.fmt_to_tokens())),
			("z", ForwardedType::Exact(&comp_ty.fmt_to_tokens())),
		],
		&[ForwardedType::OwnedVector],
	)
	.format_into(&mut forwarded_methods);

	derive_method_forward_stub(
		sess,
		"splat",
		true,
		SelfTy::Static,
		&[("v", ForwardedType::Exact(&comp_ty.fmt_to_tokens()))],
		&[ForwardedType::OwnedVector],
	)
	.format_into(&mut forwarded_methods);

	// Generation
	quote! {
		$("// === Inherent `impl` items === //")

		impl<M> $typed_vector_impl<$backing, M>
		where
			M: ?Sized + $vec_flavor<Backing = $backing>,
		{
			pub const ZERO: Self = Self::splat($unit_zero);
			pub const ONE: Self = Self::splat($unit_one);

			$forwarded_methods
		}
		$['\n']
	}
}

#[derive(Debug, Copy, Clone)]
enum ForwardedType<'a> {
	Exact(&'a rust::Tokens),
	OwnedVector,
	VectorRef,
}

impl<'a> ForwardedType<'a> {
	fn as_out_ty<'b>(self, sess: &VecDeriveSession<'b>) -> &'b rust::Tokens
	where
		'a: 'b,
	{
		match self {
			ForwardedType::Exact(exact) => exact,
			ForwardedType::OwnedVector => sess.self_owned,
			ForwardedType::VectorRef => sess.self_ref,
		}
	}

	fn as_transformer_for(self, expr: rust::Tokens) -> rust::Tokens {
		match self {
			ForwardedType::Exact(_) => expr, // (no transformation necessary)
			ForwardedType::OwnedVector => quote! { Self::from_raw($expr) },
			ForwardedType::VectorRef => quote! { Self::from_raw_ref($expr) },
		}
	}
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
enum SelfTy {
	Ref,
	Mut,
	Owned,
	Static,
}

impl SelfTy {
	fn prefix(self) -> Option<&'static str> {
		match self {
			SelfTy::Ref => Some("&self"),
			SelfTy::Mut => Some("&mut self"),
			SelfTy::Owned => Some("self"),
			SelfTy::Static => None,
		}
	}
}

// FIXME: Forgot to do input transformations
fn derive_method_forward_stub(
	sess: &VecDeriveSession,
	name: &str,
	is_const: bool,
	self_ty: SelfTy,
	args: &[(&str, ForwardedType)],
	ret_vals: &[ForwardedType],
) -> rust::Tokens {
	// Hoisting
	let backing = sess.backing;

	// Generate signature elements
	let args_fmt = self_ty
		.prefix()
		.map(|elem| elem.fmt_to_tokens())
		.into_iter()
		.chain(args.iter().map(|(arg_name, arg_ty)| {
			quote! { $(*arg_name): $(arg_ty.as_out_ty(sess)) }
		}))
		.fmt_delimited(",");

	let ret_vals_fmt = if ret_vals.len() == 1 {
		ret_vals[0].as_out_ty(sess).clone()
	} else {
		ret_vals
			.iter()
			.map(|ty| ty.as_out_ty(sess))
			.fmt_delimited(",")
			.fmt_to_tokens()
	};

	// Generate body elements
	let fwd_prefix = match self_ty {
		// I guess ease trumps good code gen :person_shrugging:.
		SelfTy::Ref | SelfTy::Mut | SelfTy::Owned => quote! { self.vec. },
		SelfTy::Static => quote! { $backing:: },
	};

	let fwd_args = args
		.iter()
		.map(|(arg_name, _)| arg_name.fmt_to_tokens())
		.fmt_delimited(",");

	let fwd_call = quote! { $fwd_prefix$name($fwd_args) };

	let fwd_body = if ret_vals.len() == 1 {
		ret_vals[0].as_transformer_for(fwd_call)
	} else {
		const ALPHABET: &'static str = "abcdefghijklmnopqrstuvwxyz";
		assert!(ret_vals.len() < ALPHABET.len());

		let fwd_tup_args = ALPHABET[0..ret_vals.len()]
			.split("")
			.fmt_delimited(",")
			.fmt_to_tokens();

		let fwd_tup_transform = ret_vals
			.iter()
			.zip(ALPHABET.split(""))
			.map(|(ret_ty, name)| ret_ty.as_transformer_for(name.fmt_to_tokens()))
			.fmt_delimited(",");

		quote! {
			let ($fwd_tup_args) = $fwd_call;
			($fwd_tup_transform)
		}
	};

	// Generate
	quote! {
		pub $(if is_const => const) fn $name($args_fmt) -> $ret_vals_fmt {
			$fwd_body
		}
	}
}

fn derive_misc_traits(sess: &VecDeriveSession) -> rust::Tokens {
	// Hoisted
	// TODO: Clean up hoisting logic
	let backing = sess.backing;
	let vec_flavor = sess.vec_flavor;
	let typed_vector_impl = sess.typed_vector_impl;
	let backing_vec_trait = sess.backing_vec_trait;
	let backing_vec_sealed = sess.backing_vec_sealed;
	let as_mut = sess.as_mut;
	let as_ref = sess.as_ref;
	let from = sess.from;
	let index = sess.index;
	let index_mut = sess.index_mut;
	let comp_ty = sess.comp_type.prim_ty();
	let dim = sess.dim;

	// Generation
	quote! {
		$("// === Misc trait derivations === //")
		$("// (most other traits are derived via trait logic in `lib.rs`)")

		impl $backing_vec_trait for $backing {}
		impl $backing_vec_sealed for $backing {}

		$("// Raw <-> Typed")
		impl<M> $from<$backing> for $typed_vector_impl<$backing, M>
		where
			M: ?Sized + $vec_flavor<Backing = $backing>,
		{
			fn from(v: $backing) -> Self {
				Self::from_raw(v)
			}
		}

		impl<M> $from<$typed_vector_impl<$backing, M>> for $backing
		where
			M: ?Sized + $vec_flavor<Backing = $backing>,
		{
			fn from(v: $typed_vector_impl<$backing, M>) -> Self {
				v.into_raw()
			}
		}

		$(format_args!("// [{comp_ty}; {dim}] <-> Typed"))
		impl<M> $from<[$comp_ty; $dim]> for $typed_vector_impl<$backing, M>
		where
			M: ?Sized + $vec_flavor<Backing = $backing>,
		{
			fn from(v: [$comp_ty; $dim]) -> Self {
				$backing::from(v).into()
			}
		}

		impl<M> $from<$typed_vector_impl<$backing, M>> for [$comp_ty; $dim]
		where
			M: ?Sized + $vec_flavor<Backing = $backing>,
		{
			fn from(v: $typed_vector_impl<$backing, M>) -> Self {
				v.into_raw().into()
			}
		}

		$(format_args!("// ({comp_ty}, ..., {comp_ty}) <-> Typed"))
		impl<M> $from<($(for _ in 0..dim => $comp_ty,))> for $typed_vector_impl<$backing, M>
		where
			M: ?Sized + $vec_flavor<Backing = $backing>,
		{
			fn from(v: ($(for _ in 0..dim => $comp_ty,))) -> Self {
				$backing::from(v).into()
			}
		}

		impl<M> $from<$typed_vector_impl<$backing, M>> for ($(for _ in 0..dim => $comp_ty,))
		where
			M: ?Sized + $vec_flavor<Backing = $backing>,
		{
			fn from(v: $typed_vector_impl<$backing, M>) -> Self {
				v.into_raw().into()
			}
		}

		$("// `AsRef` and `AsMut`")
		impl<M> $as_ref<[$comp_ty; $dim]> for $typed_vector_impl<$backing, M>
		where
			M: ?Sized + $vec_flavor<Backing = $backing>,
		{
			fn as_ref(&self) -> &[$comp_ty; $dim] {
				self.raw().as_ref()
			}
		}

		impl<M> $as_mut<[$comp_ty; $dim]> for $typed_vector_impl<$backing, M>
		where
			M: ?Sized + $vec_flavor<Backing = $backing>,
		{
			fn as_mut(&mut self) -> &mut [$comp_ty; $dim] {
				self.raw_mut().as_mut()
			}
		}

		$("// `Index` and `IndexMut`")
		impl<M> $index<usize> for $typed_vector_impl<$backing, M>
		where
			M: ?Sized + $vec_flavor<Backing = $backing>,
		{
			type Output = $comp_ty;

			fn index(&self, i: usize) -> &$comp_ty {
				&self.raw()[i]
			}
		}

		impl<M> $index_mut<usize> for $typed_vector_impl<$backing, M>
		where
			M: ?Sized + $vec_flavor<Backing = $backing>,
		{
			fn index_mut(&mut self, i: usize) -> &mut $comp_ty {
				&mut self.raw_mut()[i]
			}
		}
	}
}

fn derive_op_forwards(sess: &VecDeriveSession) -> rust::Tokens {
	// Hoisted
	let backing = sess.backing;
	let vec_flavor = sess.vec_flavor;
	let typed_vector_impl = sess.typed_vector_impl;

	// Generation
	let mut bin_traits = quote! {
		$("// === `core::ops` trait forwards === //\n\n")
	};

	derive_bin_op_forward(sess, "Add", false).format_into(&mut bin_traits);
	derive_bin_op_forward(sess, "Sub", false).format_into(&mut bin_traits);
	derive_bin_op_forward(sess, "Mul", false).format_into(&mut bin_traits);
	derive_bin_op_forward(sess, "Div", false).format_into(&mut bin_traits);
	derive_bin_op_forward(sess, "Rem", false).format_into(&mut bin_traits);

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
	}
}
