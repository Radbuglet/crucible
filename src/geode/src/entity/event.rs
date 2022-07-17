pub macro delegate {
	// Muncher base case
	() => {},

	// Immutable
	(
		$(#[$attr:meta])*
		$vis:vis trait
			$name:ident
			$(::<$($generic_param:ident),*$(,)?>)?
			::
			$fn_name:ident
			$(<
				$($lt_decl:lifetime),*
				$(,)?
			>)?
		(
			&self,
			$($arg_name:ident: $arg_ty:ty),*
			$(,)?
		) $(-> $ret:ty)?;

		$($rest:tt)*
	) => {
		$(#[$attr:meta])*
		$vis trait $name $(<$($generic_param),*>)?: Send {
			fn $fn_name $(<$($lt_decl),*>)? (&self, $($arg_name: $arg_ty),*) $(-> $ret)?;
		}

		impl<F: Send $(,$($generic_param),*)?> $name $(<$($generic_param),*>)? for F
		where
			F: $(for<$($lt_decl),*>)? Fn($($arg_ty),*) $(-> $ret)?,
		{
			fn $fn_name $(<$($lt_decl),*>)? (&self, $($arg_name: $arg_ty),*) $(-> $ret)? {
				(self)($($arg_name),*)
			}
		}

		delegate!($($rest)*);
	},

	// Mutable
	(
		$(#[$attr:meta])*
		$vis:vis trait
			$name:ident
			$(::<$($generic_param:ident),*$(,)?>)?
			::
			$fn_name:ident
			$(<
				$($lt_decl:lifetime),*
				$(,)?
			>)?
		(
			&mut self,
			$($arg_name:ident: $arg_ty:ty),*
			$(,)?
		) $(-> $ret:ty)?;

		$($rest:tt)*
	) => {
		$(#[$attr:meta])*
		$vis trait $name $(<$($generic_param),*>)?: Send {
			fn $fn_name $(<$($lt_decl),*>)? (&mut self, $($arg_name: $arg_ty),*) $(-> $ret)?;
		}

		impl<F: Send $(,$($generic_param),*)?> $name $(<$($generic_param),*>)? for F
		where
			F: $(for<$($lt_decl),*>)? FnMut($($arg_ty),*) $(-> $ret)?,
		{
			fn $fn_name $(<$($lt_decl),*>)? (&mut self, $($arg_name: $arg_ty),*) $(-> $ret)? {
				(self)($($arg_name),*)
			}
		}

		delegate!($($rest)*);
	},
}
