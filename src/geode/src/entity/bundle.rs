use crate::core::{
	owned::{Destructible, Owned},
	session::{LocalSessionGuard, Session},
};

use super::entity::ComponentList;
#[allow(unused)] // Actually captured by the macro
use super::{
	entity::{ComponentAttachTarget, Entity, OwnedOrWeak},
	key::typed_key,
};
#[allow(unused)]
use crucible_core::macros::prefer_left;

pub trait ComponentBundle: Sized + Destructible {
	type CompList: ComponentList;

	// === Required methods === //

	fn try_cast(session: Session, entity: Entity) -> anyhow::Result<Self>;

	fn force_cast(entity: Entity) -> Self;

	// === Derived casting methods === //

	fn can_cast(session: Session, entity: Entity) -> bool {
		Self::try_cast(session, entity).is_ok()
	}

	fn unchecked_cast(entity: Entity) -> Self {
		debug_assert!(Self::can_cast(LocalSessionGuard::new().handle(), entity));
		Self::force_cast(entity)
	}

	// === Entity constructors === //

	fn spawn(session: Session, components: Self::CompList) -> Owned<Self> {
		let entity = Entity::new_with(session, components).manually_destruct();
		let bundled = Self::unchecked_cast(entity);

		Owned::new(bundled)
	}

	fn add_onto(session: Session, entity: Entity, components: Self::CompList) -> Self {
		entity.add(session, components);
		Self::unchecked_cast(entity)
	}

	// === Deconstructors === //

	fn raw(self) -> Entity;
}

pub macro component_bundle($(
    $vis:vis struct $bundle_name:ident($bundle_ctor_name:ident) {
        $(..$ext_ty:ty;)*

        $(
            $field_name:ident$([$key:expr])?: $field_ty:ty
        ),*
        $(,)?
    }
)*) {$(
    #[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
    $vis struct $bundle_name(Entity);

    #[derive(Debug)]
    $vis struct $bundle_ctor_name {
        $(pub $field_name: OwnedOrWeak<$field_ty>,)*
    }

    impl ComponentList for $bundle_ctor_name {
        fn push_values(self, registry: &mut ComponentAttachTarget) {
            $(ComponentList::push_values(self.$field_name, registry);)*
        }
    }

    impl ComponentBundle for $bundle_name {
        type CompList = $bundle_ctor_name;

        fn try_cast(session: Session, entity: Entity) -> anyhow::Result<Self> {
            $(
                if let Err(err) = entity.falliable_get_in(session, prefer_left!(
                    $({$key})?
                    { typed_key::<$field_ty>() }
                )) {
                    if err.as_permission_error().is_none() {
                        return Err(anyhow::Error::new(err)
                            .context(concat!(
                                "failed to construct ",
                                stringify!($bundle_name),
                                " component bundle"
                            ))
                        );
                    }
                }
            )*
            Ok(Self::force_cast(entity))
        }

        fn force_cast(entity: Entity) -> Self {
            Self(entity)
        }

        fn raw(self) -> Entity {
            self.0
        }
    }

    impl $bundle_name {
        $(
            pub fn $field_name<'a>(self, session: Session<'a>) -> &'a $field_ty {
                self.raw().get_in(session, prefer_left!(
                    $({$key})?
                    { typed_key::<$field_ty>() }
                ))
            }
        )*
    }

    impl Destructible for $bundle_name {
        fn destruct(self) {
            self.0.destruct();
        }
    }
)*}
