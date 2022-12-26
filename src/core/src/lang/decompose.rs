use std::marker::PhantomData;

use super::macros::impl_tuples;

pub struct TupleHole {
	_private: (),
}

pub trait Decompose<T, R, V> {
	fn decompose(self) -> (T, R);
}

pub trait TupleTruncate<R> {
	fn truncate(self) -> R;
}

pub struct TupleTypeInference<T> {
	_ty: PhantomData<T>,
}

pub trait TupleTypeInferenceHygieneBend<V, R>: Sized {
	fn id(self, v: V) -> (V, TupleTypeInference<R>) {
		(v, TupleTypeInference::new())
	}
}

impl<T> TupleTypeInference<T> {
	pub fn new() -> Self {
		Self { _ty: PhantomData }
	}

	pub fn id_all_infer(&self) -> Option<T> {
		None
	}
}

impl TupleTypeInferenceHygieneBend<TupleHole, ()> for TupleTypeInference<()> {}

macro impl_decompose($first_para:ident:$first_field:tt $(,$para:ident:$field:tt)*) {
	impl_decompose_inner!([$first_para:$first_field] $(,$para:$field)*);

	impl<$first_para $(,$para)*> Decompose<TupleHole, Self, TupleHole> for ($first_para, $($para),*) {
		fn decompose(self) -> (TupleHole, Self) {
			(TupleHole { _private: () }, self)
		}
	}

	impl<$first_para $(,$para)*>
		TupleTypeInferenceHygieneBend<$first_para, ($($para,)*)> for
		TupleTypeInference<($first_para, $($para,)*)> {}
}

macro impl_decompose_inner {
	(
		$($left_para:ident:$left_field:tt,)*
		[$curr_para:ident:$curr_field:tt]
		,$next_para:ident:$next_field:tt
		$(,$right_para:ident:$right_field:tt)*
	) => {
		impl<$($left_para,)* $curr_para, $next_para $(,$right_para)*>
			Decompose<
				$curr_para,
				($($left_para,)* $next_para, $($right_para),*),
				($($left_para,)*),
			>
			for
			($($left_para,)* $curr_para, $next_para $(,$right_para)*)
		{
			fn decompose(self) -> ($curr_para, ($($left_para,)* $next_para, $($right_para),*)) {
				(self.$curr_field, (
                    $(self.$left_field,)*
                    self.$next_field,
                    $(self.$right_field,)*
                ))
			}
		}

		impl<$($left_para,)* $curr_para, $next_para $(,$right_para)*>
			TupleTruncate<($($left_para,)*)> for
			($($left_para,)* $curr_para, $next_para $(,$right_para)*)
		{
			#[allow(unused_variables, non_snake_case)]
			fn truncate(self) -> ($($left_para,)*) {
				let ($($left_para,)* $curr_para, $next_para $(,$right_para)*) = self;
				($($left_para,)*)
			}
		}

		impl_decompose_inner!(
			$($left_para:$left_field,)*
			$curr_para:$curr_field,
			[$next_para:$next_field]
			$(,$right_para:$right_field)*
		);
	},
	(
		$($left_para:ident:$left_field:tt,)*
		[$curr_para:ident:$curr_field:tt]
	) => {
		impl<$($left_para,)* $curr_para>
			Decompose<
				$curr_para,
				($($left_para,)*),
				($($left_para,)*),
			>
			for
			($($left_para,)* $curr_para,)
		{
			fn decompose(self) -> ($curr_para, ($($left_para,)*)) {
				(self.$curr_field, ($(self.$left_field,)*))
			}
		}

		impl<$($left_para,)* $curr_para>
			TupleTruncate<($($left_para,)*)> for
			($($left_para,)* $curr_para,)
		{
			#[allow(unused_variables, non_snake_case)]
			fn truncate(self) -> ($($left_para,)*) {
				let ($($left_para,)* $curr_para,) = self;
				($($left_para,)*)
			}
		}
	},
}

impl_tuples!(impl_decompose; no_unit);

pub macro decompose($input:expr) {
	'a: {
		let decomposer = TupleTypeInference::new();

		if let Some(v) = decomposer.id_all_infer() {
			fn any<T>() -> T {
				unreachable!();
			}

			break 'a (v, any());
		}

		// TODO: Stop hardcoding maximum arity of tuple.
		let (v, input) = Decompose::decompose($input);
		let (a, decomposer) = TupleTypeInferenceHygieneBend::id(decomposer, v);

		let (v, input) = Decompose::decompose(input);
		let (b, decomposer) = TupleTypeInferenceHygieneBend::id(decomposer, v);

		let (v, input) = Decompose::decompose(input);
		let (c, decomposer) = TupleTypeInferenceHygieneBend::id(decomposer, v);

		let (v, input) = Decompose::decompose(input);
		let (d, decomposer) = TupleTypeInferenceHygieneBend::id(decomposer, v);

		let (v, input) = Decompose::decompose(input);
		let (e, decomposer) = TupleTypeInferenceHygieneBend::id(decomposer, v);

		let (v, input) = Decompose::decompose(input);
		let (f, decomposer) = TupleTypeInferenceHygieneBend::id(decomposer, v);

		let (v, input) = Decompose::decompose(input);
		let (g, decomposer) = TupleTypeInferenceHygieneBend::id(decomposer, v);

		let (v, input) = Decompose::decompose(input);
		let (h, decomposer) = TupleTypeInferenceHygieneBend::id(decomposer, v);

		let (v, input) = Decompose::decompose(input);
		let (i, decomposer) = TupleTypeInferenceHygieneBend::id(decomposer, v);

		let (v, input) = Decompose::decompose(input);
		let (j, decomposer) = TupleTypeInferenceHygieneBend::id(decomposer, v);

		let (v, input) = Decompose::decompose(input);
		let (k, decomposer) = TupleTypeInferenceHygieneBend::id(decomposer, v);

		let (v, input) = Decompose::decompose(input);
		let (l, decomposer) = TupleTypeInferenceHygieneBend::id(decomposer, v);

		let (v, input) = Decompose::decompose(input);
		let (m, decomposer) = TupleTypeInferenceHygieneBend::id(decomposer, v);

		let _ = decomposer;
		(
			TupleTruncate::truncate((a, b, c, d, e, f, g, h, i, j, k, l, m)),
			input,
		)
	}
}
