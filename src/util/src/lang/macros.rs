#[macro_export]
macro_rules! impl_tuples {
	// === impl_tuples_with === //
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
		$crate::lang::macros::impl_tuples!(
			$target : [
				$($rest)*
			] | [
				$($({$($pre)*})*)?
				{$($next)*}
			]
		);
	};

	// === impl_tuples === //
	($target:path; no_unit) => {
		$crate::lang::macros::impl_tuples!(
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
				{M: 12}
			]
		);
	};
	($target:path) => {
		$target!();
		$crate::lang::macros::impl_tuples!($target; no_unit);
	};
}

pub use impl_tuples;
