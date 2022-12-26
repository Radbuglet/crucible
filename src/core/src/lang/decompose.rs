use super::macros::impl_tuples;

pub trait Decompose<T, R, V> {
	fn decompose(self) -> (T, R);
}

macro impl_decompose($first_para:ident:$first_field:tt $(,$para:ident:$field:tt)*) {
	impl_decompose_inner!([$first_para:$first_field] $(,$para:$field)*);
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
	},
}

impl_tuples!(impl_decompose; no_unit);
