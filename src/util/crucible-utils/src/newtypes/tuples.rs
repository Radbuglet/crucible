#[macro_export]
macro_rules! impl_tuples {
	// Internal
	(
		$target:path : []
		$(| [
			$({$($pre:tt)*})*
		])?
	) => { /* terminal recursion case */ };
	(
		$target:path : [
			{$($next:tt)*}
			// Remaining invocations
			$($rest:tt)*
		] $(| [
			// Accumulated arguments
			$({$($pre:tt)*})*
		])?
	) => {
		$target!(
			$($($($pre)*,)*)?
			$($next)*
		);
		$crate::impl_tuples!(
			$target : [
				$($rest)*
			] | [
				$($({$($pre)*})*)?
				{$($next)*}
			]
		);
	};

	// Public
	($target:path; no_unit) => {
		$crate::impl_tuples!(
			$target : [
				{A: 0}
				{B: 1}
				{C: 2}
				{D: 3}
				{E: 4}
				{F: 5}
				{G: 6}
				{H: 7}
				{I: 8}
				{J: 9}
				{K: 10}
				{L: 11}
			]
		);
	};
	($target:path; only_full) => {
		$target!(
			A:0,
			B:1,
			C:2,
			D:3,
			E:4,
			F:5,
			G:6,
			H:7,
			I:8,
			J:9,
			K:10,
			L:11
		);
	};
	($target:path) => {
		$target!();
		$crate::impl_tuples!($target; no_unit);
	};
}

pub use impl_tuples;
