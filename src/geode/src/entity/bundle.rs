use crucible_core::marker::PhantomInvariant;
use std::{
	borrow::Borrow,
	cell::{Ref, RefCell, RefMut},
	fmt, hash,
	marker::PhantomData,
};

use crate::core::{
	obj::ObjPointee,
	owned::{Destructible, Owned},
	session::Session,
};

use super::entity::{ComponentList, Entity};

#[allow(unused)] // Actually captured by the macro
use {
	super::{
		entity::{ComponentAttachTarget, SingleComponent},
		key::typed_key,
	},
	crate::core::{obj::Obj, owned::MaybeOwned},
	bytemuck::TransparentWrapper,
	crucible_core::macros::prefer_left,
};

pub trait ComponentBundle: Sized + Destructible + Borrow<Entity> {
	// === Required methods === //

	fn try_cast(session: Session, entity: Entity) -> anyhow::Result<Self>;

	fn force_cast(entity: Entity) -> Self;

	fn force_cast_ref(entity: &Entity) -> &Self;

	// === Derived casting methods === //

	fn try_cast_owned(session: Session, entity: Owned<Entity>) -> anyhow::Result<Owned<Self>> {
		entity.try_map_owned(|entity| Self::try_cast(session, entity))
	}

	fn force_cast_owned(entity: Owned<Entity>) -> Owned<Self> {
		entity.map_owned(|entity| Self::force_cast(entity))
	}

	fn unchecked_cast_owned(entity: Owned<Entity>) -> Owned<Self> {
		entity.map_owned(|entity| Self::cast(entity))
	}

	fn can_cast(session: Session, entity: Entity) -> bool {
		Self::try_cast(session, entity).is_ok()
	}

	fn cast(entity: Entity) -> Self {
		#[cfg(debug_assertions)]
		{
			use crate::core::session::LocalSessionGuard;
			use crucible_core::error::{AnyhowConvertExt, ErrorFormatExt};

			if let Err(err) =
				Self::try_cast(LocalSessionGuard::new().handle(), entity).into_std_error()
			{
				err.raise();
			}
		}
		Self::force_cast(entity)
	}

	// === Deconstructors === //

	fn raw(self) -> Entity {
		*self.raw_ref()
	}

	fn raw_ref(&self) -> &Entity {
		self.borrow()
	}
}

pub trait ComponentBundleWithCtor: ComponentBundle {
	type CompList: ComponentList;

	// === Entity constructors === //

	fn spawn(session: Session, components: Self::CompList) -> Owned<Self> {
		let entity = Entity::new_with(session, components).manually_destruct();
		let bundled = Self::force_cast(entity);

		Owned::new(bundled)
	}

	fn add_onto(session: Session, entity: Entity, components: Self::CompList) -> Self {
		entity.add(session, components);
		Self::force_cast(entity)
	}

	fn add_onto_owned(
		session: Session,
		entity: Owned<Entity>,
		components: Self::CompList,
	) -> Owned<Self> {
		entity.add(session, components);
		Self::force_cast_owned(entity)
	}
}

// === `Owned` integration === //

impl<T: ComponentBundle> Owned<T> {
	pub fn raw(self) -> Owned<Entity> {
		self.map_owned(|bundle| bundle.raw())
	}
}

// TODO: Deref integration

// === `EntityWith` === //

pub type EntityWithRw<T> = EntityWith<RefCell<T>>;

#[derive(TransparentWrapper)]
#[repr(transparent)]
#[transparent(Entity)]
pub struct EntityWith<T: ?Sized + ObjPointee> {
	_ty: PhantomInvariant<T>,
	entity: Entity,
}

impl<T: ?Sized + ObjPointee> ComponentBundle for EntityWith<T> {
	fn try_cast(session: Session, entity: Entity) -> anyhow::Result<Self> {
		if let Err(err) = entity.fallible_get::<T>(session) {
			if err.as_permission_error().is_none() {
				return Err(anyhow::Error::new(err).context(format!(
					"failed to construct `EntityWith<{}>` component bundle",
					std::any::type_name::<T>()
				)));
			}
		}
		Ok(Self::force_cast(entity))
	}

	fn force_cast(entity: Entity) -> Self {
		Self {
			_ty: PhantomData,
			entity,
		}
	}

	fn force_cast_ref(entity: &Entity) -> &Self {
		<Self as TransparentWrapper<Entity>>::wrap_ref(entity)
	}
}

impl<T: ?Sized + ObjPointee> ComponentBundleWithCtor for EntityWith<T> {
	type CompList = Option<MaybeOwned<Obj<T>>>;
}

impl<T: ?Sized + ObjPointee> fmt::Debug for EntityWith<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("EntityWith")
			.field("entity", &self.entity)
			.finish_non_exhaustive()
	}
}

impl<T: ?Sized + ObjPointee> Copy for EntityWith<T> {}

impl<T: ?Sized + ObjPointee> Clone for EntityWith<T> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<T: ?Sized + ObjPointee> hash::Hash for EntityWith<T> {
	fn hash<H: hash::Hasher>(&self, state: &mut H) {
		self.entity.hash(state);
	}
}

impl<T: ?Sized + ObjPointee> Eq for EntityWith<T> {}

impl<T: ?Sized + ObjPointee> PartialEq for EntityWith<T> {
	fn eq(&self, other: &Self) -> bool {
		self.entity == other.entity
	}
}

impl<T: ?Sized + ObjPointee> Borrow<Entity> for EntityWith<T> {
	fn borrow(&self) -> &Entity {
		&self.entity
	}
}

impl<T: ?Sized + ObjPointee> EntityWith<T> {
	pub fn get<'a>(&self, session: Session<'a>) -> &'a T {
		self.entity.get::<T>(session)
	}
}

impl<T: ?Sized + ObjPointee> EntityWithRw<T> {
	pub fn borrow(self, session: Session) -> Ref<T> {
		self.entity.borrow::<T>(session)
	}

	pub fn borrow_mut(self, session: Session) -> RefMut<T> {
		self.entity.borrow_mut::<T>(session)
	}
}

impl<T: ?Sized + ObjPointee> Destructible for EntityWith<T> {
	fn destruct(self) {
		self.entity.destruct();
	}
}

// TODO: Optional components to replace the old "trust me bro I'm gonna initialize this later" system.
pub macro component_bundle {
    () => {},
    (
        $vis:vis struct $bundle_name:ident
			$(<
				$($para_name:ident),*
				$(,)?
			>)?
		{
            $(..$ext_name:ident: $ext_ty:ty;)*

            $(
                $field_name:ident$([$key:expr])?: $field_ty:ty
            ),*
            $(,)?
        }

        $($rest:tt)*
    ) => {
        #[derive(TransparentWrapper)]
        #[repr(transparent)]
		#[transparent(Entity)]
        $vis struct $bundle_name$(<$($para_name: ?Sized + ObjPointee),*>)? {
			$(_invariant: PhantomData<fn($($para_name),*)>,)?
			// Seriously, don't name this field in the macro. `decl_macro` hygiene is far from finished.
			do_not_name_this_field_hygiene_is_jank: Entity,
		}

		impl$(<$($para_name: ?Sized + ObjPointee),*>)? fmt::Debug for $bundle_name$(<$($para_name),*>)? {
			fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
				f.debug_struct(stringify!($bundle_name))
					.field("entity", ComponentBundle::raw_ref(self))
					.finish_non_exhaustive()
			}
		}

		impl$(<$($para_name: ?Sized + ObjPointee),*>)? Copy for $bundle_name$(<$($para_name),*>)? {}

		impl$(<$($para_name: ?Sized + ObjPointee),*>)? Clone for $bundle_name$(<$($para_name),*>)? {
			fn clone(&self) -> Self {
				*self
			}
		}

		impl$(<$($para_name: ?Sized + ObjPointee),*>)? hash::Hash for $bundle_name$(<$($para_name),*>)? {
			fn hash<H: hash::Hasher>(&self, state: &mut H) {
				ComponentBundle::raw_ref(self).hash(state);
			}
		}

		impl$(<$($para_name: ?Sized + ObjPointee),*>)? Eq for $bundle_name$(<$($para_name),*>)? {}

		impl$(<$($para_name: ?Sized + ObjPointee),*>)? PartialEq for $bundle_name$(<$($para_name),*>)? {
			fn eq(&self, other: &Self) -> bool {
				ComponentBundle::raw_ref(self) == ComponentBundle::raw_ref(other)
			}
		}

        impl$(<$($para_name: ?Sized + ObjPointee),*>)? ComponentBundle for $bundle_name$(<$($para_name),*>)? {
            #[allow(unused)]  // `session` and `BUNDLE_MAKE_ERR` may be unused in empty bundles.
            fn try_cast(session: Session, entity: Entity) -> anyhow::Result<Self> {
                const BUNDLE_MAKE_ERR: &'static str = concat!(
                    "failed to construct ",
                    stringify!($bundle_name),
                    " component bundle"
                );

                $(
                    <$ext_ty as ComponentBundle>::try_cast(session, entity)?;
                )*

                $(
                    if let Err(err) = entity.fallible_get_in(session, prefer_left!(
                        $({$key})?
                        { typed_key::<$field_ty>() }
                    )) {
                        if err.as_permission_error().is_none() {
                            return Err(anyhow::Error::new(err).context(BUNDLE_MAKE_ERR));
                        }
                    }
                )*
                Ok(Self::force_cast(entity))
            }

            fn force_cast(entity: Entity) -> Self {
                <Self as TransparentWrapper<Entity>>::wrap(entity)
            }

            fn force_cast_ref(entity: &Entity) -> &Self {
                <Self as TransparentWrapper<Entity>>::wrap_ref(entity)
            }
        }

		impl$(<$($para_name: ?Sized + ObjPointee),*>)? $bundle_name$(<$($para_name),*>)? {
			$(
                pub fn $ext_name(&self) -> $ext_ty {
                    *self.as_ref()
                }
            )*

            $(
                pub fn $field_name<'a>(&self, session: Session<'a>) -> &'a $field_ty {
					<Self as TransparentWrapper<Entity>>::peel_ref(self).get_in(session, prefer_left!(
                        $({$key})?
                        { typed_key::<$field_ty>() }
                    ))
                }
            )*
		}

        impl$(<$($para_name: ?Sized + ObjPointee),*>)? Borrow<Entity> for $bundle_name$(<$($para_name),*>)? {
            fn borrow(&self) -> &Entity {
                <Self as TransparentWrapper<Entity>>::peel_ref(self)
            }
        }

        impl$(<$($para_name: ?Sized + ObjPointee),*>)? Destructible for $bundle_name$(<$($para_name),*>)? {
            fn destruct(self) {
                <Self as TransparentWrapper<Entity>>::peel(self).destruct();
            }
        }

        component_bundle!($($rest)*);
    },
	(
        $vis:vis struct $bundle_name:ident
		$(<
			$($para_name:ident),*
			$(,)?
		>)?
		($bundle_ctor_name:ident)
		{
            $(..$ext_name:ident: $ext_ty:ty;)*

            $(
                $field_name:ident$([$key:expr])?: $field_ty:ty
            ),*
            $(,)?
        }

        $($rest:tt)*
    ) => {
        component_bundle! {
            $vis struct $bundle_name $(<$($para_name),*>)? {
                $(..$ext_name: $ext_ty;)*

                $(
                    $field_name$([$key])?: $field_ty
                ),*
            }

            $($rest)*
        }

        $vis struct $bundle_ctor_name$(<$($para_name: ?Sized + ObjPointee),*>)? {
            $(pub $ext_name: <$ext_ty as ComponentBundle>::CompList,)*
            $(pub $field_name: Option<MaybeOwned<Obj<$field_ty>>>,)*
        }

        impl$(<$($para_name: ?Sized + ObjPointee),*>)? ComponentList for $bundle_ctor_name$(<$($para_name),*>)? {
            #[allow(unused)]  // `registry` may be unused in empty bundles.
            fn push_values(self, registry: &mut ComponentAttachTarget) {
                $(ComponentList::push_values(self.$ext_name, registry);)*
                $(SingleComponent::push_value_under(self.$field_name, registry, prefer_left!(
                    $({$key})? {typed_key()}
                ));)*
            }
        }

        impl$(<$($para_name: ?Sized + ObjPointee),*>)? ComponentBundleWithCtor for $bundle_name$(<$($para_name),*>)? {
            type CompList = $bundle_ctor_name$(<$($para_name),*>)?;
        }
    },
}
