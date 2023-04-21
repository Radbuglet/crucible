use std::{
	any::{type_name, TypeId},
	mem::{self, ManuallyDrop},
};

// === Transmute === //

pub const unsafe fn entirely_unchecked_transmute<A, B>(a: A) -> B {
	union Punny<A, B> {
		a: ManuallyDrop<A>,
		b: ManuallyDrop<B>,
	}

	let punned = Punny {
		a: ManuallyDrop::new(a),
	};

	ManuallyDrop::into_inner(punned.b)
}

pub const unsafe fn sizealign_checked_transmute<A, B>(a: A) -> B {
	assert!(mem::size_of::<A>() == mem::size_of::<B>());
	assert!(mem::align_of::<A>() >= mem::align_of::<B>());

	entirely_unchecked_transmute(a)
}

// === Runtime type unification === //

pub fn try_unify<A: 'static, B: 'static>(a: A) -> Option<B> {
	if TypeId::of::<A>() == TypeId::of::<B>() {
		Some(unsafe { sizealign_checked_transmute(a) })
	} else {
		None
	}
}

pub fn try_unify_ref<A: ?Sized + 'static, B: ?Sized + 'static>(a: &A) -> Option<&B> {
	if TypeId::of::<A>() == TypeId::of::<B>() {
		Some(unsafe { sizealign_checked_transmute(a) })
	} else {
		None
	}
}

pub fn try_unify_mut<A, B>(a: &mut A) -> Option<&mut B>
where
	A: ?Sized + 'static,
	B: ?Sized + 'static,
{
	if TypeId::of::<A>() == TypeId::of::<B>() {
		Some(unsafe { sizealign_checked_transmute(a) })
	} else {
		None
	}
}

fn unify_err<A, B>() -> ! {
	panic!(
		"{} cannot be unified with {}",
		type_name::<A>(),
		type_name::<B>()
	)
}

pub fn unify<A: 'static, B: 'static>(a: A) -> B {
	try_unify(a).unwrap_or_else(|| unify_err::<A, B>())
}

pub fn unify_ref<A: ?Sized + 'static, B: ?Sized + 'static>(a: &A) -> &B {
	try_unify_ref(a).unwrap_or_else(|| unify_err::<&A, &B>())
}

pub fn unify_mut<A: ?Sized + 'static, B: ?Sized + 'static>(a: &mut A) -> &mut B {
	try_unify_mut(a).unwrap_or_else(|| unify_err::<&mut A, &mut B>())
}
