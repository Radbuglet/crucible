use std::{borrow::BorrowMut, mem};

use super::smuggle_drop;

// If `split_remainder` is none, the `target` string is split up as follows:
//
// ```
// [finished text][write gap][remainder]
//                ^ `finished`
//                           ^ `remainder`
// ```
//
// ...otherwise, this string is just a regular accumulator and `finished` loses all function.
#[repr(transparent)]
pub struct Splicer<T: Copy, V: BorrowMut<Vec<T>>>(SplicerInner<T, V>);

struct SplicerInner<T: Copy, V: BorrowMut<Vec<T>>> {
    target: V,
    split_remainder: Option<Box<[T]>>,
    finished: usize,
    remainder: usize,
}

impl<T: Copy, V: BorrowMut<Vec<T>>> Splicer<T, V> {
    pub fn new(target: V) -> Self {
        Self(SplicerInner {
            target,
            split_remainder: None,
            finished: 0,
            remainder: 0,
        })
    }

    pub fn remaining(&self) -> &[T] {
        &self.0.target.borrow()[self.0.remainder..]
    }

    pub fn splice(&mut self, offset: usize, len: usize, with: &[T]) {
        let me = &mut self.0;
        let target = me.target.borrow_mut();

        // Downgrade to split remainders if we can't handle the request directly.
        if me.split_remainder.is_none() && me.finished + with.len() > me.remainder + offset + len {
            me.split_remainder = Some(Box::from_iter(target[me.remainder..].iter().copied()));
            me.remainder = 0;
        }

        // Process `split_remainder` special-case.
        if let Some(split_remainder) = &me.split_remainder {
            #[rustfmt::skip]
            target.extend_from_slice(&split_remainder[me.remainder..][..offset]);
            target.extend_from_slice(with);
            me.remainder += offset + len;
            return;
        }

        // Otherwise, handle the operation directly.
        #[rustfmt::skip]
        target.copy_within(me.remainder..(me.remainder + offset), me.finished);
        me.finished += offset;
        target[me.finished..][..with.len()].copy_from_slice(with);
        me.finished += with.len();
        me.remainder += offset + len;
    }

    pub fn finish(mut self) -> V {
        self.finish_state();
        smuggle_drop(self, |v| &v.0).target
    }

    fn finish_state(&mut self) {
        let me = &mut self.0;
        let target = me.target.borrow_mut();

        if let Some(split_remainder) = &me.split_remainder {
            target.extend_from_slice(split_remainder);
        } else {
            target.copy_within(me.remainder.., me.finished);
            #[rustfmt::skip]
            target.truncate(target.len() - (me.remainder - me.finished));
        }
    }
}

impl<T: Copy, V: BorrowMut<Vec<T>>> Drop for Splicer<T, V> {
    fn drop(&mut self) {
        self.finish_state();
    }
}

pub struct StrSplicer<'a> {
    target: &'a mut String,
    splicer: Splicer<u8, Vec<u8>>,
}

impl<'a> StrSplicer<'a> {
    pub fn new(target: &'a mut String) -> Self {
        let splicer = Splicer::new(mem::take(target).into_bytes());
        Self { target, splicer }
    }

    pub fn remaining(&self) -> &[u8] {
        self.splicer.remaining()
    }

    pub fn splice(&mut self, offset: usize, len: usize, with: &[u8]) {
        self.splicer.splice(offset, len, with);
    }
}

impl<'a> Drop for StrSplicer<'a> {
    fn drop(&mut self) {
        let bytes = mem::replace(&mut self.splicer, Splicer::new(Vec::new())).finish();
        *self.target = String::from_utf8(bytes).unwrap();
    }
}
