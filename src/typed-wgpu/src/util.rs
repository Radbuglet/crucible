// === transparent_wrapper === //

// TODO: Extract into `crucible-util`.
macro_rules! transparent_wrapper {
	($(
		$(#[$attr:meta])*
		$vis:vis struct $name:ident
			$(<
				$($lt:lifetime),*
				$(,)?
				$($para:ident),*
				$(,)?
			>)?
			($raw:ty)
			$( where { $($where_clause:tt)* } )?;
	)*) => {$(
		#[::derive_where::derive_where(Debug)]
		$(#[$attr])*
		$vis struct $name<$($($lt,)* $($para,)*)? T: ?Sized>
		$(where $($where_clause)*)?
		{
			$vis ty: ::std::marker::PhantomData<fn(T) -> T>,
			$vis raw: $raw,
		}

		impl<$($($lt,)* $($para,)*)? T: ?Sized> $name<$($($lt,)* $($para,)*)? T>
		$(where $($where_clause)*)?
		{
			pub fn new(raw: $raw) -> Self {
				Self {
					ty: ::std::marker::PhantomData,
					raw,
				}
			}

			pub fn new_ref<'_rlt>(raw: &'_rlt $raw) -> &'_rlt Self {
				unsafe { ::std::mem::transmute(raw) }
			}

			pub fn new_mut<'_rlt>(raw: &'_rlt mut $raw) -> &'_rlt mut Self {
				unsafe { ::std::mem::transmute(raw) }
			}
		}

		impl<$($($lt,)* $($para,)*)? T: ?Sized> From<$raw> for $name<$($($lt,)* $($para,)*)? T>
		$(where $($where_clause)*)?
		{
			fn from(raw: $raw) -> Self {
				Self::new(raw)
			}
		}

		impl<$($($lt,)* $($para,)*)? T: ?Sized> From<$name<$($($lt,)* $($para,)*)? T>> for $raw
		$(where $($where_clause)*)?
		{
			fn from(me: $name<$($($lt,)* $($para,)*)? T>) -> $raw {
				me.raw
			}
		}
	)*};
}

pub(crate) use transparent_wrapper;

// === SlotAssigner === //

#[derive(Debug, Clone, Default)]
pub struct SlotAssigner {
	next_slot: u32,
}

impl SlotAssigner {
	pub fn jump_to(&mut self, slot: u32) {
		self.next_slot = slot;
	}

	pub fn peek(&self) -> u32 {
		self.next_slot
	}

	pub fn next(&mut self) -> u32 {
		let binding = self.next_slot;
		self.next_slot = self
			.next_slot
			.checked_add(1)
			.expect("Cannot create a binding at slot `u32::MAX`.");

		binding
	}
}
