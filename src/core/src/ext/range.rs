use core::ops::Bound;
use std::ops::RangeBounds;

pub type AnyRange<T> = (Bound<T>, Bound<T>);

pub fn as_any_range_cloned<T: Clone, R: RangeBounds<T>>(range: &R) -> AnyRange<T> {
	clone_any_range(&as_any_range(range))
}

pub fn as_any_range<T, R: RangeBounds<T>>(range: &R) -> AnyRange<&T> {
	(range.start_bound(), range.end_bound())
}

pub fn clone_any_range<T: Clone>(range: &AnyRange<&T>) -> AnyRange<T> {
	(range.start_bound().cloned(), range.end_bound().cloned())
}

pub fn bound_as_ref<T>(bound: &Bound<T>) -> Bound<&T> {
	match bound {
		Bound::Included(bound) => Bound::Included(bound),
		Bound::Excluded(bound) => Bound::Excluded(bound),
		Bound::Unbounded => Bound::Unbounded,
	}
}

pub fn unwrap_or_unbounded<T>(bound: Bound<T>) -> Option<T> {
	match bound {
		Bound::Included(val) => Some(val),
		Bound::Excluded(val) => Some(val),
		Bound::Unbounded => None,
	}
}
