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
	session::{LocalSessionGuard, Session},
};

use super::{
	entity::{ComponentList, Entity},
	key::TypedKey,
};

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

	fn late_cast(entity: Entity) -> Self;

	// === Derived casting methods === //

	fn try_cast_owned(session: Session, entity: Owned<Entity>) -> anyhow::Result<Owned<Self>> {
		entity.try_map(|entity| Self::try_cast(session, entity))
	}

	fn force_cast_owned(entity: Owned<Entity>) -> Owned<Self> {
		entity.map(|entity| Self::late_cast(entity))
	}

	fn cast_owned(entity: Owned<Entity>) -> Owned<Self> {
		entity.map(|entity| Self::cast(entity))
	}

	fn can_cast(session: Session, entity: Entity) -> bool {
		Self::try_cast(session, entity).is_ok()
	}

	fn cast(entity: Entity) -> Self {
		#[cfg(debug_assertions)]
		{
			use crucible_core::error::{AnyhowConvertExt, ErrorFormatExt};

			if let Err(err) =
				Self::try_cast(LocalSessionGuard::new().handle(), entity).into_std_error()
			{
				err.raise();
			}
		}
		Self::late_cast(entity)
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
		let bundled = Self::late_cast(entity);

		Owned::new(bundled)
	}

	fn add_onto(session: Session, entity: Entity, components: Self::CompList) -> Self {
		entity.add(session, components);
		Self::late_cast(entity)
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
		self.map(|bundle| bundle.raw())
	}
}

// TODO: Deref integration

// === `EntityWith` === //

pub type EntityWithRw<T> = EntityWith<RefCell<T>>;

pub struct EntityWith<T: ?Sized + ObjPointee> {
	_ty: PhantomInvariant<T>,
	entity: Entity,
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

impl<T: ?Sized + ObjPointee> Destructible for EntityWith<T> {
	fn destruct(self) {
		self.entity.destruct();
	}
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
		Ok(Self::late_cast(entity))
	}

	fn late_cast(entity: Entity) -> Self {
		Self {
			_ty: PhantomData,
			entity,
		}
	}
}

impl<T: ?Sized + ObjPointee> ComponentBundleWithCtor for EntityWith<T> {
	type CompList = Option<MaybeOwned<Obj<T>>>;
}

impl<T: ?Sized + ObjPointee> EntityWith<T> {
	pub fn comp<'a>(&self, session: Session<'a>) -> &'a T {
		self.entity.get::<T>(session)
	}
}

impl<T: ?Sized + ObjPointee> EntityWithRw<T> {
	pub fn borrow_comp(self, session: Session) -> Ref<T> {
		self.entity.borrow::<T>(session)
	}

	pub fn borrow_comp_mut(self, session: Session) -> RefMut<T> {
		self.entity.borrow_mut::<T>(session)
	}
}

impl<T: ?Sized + ObjPointee> Owned<EntityWith<T>> {
	pub fn comp<'a>(&self, session: Session<'a>) -> &'a T {
		self.weak_copy().comp(session)
	}
}

impl<T: ?Sized + ObjPointee> Owned<EntityWithRw<T>> {
	pub fn borrow_comp<'s>(&self, session: Session<'s>) -> Ref<'s, T> {
		self.weak_copy().borrow_comp(session)
	}

	pub fn borrow_comp_mut<'s>(&self, session: Session<'s>) -> RefMut<'s, T> {
		self.weak_copy().borrow_comp_mut(session)
	}
}

impl<T: ?Sized + ObjPointee> MaybeOwned<EntityWith<T>> {
	pub fn comp<'a>(&self, session: Session<'a>) -> &'a T {
		self.weak_copy().comp(session)
	}
}

impl<T: ?Sized + ObjPointee> MaybeOwned<EntityWithRw<T>> {
	pub fn borrow_comp<'s>(&self, session: Session<'s>) -> Ref<'s, T> {
		self.weak_copy().borrow_comp(session)
	}

	pub fn borrow_comp_mut<'s>(&self, session: Session<'s>) -> RefMut<'s, T> {
		self.weak_copy().borrow_comp_mut(session)
	}
}

// === `CachedEntityWith` === //

pub type CachedEntityWithRw<T> = CachedEntityWith<RefCell<T>>;

pub struct CachedEntityWith<T: ?Sized + ObjPointee> {
	entity: Entity,
	cache: Option<Obj<T>>,
}

impl<T: ?Sized + ObjPointee> fmt::Debug for CachedEntityWith<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("CachedEntityWith")
			.field("entity", &self.entity)
			.finish_non_exhaustive()
	}
}

impl<T: ?Sized + ObjPointee> Copy for CachedEntityWith<T> {}

impl<T: ?Sized + ObjPointee> Clone for CachedEntityWith<T> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<T: ?Sized + ObjPointee> hash::Hash for CachedEntityWith<T> {
	fn hash<H: hash::Hasher>(&self, state: &mut H) {
		self.entity.hash(state);
	}
}

impl<T: ?Sized + ObjPointee> Eq for CachedEntityWith<T> {}

impl<T: ?Sized + ObjPointee> PartialEq for CachedEntityWith<T> {
	fn eq(&self, other: &Self) -> bool {
		self.entity == other.entity
	}
}

impl<T: ?Sized + ObjPointee> Borrow<Entity> for CachedEntityWith<T> {
	fn borrow(&self) -> &Entity {
		&self.entity
	}
}

impl<T: ?Sized + ObjPointee> Destructible for CachedEntityWith<T> {
	fn destruct(self) {
		self.entity.destruct();
	}
}

impl<T: ?Sized + ObjPointee> ComponentBundle for CachedEntityWith<T> {
	fn try_cast(session: Session, entity: Entity) -> anyhow::Result<Self> {
		match entity.fallible_get_obj::<T>(session) {
			Ok(obj) => Ok(Self {
				entity,
				cache: Some(obj),
			}),
			Err(err) => {
				if err.as_permission_error().is_none() {
					Err(anyhow::Error::new(err).context(format!(
						"failed to construct `CachedEntityWith<{}>` component bundle",
						std::any::type_name::<T>()
					)))
				} else {
					Ok(Self {
						entity,
						cache: None,
					})
				}
			}
		}
	}

	fn late_cast(entity: Entity) -> Self {
		Self {
			entity,
			cache: None,
		}
	}
}

impl<T: ?Sized + ObjPointee> ComponentBundleWithCtor for CachedEntityWith<T> {
	type CompList = Option<MaybeOwned<Obj<T>>>;
}

impl<T: ?Sized + ObjPointee> CachedEntityWith<T> {
	pub fn invalidate_cache(&mut self) {
		self.cache = None;
	}

	pub fn comp<'s>(&mut self, session: Session<'s>) -> &'s T {
		let obj = self
			.cache
			.get_or_insert_with(|| self.entity.get_obj::<T>(session));

		obj.get(session)
	}
}

impl<T: ?Sized + ObjPointee> CachedEntityWithRw<T> {
	pub fn borrow_comp<'s>(&mut self, session: Session<'s>) -> Ref<'s, T> {
		self.comp(session).borrow()
	}

	pub fn borrow_comp_mut<'s>(&mut self, session: Session<'s>) -> RefMut<'s, T> {
		self.comp(session).borrow_mut()
	}
}

impl<T: ?Sized + ObjPointee> Owned<CachedEntityWith<T>> {
	pub fn comp<'s>(&mut self, session: Session<'s>) -> &'s T {
		self.weak_mut().comp(session)
	}
}

impl<T: ?Sized + ObjPointee> Owned<CachedEntityWithRw<T>> {
	pub fn borrow_comp<'s>(&mut self, session: Session<'s>) -> Ref<'s, T> {
		self.weak_mut().borrow_comp(session)
	}

	pub fn borrow_comp_mut<'s>(&mut self, session: Session<'s>) -> RefMut<'s, T> {
		self.weak_mut().borrow_comp_mut(session)
	}
}

impl<T: ?Sized + ObjPointee> MaybeOwned<CachedEntityWith<T>> {
	pub fn comp<'a>(&mut self, session: Session<'a>) -> &'a T {
		self.weak_mut().comp(session)
	}
}

impl<T: ?Sized + ObjPointee> MaybeOwned<CachedEntityWithRw<T>> {
	pub fn borrow_comp<'s>(&mut self, session: Session<'s>) -> Ref<'s, T> {
		self.weak_mut().borrow_comp(session)
	}

	pub fn borrow_comp_mut<'s>(&mut self, session: Session<'s>) -> RefMut<'s, T> {
		self.weak_mut().borrow_comp_mut(session)
	}
}

// === `CompEntity` === //

pub trait ObjBackref: ObjPointee {
	fn entity(&self) -> Entity;
}

pub struct BackrefEntityWith<T: ?Sized + ObjBackref> {
	obj: Obj<T>,
}

impl<T: ?Sized + ObjBackref> fmt::Debug for BackrefEntityWith<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let session = LocalSessionGuard::new();
		let s = session.handle();

		f.debug_struct("BackrefEntityWith")
			.field("entity", &self.entity(s))
			.finish()
	}
}

impl<T: ?Sized + ObjBackref> Copy for BackrefEntityWith<T> {}

impl<T: ?Sized + ObjBackref> Clone for BackrefEntityWith<T> {
	fn clone(&self) -> Self {
		*self
	}
}

// N.B. we don't derive comparison operations because it's unclear how these should be compared.
// e.g. should they be compared like all the other `EntityWith` variants (i.e. by the entity) or
// compared by their component instance. The former would incur a performance penalty from the
// construction of `LocalSessionGuards`.

impl<T: ?Sized + ObjBackref> Destructible for BackrefEntityWith<T> {
	fn destruct(self) {
		let session = LocalSessionGuard::new();
		let s = session.handle();

		if let Ok(obj) = self.obj.weak_get(s) {
			obj.entity().destroy(s);
		}
	}
}

impl<T: ?Sized + ObjBackref> BackrefEntityWith<T> {
	pub fn from_comp(obj: Obj<T>) -> Self {
		Self { obj }
	}

	pub fn from_entity(session: Session, entity: Entity) -> Self {
		Self {
			obj: entity.get_obj::<T>(session),
		}
	}

	pub fn from_entity_in(session: Session, entity: Entity, key: TypedKey<T>) -> Self {
		Self {
			obj: entity.get_obj_in::<T>(session, key),
		}
	}

	pub fn entity(&self, session: Session) -> Entity {
		self.obj.get(session).entity()
	}

	pub fn comp_obj(&self) -> Obj<T> {
		self.obj
	}

	pub fn comp<'s>(&self, session: Session<'s>) -> &'s T {
		self.obj.get(session)
	}
}

impl<T: ?Sized + ObjBackref> Owned<BackrefEntityWith<T>> {
	pub fn from_comp(obj: Owned<Obj<T>>) -> Self {
		obj.map(|obj| BackrefEntityWith::from_comp(obj))
	}

	pub fn from_entity(session: Session, entity: Owned<Entity>) -> Self {
		entity.map(|entity| BackrefEntityWith::from_entity(session, entity))
	}

	pub fn from_entity_in(session: Session, entity: Owned<Entity>, key: TypedKey<T>) -> Self {
		entity.map(|entity| BackrefEntityWith::from_entity_in(session, entity, key))
	}

	pub fn entity(&self, session: Session) -> Entity {
		self.weak_copy().entity(session)
	}

	pub fn comp_obj(&self) -> Obj<T> {
		self.weak_copy().comp_obj()
	}

	pub fn comp<'s>(&self, session: Session<'s>) -> &'s T {
		self.weak_copy().comp(session)
	}
}

impl<T: ?Sized + ObjBackref> MaybeOwned<BackrefEntityWith<T>> {
	pub fn from_comp(obj: MaybeOwned<Obj<T>>) -> Self {
		obj.map(|obj| BackrefEntityWith::from_comp(obj))
	}

	pub fn from_entity(session: Session, entity: MaybeOwned<Entity>) -> Self {
		entity.map(|entity| BackrefEntityWith::from_entity(session, entity))
	}

	pub fn from_entity_in(session: Session, entity: MaybeOwned<Entity>, key: TypedKey<T>) -> Self {
		entity.map(|entity| BackrefEntityWith::from_entity_in(session, entity, key))
	}

	pub fn entity(&self, session: Session) -> Entity {
		self.weak_copy().entity(session)
	}

	pub fn comp_obj(&self) -> Obj<T> {
		self.weak_copy().comp_obj()
	}

	pub fn comp<'s>(&self, session: Session<'s>) -> &'s T {
		self.weak_copy().comp(session)
	}
}

// === `component_bundle` === //

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
                Ok(Self::late_cast(entity))
            }

            fn late_cast(entity: Entity) -> Self {
                <Self as TransparentWrapper<Entity>>::wrap(entity)
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
