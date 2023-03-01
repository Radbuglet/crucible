// #[macro_export]
// macro_rules! transparent {
// 	// Front-end to convert $ty_inherit:ty into `[$ty_inherit:tt]`
// 	($(
// 		$(#[$attr:meta])*
// 		$vis:vis struct $name:ident
// 		$(<
// 			$($lt:lifetime $(: $lt_inherit_first:lifetime $(+ $lt_inherit_subs:lifetime)* )?),*
// 			$(,)?
// 			$($ty:ident $(: $ty_inherit:ty)? $(= $ty_default:ty)?),*
// 			$(,const $const:ident $(: $const_ty:ty)? $(= {$const_default:expr})? )*
// 			$(,)?
// 		>)?
// 		$(where { $($clause:tt)* })?
// 		{
// 			$(
// 				$(#[$f_attr:meta])*
// 				$f_vis:vis $f_name:ident: $f_ty:ty
// 			),*
// 			$(,)?
// 		}
// 	)*) => {
// 		$crate::transparent! {~INTERNAL $(
// 			$(#[$attr])*
// 			$vis struct $name
// 			$(<
// 				$($lt $(: $lt_inherit_first $(+ $lt_inherit_subs)* )?,)*
// 				$($ty $(: {$ty_inherit})? $(= $ty_default)?,)*
// 				$(const $const $(: $const_ty)? $(= {$const_default})?, )*
// 			>)?
// 			$(where { $($clause)* })?
// 			{
// 				$(
// 					$(#[$f_attr])*
// 					$f_vis $f_name: $f_ty
// 					,
// 				)*
// 			}
// 		)*}
// 	};
// 	// Actual logic
// 	(~INTERNAL $(
// 		$(#[$attr:meta])*
// 		$vis:vis struct $name:ident
// 		$(<
// 			$($lt:lifetime $(: $lt_inherit_first:lifetime $(+ $lt_inherit_subs:lifetime)* )?,)*
// 			$($ty:ident $(: {$($ty_inherit:tt)*})? $(= $ty_default:ty)?,)*
// 			$(const $const:ident $(: $const_ty:ty)? $(= {$const_default:expr})?,)*
// 		>)?
// 		$(where { $($clause:tt)* })?
// 		{
// 			$(
// 				$(#[$f_attr:meta])*
// 				$f_vis:vis $f_name:ident: $f_ty:ty
// 				,
// 			)*
// 		}
// 	)*) => {$(
// 		$(#[$attr])*
// 		$vis struct $name $(<
// 			$($lt $(: $lt_inherit_first $(+ $lt_inherit_subs)*)?,)*
// 			$($ty $(: $($ty_inherit)*,)? $(= $($ty_default)*,)? )*
// 		>)? {
// 			$(
// 				$(#[$f_attr])*
// 				$f_vis $f_name: $f_ty,
// 			)*
// 		}
// 	)*};
// }
//
// transparent! {
// 	#[derive(Debug)]
// 	pub struct Foo<'a: 'b + 'c, 'b, 'c, T: Sized> {
// 		a: &'a u32,
// 		b: &'b u32,
// 		c: &'c u32,
// 		d: &'c T,
// 	}
// }
