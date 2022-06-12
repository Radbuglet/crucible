//! Utilities to write randomized tests

use crate::util::number::{NumberGenMut, U64Generator};

pub fn init_seed() {
	let seed = fastrand::u64(..);
	fastrand::seed(seed);
	println!("Set seed to {seed}.");
}

pub macro rand_choice(
	$($cond:expr => $out:expr,)*
	_ => $fallback:expr$(,)?
) {{
	let mut choice_gen = 0usize;
	let mut choices = [0; 0 $(+ {
		bind!($cond);
		1
	})*];

	{
		let mut branch_gen = U64Generator { next: 0 };
		$(
			if $cond {
				choices[choice_gen] = branch_gen.next;
				choice_gen += 1;
			}
			branch_gen.generate_mut();
		)*
	}

	if choice_gen > 0 {
		let index = choices[fastrand::usize(0..choice_gen)];
		let mut branch_gen = U64Generator { next: 0 };

		$(if index == branch_gen.generate_mut() {
			$out
		} else)* {
			unreachable!();
		}
	} else {
		$fallback
	}
}}

pub macro bind($($tt:tt)*) {}
