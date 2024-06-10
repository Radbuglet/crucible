use std::{any::Any, borrow::Borrow, fmt, hash, ops::Deref};

use newtypes::transparent;
use std_traits::AnyLike;

// === ReifiedKey === //

#[transparent(inner, wrap_raw)]
#[repr(transparent)]
pub struct ReifiedKey {
    inner: dyn ReifiedKeyInner,
}

trait ReifiedKeyInner {
    fn hash(&self) -> u64;

    fn owned_as_reified(&self) -> Option<&dyn Any>;

    fn borrowed_cmp_to_owned(&self, other: &dyn Any) -> bool;
}

impl Eq for ReifiedKey {}

impl PartialEq for ReifiedKey {
    fn eq(&self, other: &Self) -> bool {
        if let Some(reified) = self.inner.owned_as_reified() {
            other.inner.borrowed_cmp_to_owned(reified)
        } else {
            self.inner.borrowed_cmp_to_owned(
                other
                    .inner
                    .owned_as_reified()
                    .expect("at least one `ReifiedKey` in a pair must be owned"),
            )
        }
    }
}

impl hash::Hash for ReifiedKey {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.inner.hash());
    }
}

// === Borrowed ReifiedKey === //

#[transparent(inner, wrap_raw)]
#[repr(transparent)]
struct BorrowedReifiedKey<T> {
    inner: (u64, T),
}

impl<T: 'static + Eq> ReifiedKeyInner for BorrowedReifiedKey<T> {
    fn hash(&self) -> u64 {
        self.inner.0
    }

    fn owned_as_reified(&self) -> Option<&dyn Any> {
        None
    }

    fn borrowed_cmp_to_owned(&self, other: &dyn Any) -> bool {
        other
            .downcast_ref::<T>()
            .is_some_and(|other| &self.inner.1 == other)
    }
}

impl ReifiedKey {
    pub fn wrap_borrowed<T: 'static + Eq>(entry: &(u64, T)) -> &Self {
        ReifiedKey::wrap_raw_ref(BorrowedReifiedKey::wrap_raw_ref(entry))
    }
}

// === OwnedReifiedKey === //

pub struct OwnedReifiedKey<T> {
    pub hash: u64,
    pub key: T,
}

impl<T: fmt::Debug> fmt::Debug for OwnedReifiedKey<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.key.fmt(f)
    }
}

impl<T> hash::Hash for OwnedReifiedKey<T> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.hash);
    }
}

impl<T> ReifiedKeyInner for OwnedReifiedKey<T>
where
    T: Deref,
    T::Target: AnyLike,
{
    fn hash(&self) -> u64 {
        self.hash
    }

    fn owned_as_reified(&self) -> Option<&dyn Any> {
        Some(AnyLike::as_any(&*self.key))
    }

    fn borrowed_cmp_to_owned(&self, _other: &dyn Any) -> bool {
        panic!("at least one `BorrowedReifiedKey` in a pair must be borrowed");
    }
}

impl<T> Borrow<ReifiedKey> for OwnedReifiedKey<T>
where
    T: Deref,
    T::Target: AnyLike,
{
    fn borrow(&self) -> &ReifiedKey {
        ReifiedKey::wrap_raw_ref(self)
    }
}
