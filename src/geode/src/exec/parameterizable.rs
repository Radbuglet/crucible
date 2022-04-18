use std::marker::PhantomData;

pub trait Parameterizable<'a, 'b, 'c, 'd> {
	type Value;
}

pub struct Unpara<T>(PhantomData<fn(T) -> T>);

impl<T> Parameterizable<'_, '_, '_, '_> for Unpara<T> {
	type Value = T;
}

#[rustfmt::skip]
pub macro event_type {
	// Base case
	() => {},
	// Atom
	(
		$(#[$attr:meta])*
		$vis:vis struct $name:ident $(<>)?;
		$($rest:tt)*
	) => {
		$(#[$attr])*
		$vis struct $name;
		
		impl Parameterizable<'_, '_, '_, '_> for $name {
			type Value = $name;
		}
		
		event_type!($($rest)*);
	},
	// Braced
	(
		$(#[$attr:meta])*
		$vis:vis struct $name:ident $(<>)? { $($def:tt)* }
		$($rest:tt)*
	) => {
		$(#[$attr])*
		$vis struct $name { $($def)* }
		
		impl Parameterizable<'_, '_, '_, '_> for $name {
			type Value = $name;
		}
		
		event_type!($($rest)*);
	},
	(
		$(#[$attr:meta])*
		$vis:vis struct $name:ident <
			$pa:lifetime
			$(,)?
		> { $($def:tt)* }
		$($rest:tt)*
	) => {
		$(#[$attr])*
		$vis struct $name <$pa> { $($def)* }
		
		impl<$pa>
			Parameterizable<$pa, '_, '_, '_>
			for $name<'static>
		{
			type Value = $name<$pa>;
		}
		
		event_type!($($rest)*);
	},
	(
		$(#[$attr:meta])*
		$vis:vis struct $name:ident <
			$pa:lifetime,
			$pb:lifetime
			$(,)?
		> { $($def:tt)* }
		$($rest:tt)*
	) => {
		$(#[$attr])*
		$vis struct $name <$pa, $pb> { $($def)* }
		
		impl<$pa, $pb>
			Parameterizable<$pa, $pb, '_, '_>
			for $name<'static, 'static>
		{
			type Value = $name<$pa, $pb>;
		}
		
		event_type!($($rest)*);
	},
	(
		$(#[$attr:meta])*
		$vis:vis struct $name:ident <
			$pa:lifetime,
			$pb:lifetime,
			$pc:lifetime
			$(,)?
		> { $($def:tt)* }
		$($rest:tt)*
	) => {
		$(#[$attr])*
		$vis struct $name <$pa, $pb, $pc> { $($def)* }
		
		impl<$pa, $pb, $pc>
			Parameterizable<$pa, $pb, $pc, '_>
			for $name<'static, 'static, 'static>
		{
			type Value = $name<$pa, $pb, $pc>;
		}
		
		event_type!($($rest)*);
	},
	(
		$(#[$attr:meta])*
		$vis:vis struct $name:ident <
			$pa:lifetime,
			$pb:lifetime,
			$pc:lifetime,
			$pd:lifetime
			$(,)?
		> { $($def:tt)* }
		$($rest:tt)*
	) => {
		$(#[$attr])*
		$vis struct $name <$pa, $pb, $pc, $pd> { $($def)* }
		
		impl<$pa, $pb, $pc, $pd>
			Parameterizable<$pa, $pb, $pc, $pd>
			for $name<'static, 'static, 'static, 'static>
		{
			type Value = $name<$pa, $pb, $pc, $pd>;
		}
		
		event_type!($($rest)*);
	},
	(
		$(#[$attr:meta])*
		$vis:vis struct $name:ident <
			$pa:lifetime,
			$pb:lifetime,
			$pc:lifetime,
			$pd:lifetime,
			$($l:lifetime),*$(,)?
		> { $($def:tt)* }
		$($rest:tt)*
	) => {
		compile_error!("`event_type` does not support event types with more than 4 lifetime parameters!");
	},
	// Parenthesized
	(
		$(#[$attr:meta])*
		$vis:vis struct $name:ident $(<>)? ( $($def:tt)* );
		$($rest:tt)*
	) => {
		$(#[$attr])*
		$vis struct $name ( $($def)* );
		
		impl Parameterizable<'_, '_, '_, '_> for $name {
			type Value = $name;
		}
		
		event_type!($($rest)*);
	},
	(
		$(#[$attr:meta])*
		$vis:vis struct $name:ident <
			$pa:lifetime
			$(,)?
		> ( $($def:tt)* );
		$($rest:tt)*
	) => {
		$(#[$attr])*
		$vis struct $name <$pa> ( $($def)* );
		
		impl<$pa>
			Parameterizable<$pa, '_, '_, '_>
			for $name<'static>
		{
			type Value = $name<$pa>;
		}
		
		event_type!($($rest)*);
	},
	(
		$(#[$attr:meta])*
		$vis:vis struct $name:ident <
			$pa:lifetime,
			$pb:lifetime
			$(,)?
		> ( $($def:tt)* );
		$($rest:tt)*
	) => {
		$(#[$attr])*
		$vis struct $name <$pa, $pb> ( $($def)* );
		
		impl<$pa, $pb>
			Parameterizable<$pa, $pb, '_, '_>
			for $name<'static, 'static>
		{
			type Value = $name<$pa, $pb>;
		}
		
		event_type!($($rest)*);
	},
	(
		$(#[$attr:meta])*
		$vis:vis struct $name:ident <
			$pa:lifetime,
			$pb:lifetime,
			$pc:lifetime
			$(,)?
		> ( $($def:tt)* );
		$($rest:tt)*
	) => {
		$(#[$attr])*
		$vis struct $name <$pa, $pb, $pc> ( $($def)* );
		
		impl<$pa, $pb, $pc>
			Parameterizable<$pa, $pb, $pc, '_>
			for $name<'static, 'static, 'static>
		{
			type Value = $name<$pa, $pb, $pc>;
		}
		
		event_type!($($rest)*);
	},
	(
		$(#[$attr:meta])*
		$vis:vis struct $name:ident <
			$pa:lifetime,
			$pb:lifetime,
			$pc:lifetime,
			$pd:lifetime
			$(,)?
		> ( $($def:tt)* );
		$($rest:tt)*
	) => {
		$(#[$attr])*
		$vis struct $name <$pa, $pb, $pc, $pd> ( $($def)* );
		
		impl<$pa, $pb, $pc, $pd>
			Parameterizable<$pa, $pb, $pc, $pd>
			for $name<'static, 'static, 'static, 'static>
		{
			type Value = $name<$pa, $pb, $pc, $pd>;
		}
		
		event_type!($($rest)*);
	},
	(
		$(#[$attr:meta])*
		$vis:vis struct $name:ident <
			$pa:lifetime,
			$pb:lifetime,
			$pc:lifetime,
			$pd:lifetime,
			$($l:lifetime),*$(,)?
		> ( $($def:tt)* );
		$($rest:tt)*
	) => {
		compile_error!("`event_type` does not support event types with more than 4 lifetime parameters!");
	},
	// Enums
	(
		$(#[$attr:meta])*
		$vis:vis enum $name:ident $(<>)? { $($def:tt)* }
		$($rest:tt)*
	) => {
		$(#[$attr])*
		$vis enum $name { $($def)* }
		
		impl Parameterizable<'_, '_, '_, '_> for $name {
			type Value = $name;
		}
		
		event_type!($($rest)*);
	},
	(
		$(#[$attr:meta])*
		$vis:vis enum $name:ident <
			$pa:lifetime
			$(,)?
		> { $($def:tt)* }
		$($rest:tt)*
	) => {
		$(#[$attr])*
		$vis enum $name <$pa> { $($def)* }
		
		impl<$pa>
			Parameterizable<$pa, '_, '_, '_>
			for $name<'static>
		{
			type Value = $name<$pa>;
		}
		
		event_type!($($rest)*);
	},
	(
		$(#[$attr:meta])*
		$vis:vis enum $name:ident <
			$pa:lifetime,
			$pb:lifetime
			$(,)?
		> { $($def:tt)* }
		$($rest:tt)*
	) => {
		$(#[$attr])*
		$vis enum $name <$pa, $pb> { $($def)* }
		
		impl<$pa, $pb>
			Parameterizable<$pa, $pb, '_, '_>
			for $name<'static, 'static>
		{
			type Value = $name<$pa, $pb>;
		}
		
		event_type!($($rest)*);
	},
	(
		$(#[$attr:meta])*
		$vis:vis enum $name:ident <
			$pa:lifetime,
			$pb:lifetime,
			$pc:lifetime
			$(,)?
		> { $($def:tt)* }
		$($rest:tt)*
	) => {
		$(#[$attr])*
		$vis enum $name <$pa, $pb, $pc> { $($def)* }
		
		impl<$pa, $pb, $pc>
			Parameterizable<$pa, $pb, $pc, '_>
			for $name<'static, 'static, 'static>
		{
			type Value = $name<$pa, $pb, $pc>;
		}
		
		event_type!($($rest)*);
	},
	(
		$(#[$attr:meta])*
		$vis:vis enum $name:ident <
			$pa:lifetime,
			$pb:lifetime,
			$pc:lifetime,
			$pd:lifetime
			$(,)?
		> { $($def:tt)* }
		$($rest:tt)*
	) => {
		$(#[$attr])*
		$vis enum $name <$pa, $pb, $pc, $pd> { $($def)* }
		
		impl<$pa, $pb, $pc, $pd>
			Parameterizable<$pa, $pb, $pc, $pd>
			for $name<'static, 'static, 'static, 'static>
		{
			type Value = $name<$pa, $pb, $pc, $pd>;
		}
		
		event_type!($($rest)*);
	},
	(
		$(#[$attr:meta])*
		$vis:vis enum $name:ident <
			$pa:lifetime,
			$pb:lifetime,
			$pc:lifetime,
			$pd:lifetime,
			$($l:lifetime),*$(,)?
		> { $($def:tt)* }
		$($rest:tt)*
	) => {
		compile_error!("`event_type` does not support event types with more than 4 lifetime parameters!");
	},
	// Unions
	(
		$(#[$attr:meta])*
		$vis:vis union $name:ident $(<>)? { $($def:tt)* }
		$($rest:tt)*
	) => {
		$(#[$attr])*
		$vis union $name { $($def)* }
		
		impl Parameterizable<'_, '_, '_, '_> for $name {
			type Value = $name;
		}
		
		event_type!($($rest)*);
	},
	(
		$(#[$attr:meta])*
		$vis:vis union $name:ident <
			$pa:lifetime
			$(,)?
		> { $($def:tt)* }
		$($rest:tt)*
	) => {
		$(#[$attr])*
		$vis union $name <$pa> { $($def)* }
		
		impl<$pa>
			Parameterizable<$pa, '_, '_, '_>
			for $name<'static>
		{
			type Value = $name<$pa>;
		}
		
		event_type!($($rest)*);
	},
	(
		$(#[$attr:meta])*
		$vis:vis union $name:ident <
			$pa:lifetime,
			$pb:lifetime
			$(,)?
		> { $($def:tt)* }
		$($rest:tt)*
	) => {
		$(#[$attr])*
		$vis union $name <$pa, $pb> { $($def)* }
		
		impl<$pa, $pb>
			Parameterizable<$pa, $pb, '_, '_>
			for $name<'static, 'static>
		{
			type Value = $name<$pa, $pb>;
		}
		
		event_type!($($rest)*);
	},
	(
		$(#[$attr:meta])*
		$vis:vis union $name:ident <
			$pa:lifetime,
			$pb:lifetime,
			$pc:lifetime
			$(,)?
		> { $($def:tt)* }
		$($rest:tt)*
	) => {
		$(#[$attr])*
		$vis union $name <$pa, $pb, $pc> { $($def)* }
		
		impl<$pa, $pb, $pc>
			Parameterizable<$pa, $pb, $pc, '_>
			for $name<'static, 'static, 'static>
		{
			type Value = $name<$pa, $pb, $pc>;
		}
		
		event_type!($($rest)*);
	},
	(
		$(#[$attr:meta])*
		$vis:vis union $name:ident <
			$pa:lifetime,
			$pb:lifetime,
			$pc:lifetime,
			$pd:lifetime
			$(,)?
		> { $($def:tt)* }
		$($rest:tt)*
	) => {
		$(#[$attr])*
		$vis union $name <$pa, $pb, $pc, $pd> { $($def)* }
		
		impl<$pa, $pb, $pc, $pd>
			Parameterizable<$pa, $pb, $pc, $pd>
			for $name<'static, 'static, 'static, 'static>
		{
			type Value = $name<$pa, $pb, $pc, $pd>;
		}
		
		event_type!($($rest)*);
	},
	(
		$(#[$attr:meta])*
		$vis:vis union $name:ident <
			$pa:lifetime,
			$pb:lifetime,
			$pc:lifetime,
			$pd:lifetime,
			$($l:lifetime),*$(,)?
		> { $($def:tt)* }
		$($rest:tt)*
	) => {
		compile_error!("`event_type` does not support event types with more than 4 lifetime parameters!");
	},
}
