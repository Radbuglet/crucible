use crucible_core::{
	error::{AnyhowConvertExt, ErrorFormatExt},
	marker::PhantomInvariant,
};
use derive_where::derive_where;
use std::{
	borrow::Borrow,
	cell::{Ref, RefCell, RefMut},
	marker::PhantomData,
	mem::transmute,
	ops::Deref,
};

use crate::core::{
	obj::ObjPointee,
	owned::{Destructible, Owned},
	session::{LocalSessionGuard, Session},
};

use super::entity::{ComponentList, Entity};

#[allow(unused)] // Actually captured by the macro
use {
	super::{
		entity::{ComponentAttachTarget, OwnedOrWeak, SingleComponent},
		key::typed_key,
	},
	bytemuck::TransparentWrapper,
	crucible_core::macros::prefer_left,
};

pub trait ComponentBundle: Sized + Destructible + Borrow<Entity> + Deref {
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
		entity.map_owned(|entity| Self::unchecked_cast(entity))
	}

	fn can_cast(session: Session, entity: Entity) -> bool {
		Self::try_cast(session, entity).is_ok()
	}

	fn unchecked_cast(entity: Entity) -> Self {
		#[cfg(debug_assertions)]
		{
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

impl<T: ComponentBundle> Deref for Owned<T> {
	type Target = T::Target;

	fn deref(&self) -> &Self::Target {
		self.weak_copy_ref().deref()
	}
}

// === `EntityWith` === //

pub type EntityWithRw<T> = EntityWith<RefCell<T>>;

#[derive_where(Debug, Copy, Clone, Hash, Eq, PartialEq)]
#[repr(transparent)]
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
		// `derive(TransparentWrapper)` crashes on this struct so we just have to do this manually.
		unsafe { transmute(entity) }
	}
}

impl<T: ?Sized + ObjPointee> Borrow<Entity> for EntityWith<T> {
	fn borrow(&self) -> &Entity {
		&self.entity
	}
}

impl<T: ?Sized + ObjPointee> Deref for EntityWith<T> {
	type Target = EntityWithBundledMethods<T>;

	fn deref(&self) -> &Self::Target {
		unsafe { transmute(self) }
	}
}

impl<T: ?Sized + ObjPointee> Destructible for EntityWith<T> {
	fn destruct(self) {
		self.entity.destruct();
	}
}

#[repr(transparent)]
pub struct EntityWithBundledMethods<T: ?Sized + ObjPointee> {
	_ty: PhantomInvariant<T>,
	entity: Entity,
}

impl<T: ?Sized + ObjPointee> EntityWithBundledMethods<T> {
	pub fn get(self, session: Session) -> &T {
		self.entity.get::<T>(session)
	}
}

impl<T: ?Sized + ObjPointee> EntityWithBundledMethods<RefCell<T>> {
	pub fn borrow(self, session: Session) -> Ref<T> {
		self.entity.borrow::<T>(session)
	}

	pub fn borrow_mut(self, session: Session) -> RefMut<T> {
		self.entity.borrow_mut::<T>(session)
	}
}

pub macro component_bundle {
    () => {},
    (
        $vis:vis struct $bundle_name:ident($bundle_ctor_name:ident) {
            $(..$ext_name:ident: $ext_ty:ty;)*

            $(
                $field_name:ident$([$key:expr])?: $field_ty:ty
            ),*
            $(,)?
        }

        $($rest:tt)*
    ) => {
        component_bundle! {
            $vis struct $bundle_name {
                $(..$ext_name: $ext_ty;)*

                $(
                    $field_name$([$key])?: $field_ty
                ),*
            }

            $($rest)*
        }

        #[derive(Debug)]
        $vis struct $bundle_ctor_name {
            $(pub $ext_name: <$ext_ty as ComponentBundle>::CompList,)*
            $(pub $field_name: Option<OwnedOrWeak<$field_ty>>,)*
        }

        impl ComponentList for $bundle_ctor_name {
            #[allow(unused)]  // `registry` may be unused in empty bundles.
            fn push_values(self, registry: &mut ComponentAttachTarget) {
                $(ComponentList::push_values(self.$ext_name, registry);)*
                $(SingleComponent::push_value_under(self.$field_name, registry, prefer_left!(
                    $({$key})? {typed_key()}
                ));)*
            }
        }

        impl ComponentBundleWithCtor for $bundle_name {
            type CompList = $bundle_ctor_name;
        }
    },
    (
        $vis:vis struct $bundle_name:ident {
            $(..$ext_name:ident: $ext_ty:ty;)*

            $(
                $field_name:ident$([$key:expr])?: $field_ty:ty
            ),*
            $(,)?
        }

        $($rest:tt)*
    ) => {
        #[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, TransparentWrapper)]
        #[repr(transparent)]
        $vis struct $bundle_name(Entity);

        impl ComponentBundle for $bundle_name {
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
                Self(entity)
            }

            fn force_cast_ref(entity: &Entity) -> &Self {
                <Self as TransparentWrapper<Entity>>::wrap_ref(entity)
            }
        }

        $(
            impl Borrow<$ext_ty> for $bundle_name {
                fn borrow(&self) -> &$ext_ty {
                    <$ext_ty as ComponentBundle>::force_cast_ref(self.raw_ref())
                }
            }
        )*

        impl Borrow<Entity> for $bundle_name {
            fn borrow(&self) -> &Entity {
                <Self as TransparentWrapper<Entity>>::peel_ref(self)
            }
        }

        impl Deref for $bundle_name {
            type Target = BundleMethods;

            fn deref(&self) -> &BundleMethods {
                <BundleMethods as TransparentWrapper<Entity>>::wrap_ref(Borrow::borrow(self))
            }
        }

        #[derive(TransparentWrapper)]
        #[repr(transparent)]
       pub struct BundleMethods(Entity);

        impl BundleMethods {
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

        impl Destructible for $bundle_name {
            fn destruct(self) {
                <Self as TransparentWrapper<Entity>>::peel(self).destruct();
            }
        }

        component_bundle!($($rest)*);
    }
}
