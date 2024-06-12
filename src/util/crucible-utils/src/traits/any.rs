use std::any::Any;

pub trait AnyLike: Any {
    fn as_any(me: &Self) -> &dyn Any;

    fn as_any_mut(me: &mut Self) -> &mut dyn Any;

    fn downcast_ref<T: 'static>(me: &Self) -> Option<&T> {
        Self::as_any(me).downcast_ref()
    }

    fn downcast_mut<T: 'static>(me: &mut Self) -> Option<&mut T> {
        Self::as_any_mut(me).downcast_mut()
    }
}

impl AnyLike for dyn Any {
    fn as_any(me: &Self) -> &dyn Any {
        me
    }

    fn as_any_mut(me: &mut Self) -> &mut dyn Any {
        me
    }
}

impl AnyLike for dyn Any + Send {
    fn as_any(me: &Self) -> &dyn Any {
        me
    }

    fn as_any_mut(me: &mut Self) -> &mut dyn Any {
        me
    }
}

impl AnyLike for dyn Any + Sync {
    fn as_any(me: &Self) -> &dyn Any {
        me
    }

    fn as_any_mut(me: &mut Self) -> &mut dyn Any {
        me
    }
}

impl AnyLike for dyn Any + Send + Sync {
    fn as_any(me: &Self) -> &dyn Any {
        me
    }

    fn as_any_mut(me: &mut Self) -> &mut dyn Any {
        me
    }
}
