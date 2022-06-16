use super::bump::Bump;
use std::alloc::Layout;
use std::cell::{Ref, RefCell, RefMut, UnsafeCell};
use std::hash::Hash;
use std::marker::{PhantomData, Unsize};
use std::num::NonZeroU64;
use std::ptr::Pointee;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::time::Duration;

// === Book-keeping === //

struct ObjectDB {
    heap: Bump,
    slots: Bump,
    free_locks: AtomicU64,
    free_sessions: AtomicU64,
}

static OBJECT_DB: ObjectDB = {
    let layout = Layout::new::<[u64; 256]>();
    ObjectDB {
        heap: Bump::new(layout),
        slots: Bump::new(layout),
        free_locks: AtomicU64::new(0),
        free_sessions: AtomicU64::new(0),
    }
};

struct ObjSlot {
    base_ptr: *mut (),
    gen_and_lock: u64,
}

fn acquire_bit(locks: &AtomicU64) -> Option<u8> {
    todo!()
}

// === Garbage collection & sessions === //

pub fn collect_garbage(settings: &GcSettings) {
    todo!()
}

#[derive(Debug, Clone)]
pub struct GcSettings {
    max_time: Option<Duration>,
}

#[derive(Debug)]
pub struct Session<'a> {
    // Behaves like an `UnsafeCell` in that we can Send it but not share it across threads.
    _ty: PhantomData<&'a UnsafeCell<()>>,

    // The `id` of our session.
    id: u8,

    // A boolean array of the locks this session has acquired.
    lock_masks: [u8; 64],
}

impl<'a> Session<'a> {
    pub fn acquire<I>(locks: I) -> Self
    where
        I: IntoIterator<Item = &'a mut LockToken>,
    {
        let id = acquire_bit(&OBJECT_DB.free_sessions)
            .expect("more than 64 concurrent sessions created (how many cores do you have?)");

        let mut lock_masks = [0x0; 64];

        for lock in locks {
            lock_masks[lock.handle().0 as usize] = 0xFF;
        }

        Self {
            _ty: PhantomData,
            id,
            lock_masks,
        }
    }
}

// === Locks === //

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct Lock(
    // The ID of our lock. This is actually 22 bits long.
    u8,
);

#[derive(Debug)]
pub struct LockToken(
    // This is a wrapper around our `Lock` that ensures that it is unique.
    Lock,
);

impl Default for LockToken {
    fn default() -> Self {
        let id = acquire_bit(&OBJECT_DB.free_locks).expect("more than 64 concurrent locks created");
        let lock = Lock(id);

        LockToken(lock)
    }
}

impl LockToken {
    pub fn new() -> (Self, Lock) {
        let token = Self::default();
        let handle = token.handle();
        (token, handle)
    }

    pub fn handle(&self) -> Lock {
        self.0
    }
}

// === Obj === //

pub type ObjRw<T> = Obj<RefCell<T>>;

pub struct Obj<T: 'static + ?Sized + Send> {
    slot: &'static ObjSlot,
    gen: NonZeroU64,
    meta: <T as Pointee>::Metadata,
}

impl<T: 'static + Send + Sync> Obj<T> {
    pub fn new(session: &Session, value: T) -> Self {
        todo!()
    }
}

impl<T: 'static + Send> Obj<T> {
    pub fn new_locked(session: &Session, lock: Lock, value: T) -> Self {
        todo!()
    }
}

impl<T: 'static + ?Sized + Send> Obj<T> {
    pub fn is_locked(&self, session: &Session) -> bool {
        todo!()
    }

    pub fn is_alive(&self, session: &Session) -> bool {
        todo!()
    }

    pub fn has_exclusive_access(&self, session: &Session) -> bool {
        todo!()
    }

    pub fn get<'a>(&self, session: &'a Session) -> &'a T {
        todo!()
    }

    pub fn unsize<U: 'static + ?Sized + Send>(&self, session: &Session) -> Obj<U>
    where
        T: Unsize<U>,
    {
        todo!()
    }
}

impl<T: 'static + Send> Obj<RefCell<T>> {
    pub fn new_rw(session: &Session, lock: Lock, value: T) -> Self {
        todo!()
    }
}

impl<T: 'static + ?Sized + Send> Obj<RefCell<T>> {
    pub fn try_borrow<'a>(&self, session: &'a Session) -> Option<Ref<'a, T>> {
        todo!()
    }

    pub fn try_borrow_mut<'a>(&self, session: &'a Session) -> Option<RefMut<'a, T>> {
        todo!()
    }

    pub fn borrow<'a>(&self, session: &'a Session) -> Ref<'a, T> {
        todo!()
    }

    pub fn borrow_mut<'a>(&self, session: &'a Session) -> RefMut<'a, T> {
        todo!()
    }
}

unsafe impl<T: 'static + Send> Send for Obj<T> {}
unsafe impl<T: 'static + Send> Sync for Obj<T> {}

impl<T: 'static + ?Sized + Send> Copy for Obj<T> {}

impl<T: 'static + ?Sized + Send> Clone for Obj<T> {
    fn clone(&self) -> Self {
        Self {
            slot: self.slot,
            gen: self.gen,
            meta: self.meta,
        }
    }
}

impl<T: 'static + ?Sized + Send> Hash for Obj<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.gen.get());
    }
}

impl<T: 'static + ?Sized + Send> Eq for Obj<T> {}

impl<T: 'static + ?Sized + Send> PartialEq for Obj<T> {
    fn eq(&self, other: &Self) -> bool {
        self.gen == other.gen
    }
}

pub trait ObjCtorExt: 'static + Sized + Send {
    fn as_obj(self, session: &Session) -> Obj<Self>
    where
        Self: Sync,
    {
        Obj::new(session, self)
    }

    fn as_obj_locked(self, session: &Session, lock: Lock) -> Obj<Self> {
        Obj::new_locked(session, lock, self)
    }

    fn as_obj_rw(self, session: &Session, lock: Lock) -> ObjRw<Self> {
        Obj::new_rw(session, lock, self)
    }
}

impl<T: 'static + Send> ObjCtorExt for T {}

pub mod prelude {
    pub use super::{collect_garbage, GcSettings, Lock, LockToken, Obj, ObjCtorExt, Session};
}
