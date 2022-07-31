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
		entity::{ComponentAttachTarget, EntityGetError, SingleComponent},
		key::typed_key,
	},
	crate::core::{obj::Obj, owned::MaybeOwned},
	anyhow,
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

// === `component_bundle` ctor helpers === //

// `MandatoryComp`
pub enum MandatoryBundleComp<T: ?Sized + ObjPointee> {
	Present(MaybeOwned<Obj<T>>),
	Late,
}

impl<T: ?Sized + ObjPointee, S: Into<MaybeOwned<Obj<T>>>> From<S> for MandatoryBundleComp<T> {
	fn from(val: S) -> Self {
		Self::Present(val.into())
	}
}

impl<T: ?Sized + ObjPointee> SingleComponent for MandatoryBundleComp<T> {
	type Value = T;

	fn push_value_under(self, registry: &mut ComponentAttachTarget, key: TypedKey<Self::Value>) {
		match self {
			Self::Present(comp) => comp.push_value_under(registry, key),
			Self::Late => {}
		}
	}
}

// === `component_bundle` === //

pub macro component_bundle ($(
	$vis:vis struct $bundle_name:ident
		$(<
			$($para_name:ident),*
			$(,)?
		>)?
	$(($bundle_ctor_name:ident))?
	{
		$($inner:tt)*
	}
)*) {$(
	// === $bundle_name definition === //

	#[repr(transparent)]
	$vis struct $bundle_name$(<$($para_name: ?Sized + ObjPointee),*>)? {
		$(_invariant: PhantomData<fn($($para_name),*)>,)?
		// Seriously, don't name this field in the macro. `decl_macro` hygiene is far from finished.
		do_not_name_this_field_hygiene_is_jank: Entity,
	}

	// We have to derive this manually because `bytemuck`'s derive macro assumes that `bytemuck`
	// is in the caller's module prelude.
	unsafe impl
		$(<$($para_name: ?Sized + ObjPointee),*>)?
		TransparentWrapper<Entity> for $bundle_name$(<$($para_name),*>)?
	{
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

	// === `ComponentBundle` implementation === //

	impl$(<$($para_name: ?Sized + ObjPointee),*>)? ComponentBundle for $bundle_name$(<$($para_name),*>)? {
		#[allow(unused)]  // `session` and `BUNDLE_MAKE_ERR` may be unused in empty bundles.
		fn try_cast(session: Session, entity: Entity) -> anyhow::Result<Self> {
			const BUNDLE_MAKE_ERR: &'static str = concat!(
				"failed to construct ",
				stringify!($bundle_name),
				" component bundle"
			);

			component_bundle_derive_cast!(
				BUNDLE_MAKE_ERR;
				session;
				entity;
				{ $($inner)* };
			);

			Ok(Self::late_cast(entity))
		}

		fn late_cast(entity: Entity) -> Self {
			<Self as TransparentWrapper<Entity>>::wrap(entity)
		}
	}

	impl$(<$($para_name: ?Sized + ObjPointee),*>)? $bundle_name$(<$($para_name),*>)? {
		component_bundle_derive_getters!($($inner)*);
	}

	component_bundle_try_derive_ctor!(
		{$vis};
		$bundle_name;
		$($bundle_ctor_name)?;
		[$($($para_name),*)?];
		{,$($inner)*,};
	);
)*}

#[allow(unused)]
macro component_bundle_derive_cast {
	// Muncher base case
	(
		$bundle_make_err:ident;
		$session:ident;
		$entity:ident;
		{ $(,)? };
	) => {},
	// Extension
	(
		$bundle_make_err:ident;
		$session:ident;
		$entity:ident;
		{
			..$ext_name:ident: $ext_ty:ty
			$(, $($rest:tt)*)?
		};
	) => {
		<$ext_ty as ComponentBundle>::try_cast($session, $entity)?;

		component_bundle_derive_cast!(
			$bundle_make_err;
			$session;
			$entity;

			{$($($rest)*)?};
		);
	},
	// Field
	(
		$bundle_make_err:ident;
		$session:ident;
		$entity:ident;
		{
			$field_name:ident$([$field_key:expr])? : $field_ty:ty
			$(, $($rest:tt)*)?
		};
	) => {
		if let Err(err) = $entity.fallible_get_in($session, prefer_left!(
			$({$field_key})?
			{ typed_key::<$field_ty>() }
		)) {
			if err.as_permission_error().is_none() {
				return Err(anyhow::Error::new(err).context($bundle_make_err));
			}
		}

		component_bundle_derive_cast!(
			$bundle_make_err;
			$session;
			$entity;

			{$($($rest)*)?};
		);
	},
	// Optional field
	(
		$bundle_make_err:ident;
		$session:ident;
		$entity:ident;
		{
			$field_name:ident$([$field_key:expr])? ?: $field_ty:ty
			$(, $($rest:tt)*)?
		};
	) => {
		component_bundle_derive_cast!(
			$bundle_make_err;
			$session;
			$entity;

			{$($($rest)*)?};
		);
	},
}

#[allow(unused)]
macro component_bundle_derive_getters {
	// Muncher base case
	($(,)?) => {},
	// Extension
	(
		..$ext_name:ident: $ext_ty:ty
		$(, $($rest:tt)*)?
	) => {
		pub fn $ext_name(&self) -> $ext_ty {
			<$ext_ty as ComponentBundle>::late_cast(self.raw())
		}

		component_bundle_derive_getters!($($($rest)*)?);
	},
	// Field
	(
		$field_name:ident$([$field_key:expr])? : $field_ty:ty
		$(, $($rest:tt)*)?
	) => {
		pub fn $field_name<'s>(&self, session: Session<'s>) -> &'s $field_ty {
			<Self as TransparentWrapper<Entity>>::peel_ref(self).get_in(session, prefer_left!(
				$({$field_key})?
				{ typed_key::<$field_ty>() }
			))
		}

		component_bundle_derive_getters!($($($rest)*)?);
	},
	// Optional field
	(
		$field_name:ident$([$field_key:expr])? ?: $field_ty:ty
		$(, $($rest:tt)*)?
	) => {
		pub fn $field_name<'s>(&self, session: Session<'s>) -> Result<&'s $field_ty, EntityGetError> {
			<Self as TransparentWrapper<Entity>>::peel_ref(self).fallible_get_in(session, prefer_left!(
				$({$field_key})?
				{ typed_key::<$field_ty>() }
			))
		}

		component_bundle_derive_getters!($($($rest)*)?);
	},
}

#[allow(unused)]
macro component_bundle_try_derive_ctor {
	// No specified constructor
	(
		// Bundle vis
		{$vis:vis};

		// Bundle name
		$bundle_name:ident;

		// Bundle ctor name
		;

		// Generic parameters
		[$($para_name:ident),*];

		// Struct definition
		{$($ignored:tt)*};
	) => {},
	// Specified constructor
	(
		// Bundle vis
		{$vis:vis};

		// Bundle name
		$bundle_name:ident;

		// Bundle ctor name
		$bundle_ctor_name:ident;

		// Generic parameters
		[$($para_name:ident),*];

		// Struct definition
		{
			$(
				$(..$ext_name:ident: $ext_ty:ty)?
				,  // <-- Janky hack
				$(
					$field_name:ident
					$([$field_key:expr])?

					$(?: $field_ty_optional:ty)?
					$(: $field_ty:ty)?
				)?
			)*
		};
	) => {
		$vis struct $bundle_ctor_name<$($para_name: ?Sized + ObjPointee),*> {$(
			$(pub $ext_name: <$ext_ty as ComponentBundle>::CompList,)?

			// TODO: Differentiate between an intention uninit and an omission if possible.
			$(
				pub $field_name:
					$(MandatoryBundleComp<$field_ty>)?
					$(Option<MaybeOwned<Obj<$field_ty_optional>>>)?,
			)?
		)*}

		impl<$($para_name: ?Sized + ObjPointee),*> ComponentList for $bundle_ctor_name<$($para_name),*> {
			#[allow(unused)]  // `registry` may go unused in empty bundles.
			fn push_values(self, registry: &mut ComponentAttachTarget) {
				$(
					$(ComponentList::push_values(self.$ext_name, registry);)?
					$(SingleComponent::push_value_under(self.$field_name, registry, prefer_left!(
						$({$field_key})? {typed_key()}
					));)?
				)*
			}
		}

		impl<$($para_name: ?Sized + ObjPointee),*> ComponentBundleWithCtor for $bundle_name<$($para_name),*> {
			type CompList = $bundle_ctor_name<$($para_name),*>;
		}
	}
}
