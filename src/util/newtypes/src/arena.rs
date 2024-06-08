use derive_where::derive_where;
use std::{
    fmt, iter,
    marker::PhantomData,
    mem::MaybeUninit,
    num::NonZeroU32,
    ops::{Index, IndexMut},
    slice,
};

#[derive_where(Default)]
pub struct Arena<T> {
    // Invariant: this vector can never be u32::MAX elements in length.
    slots: Vec<ArenaSlot<T>>,
    free_slots: Vec<u32>,
}

struct ArenaSlot<T> {
    // Invariant: Even values indicate a vacant cell. Odd values indicate an occupied cell.
    gen: u32,
    value: MaybeUninit<T>,
}

impl<T> Drop for ArenaSlot<T> {
    fn drop(&mut self) {
        if self.gen % 2 == 1 {
            unsafe { self.value.assume_init_drop() };
        }
    }
}

impl<T> Arena<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, index: Handle<T>) -> Option<&T> {
        self.slots
            .get(index.index as usize)
            .filter(|v| v.gen == index.gen.get())
            .map(|v| unsafe {
                // Safety: `v.gen` must be odd by invariant and odd values denote occupied cells.
                v.value.assume_init_ref()
            })
    }

    pub fn get_mut(&mut self, index: Handle<T>) -> Option<&mut T> {
        self.slots
            .get_mut(index.index as usize)
            .filter(|slot| slot.gen == index.gen.get())
            .map(|slot| unsafe {
                // Safety: `index.gen` must be odd by invariant and odd values denote occupied cells.
                slot.value.assume_init_mut()
            })
    }

    pub fn get_many_mut<const N: usize>(&mut self, indices: [Handle<T>; N]) -> [&mut T; N] {
        fn illegal_set<T>(indices: &[Handle<T>]) -> ! {
            panic!("invalid duplicate or dead entry in `get_many_mut` index list: {indices:?}");
        }

        // Ensure that there are no overlaps in the set.
        for i in 0..N {
            let i_val = indices[i].index;

            for j_val in &indices[(i + 1)..N] {
                if i_val == j_val.index {
                    illegal_set(&indices);
                }
            }
        }

        // Fetch the entire set.
        let slots = self.slots.as_mut_ptr().cast::<ArenaSlot<T>>();

        indices.map(|index| {
            let slot = unsafe {
                // Safety: we already ensured that indices are distinct.
                &mut *slots.add(index.index as usize)
            };

            if slot.gen != index.gen.get() {
                illegal_set(&indices);
            }

            unsafe {
                // Safety: `slot.gen` must be odd by invariant and odd values denote occupied cells.
                slot.value.assume_init_mut()
            }
        })
    }

    pub fn insert(&mut self, value: T) -> Handle<T> {
        if let Some(index) = self.free_slots.pop() {
            let slot = &mut self.slots[index as usize];

            // This cannot overflow because the value is even and `u32::MAX` is odd.
            slot.gen += 1;

            slot.value = MaybeUninit::new(value);

            Handle {
                _ty: PhantomData,
                index,
                gen: unsafe {
                    // Safety: zero is not even.
                    NonZeroU32::new_unchecked(slot.gen)
                },
            }
        } else {
            let index = u32::try_from(self.slots.len()).expect("too many slots");

            self.slots.push(ArenaSlot {
                gen: 1,
                value: MaybeUninit::new(value),
            });

            Handle {
                _ty: PhantomData,
                index,
                gen: NonZeroU32::new(1).unwrap(),
            }
        }
    }

    pub fn remove(&mut self, index: Handle<T>) -> Option<T> {
        let slot = self.slots.get_mut(index.index as usize)?;

        if slot.gen != index.gen.get() {
            return None;
        }

        let value = unsafe {
            // Safety: `slot.gen` must be odd by invariant and odd values denote occupied cells.
            slot.value.assume_init_read()
        };

        slot.gen = slot.gen.wrapping_add(1);

        if slot.gen > 0 {
            self.free_slots.push(index.index);
        }

        Some(value)
    }

    pub fn len(&self) -> usize {
        self.slots.len() - self.free_slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn clear(&mut self) {
        self.slots.clear();
        self.free_slots.clear();
    }

    pub fn iter(&self) -> ArenaIter<'_, T> {
        ArenaIter {
            slots: self.slots.iter().enumerate(),
        }
    }

    pub fn iter_mut(&mut self) -> ArenaIterMut<'_, T> {
        ArenaIterMut {
            slots: self.slots.iter_mut().enumerate(),
        }
    }
}

impl<T: fmt::Debug> fmt::Debug for Arena<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut map = f.debug_map();

        for (key, value) in self {
            map.entry(&key, value);
        }

        map.finish()
    }
}

impl<T> Index<Handle<T>> for Arena<T> {
    type Output = T;

    fn index(&self, index: Handle<T>) -> &Self::Output {
        self.get(index)
            .unwrap_or_else(|| panic!("storage does not contain {index:?}"))
    }
}

impl<T> IndexMut<Handle<T>> for Arena<T> {
    fn index_mut(&mut self, index: Handle<T>) -> &mut Self::Output {
        self.get_mut(index)
            .unwrap_or_else(|| panic!("storage does not contain {index:?}"))
    }
}

impl<'a, T> IntoIterator for &'a Arena<T> {
    type Item = (Handle<T>, &'a T);
    type IntoIter = ArenaIter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T> IntoIterator for &'a mut Arena<T> {
    type Item = (Handle<T>, &'a mut T);
    type IntoIter = ArenaIterMut<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

#[derive_where(Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct Handle<T> {
    // Allows `T` to be up-casted w.r.t lifetime variance.
    _ty: PhantomData<fn() -> T>,

    // The index of this object's slot.
    index: u32,

    // Invariant: this value must be odd!
    gen: NonZeroU32,
}

impl<T> fmt::Debug for Handle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} @ {}", self.index, self.gen)
    }
}

impl<T> Handle<T> {
    pub fn cast<V>(self) -> Handle<V> {
        Handle {
            _ty: PhantomData,
            index: self.index,
            gen: self.gen,
        }
    }
}

#[derive_where(Clone)]
pub struct ArenaIter<'a, T> {
    slots: iter::Enumerate<slice::Iter<'a, ArenaSlot<T>>>,
}

impl<'a, T> Iterator for ArenaIter<'a, T> {
    type Item = (Handle<T>, &'a T);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (index, slot) = self.slots.next()?;
            let index = index as u32;

            if slot.gen % 2 == 0 {
                continue;
            }

            let obj = Handle {
                _ty: PhantomData,
                index,
                gen: unsafe {
                    // Safety: value is odd
                    NonZeroU32::new_unchecked(slot.gen)
                },
            };

            let value = unsafe {
                // Safety: value is odd
                slot.value.assume_init_ref()
            };

            return Some((obj, value));
        }
    }
}

pub struct ArenaIterMut<'a, T> {
    slots: iter::Enumerate<slice::IterMut<'a, ArenaSlot<T>>>,
}

impl<'a, T> Iterator for ArenaIterMut<'a, T> {
    type Item = (Handle<T>, &'a mut T);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (index, slot) = self.slots.next()?;
            let index = index as u32;

            if slot.gen % 2 == 0 {
                continue;
            }

            let obj = Handle {
                _ty: PhantomData,
                index,
                gen: unsafe {
                    // Safety: value is odd
                    NonZeroU32::new_unchecked(slot.gen)
                },
            };

            let value = unsafe {
                // Safety: value is odd
                slot.value.assume_init_mut()
            };

            return Some((obj, value));
        }
    }
}
