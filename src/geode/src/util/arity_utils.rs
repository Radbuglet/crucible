// === Tuple generation === //

pub macro impl_tuples {
	($impl_macro:path) => {
		$impl_macro!();
		impl_tuples!(no_unit; $impl_macro);
	},
	(no_unit; $impl_macro:path) => {
		$impl_macro!(A: 0);
		$impl_macro!(A: 0, B: 1);
		$impl_macro!(A: 0, B: 1, C: 2);
		$impl_macro!(A: 0, B: 1, C: 2, D: 3);
		$impl_macro!(A: 0, B: 1, C: 2, D: 3, E: 4);
		$impl_macro!(A: 0, B: 1, C: 2, D: 3, E: 4, F: 5);
		$impl_macro!(A: 0, B: 1, C: 2, D: 3, E: 4, F: 5, G: 6);
		$impl_macro!(A: 0, B: 1, C: 2, D: 3, E: 4, F: 5, G: 6, H: 7);
		$impl_macro!(A: 0, B: 1, C: 2, D: 3, E: 4, F: 5, G: 6, H: 7, I: 8);
		$impl_macro!(A: 0, B: 1, C: 2, D: 3, E: 4, F: 5, G: 6, H: 7, I: 8, J: 9);
		$impl_macro!(A: 0, B: 1, C: 2, D: 3, E: 4, F: 5, G: 6, H: 7, I: 8, J: 9, K: 10);
		$impl_macro!(A: 0, B: 1, C: 2, D: 3, E: 4, F: 5, G: 6, H: 7, I: 8, J: 9, K: 10, L: 11);
		$impl_macro!(A: 0, B: 1, C: 2, D: 3, E: 4, F: 5, G: 6, H: 7, I: 8, J: 9, K: 10, L: 11, M: 12);
	}
}

// === Closure injection === //

pub trait InjectableClosure<A, D> {
	type Return;

	fn call_injected(&mut self, args: A, deps: D) -> Self::Return;
}

macro tup_impl_closure_call_derive_for_fn(
	$($left_name:ident: $left_field: tt),* ~
	$($right_name:ident: $right_field: tt),*
) {
	impl<
		ZZClosure: FnMut($($left_name,)* $($right_name,)*) -> ZZRet,
		ZZRet,
		$($left_name,)*
		$($right_name),*
	> InjectableClosure<
		($($left_name,)*),
		($($right_name,)*)
	>
	for ZZClosure
	{
		type Return = ZZRet;

		#[allow(unused_variables)]
		fn call_injected(&mut self, args: ($($left_name,)*), deps: ($($right_name,)*)) -> Self::Return {
			(self)(
				$(args.$left_field,)*
				$(deps.$right_field,)*
			)
		}
	}
}

macro tup_impl_closure_call($($name:ident: $field:tt),*) {
	tup_impl_closure_call_derive_for_fn!($($name: $field),* ~);
	tup_impl_closure_call_derive_for_fn!($($name: $field),* ~ N: 0);
	tup_impl_closure_call_derive_for_fn!($($name: $field),* ~ N: 0, O: 1);
	tup_impl_closure_call_derive_for_fn!($($name: $field),* ~ N: 0, O: 1, P: 2);
	tup_impl_closure_call_derive_for_fn!($($name: $field),* ~ N: 0, O: 1, P: 2, Q: 3);
	tup_impl_closure_call_derive_for_fn!($($name: $field),* ~ N: 0, O: 1, P: 2, Q: 3, R: 4);
	tup_impl_closure_call_derive_for_fn!($($name: $field),* ~ N: 0, O: 1, P: 2, Q: 3, R: 4, S: 5);
	tup_impl_closure_call_derive_for_fn!($($name: $field),* ~ N: 0, O: 1, P: 2, Q: 3, R: 4, S: 5, T: 6);
	tup_impl_closure_call_derive_for_fn!($($name: $field),* ~ N: 0, O: 1, P: 2, Q: 3, R: 4, S: 5, T: 6, U: 7);
	tup_impl_closure_call_derive_for_fn!($($name: $field),* ~ N: 0, O: 1, P: 2, Q: 3, R: 4, S: 5, T: 6, U: 7, V: 8);
	tup_impl_closure_call_derive_for_fn!($($name: $field),* ~ N: 0, O: 1, P: 2, Q: 3, R: 4, S: 5, T: 6, U: 7, V: 8, W: 9);
	tup_impl_closure_call_derive_for_fn!($($name: $field),* ~ N: 0, O: 1, P: 2, Q: 3, R: 4, S: 5, T: 6, U: 7, V: 8, W: 9, X: 10);
	tup_impl_closure_call_derive_for_fn!($($name: $field),* ~ N: 0, O: 1, P: 2, Q: 3, R: 4, S: 5, T: 6, U: 7, V: 8, W: 9, X: 10, Y: 11);
	tup_impl_closure_call_derive_for_fn!($($name: $field),* ~ N: 0, O: 1, P: 2, Q: 3, R: 4, S: 5, T: 6, U: 7, V: 8, W: 9, X: 10, Y: 11, Z: 12);
}

impl_tuples!(tup_impl_closure_call);
