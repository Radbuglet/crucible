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

pub macro prefer_left({$($chosen:tt)*} $({$($_ignored:tt)*})*) {
	$($chosen)*
}
