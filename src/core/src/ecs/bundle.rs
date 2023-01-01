use super::entity::Entity;

pub trait Bundle {
	type Context<'a>;

	fn attach(self, cx: Self::Context<'_>, target: Entity);
	fn detach(cx: Self::Context<'_>, target: Entity) -> Self;
}

#[macro_export]
macro_rules! bundle {
	($(
		$(#[$attr_meta:meta])*
		$vis:vis struct $name:ident {
			$(
				$(#[$field_meta:meta])*
				$field_vis:vis $field:ident: $ty:ty
			),*
			$(,)?
		}
	)*) => {$(
		$(#[$attr_meta])*
		$vis struct $name {
			$(
				$(#[$field_meta])*
				$field_vis $field: $ty
			),*
		}

		impl $crate::ecs::bundle::Bundle for $name {
			type Context<'a> = ($(&'a mut $crate::ecs::storage::Storage<$ty>,)*);

			#[allow(unused)]
			fn attach(self, mut cx: Self::Context<'_>, target: Entity) {
				$(
					$crate::ecs::context::decompose!(cx => {
						storage: &mut $crate::ecs::storage::Storage<$ty>
					});
					storage.add(target, self.$field);
				)*
			}

			#[allow(unused)]
			fn detach(mut cx: Self::Context<'_>, target: Entity) -> Self {
				$(
					$crate::ecs::context::decompose!(cx => {
						storage: &mut $crate::ecs::storage::Storage<$ty>
					});
					let $field = storage.try_remove(target).unwrap();
				)*

				Self { $($field),* }
			}
		}
	)*};
}

pub use bundle;
