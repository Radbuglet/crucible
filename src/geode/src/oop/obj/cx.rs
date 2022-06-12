use crate::util::iter_ext::DebugListIter;
use std::fmt::{Debug, Formatter};

use super::raw::ProviderOut;
use super::raw::RawObj;

pub type ObjCx<'chain, 'obj> = AncestryChain<'chain, 'obj, dyn Send + Sync + RawObj>;
pub type StObjCx<'chain, 'obj> = AncestryChain<'chain, 'obj, dyn RawObj>;
pub type SendObjCx<'chain, 'obj> = AncestryChain<'chain, 'obj, dyn Send + RawObj>;

#[derive(Clone)]
pub struct AncestryChain<'chain, 'obj, O: ?Sized> {
	pub node: &'obj O,
	pub parent: Option<&'chain AncestryChain<'chain, 'obj, O>>,
}

impl<'chain, 'obj, O: ?Sized + Debug> Debug for AncestryChain<'chain, 'obj, O> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("ObjCx")
			.field("ancestors", &DebugListIter::new(self.ancestors()))
			.finish()
	}
}

impl<'chain, 'obj, O: ?Sized> AncestryChain<'chain, 'obj, O> {
	pub fn new(node: &'obj O) -> Self {
		Self { node, parent: None }
	}

	pub fn with(&self, node: &'obj O) -> AncestryChain<O> {
		AncestryChain {
			node,
			parent: Some(self),
		}
	}

	pub fn ancestors<'a>(&'a self) -> ObjCxAncestryIter<'a, 'obj, O> {
		ObjCxAncestryIter { next: Some(self) }
	}
}

impl<O: ?Sized + RawObj> RawObj for AncestryChain<'_, '_, O> {
	fn provide_raw<'t, 'r>(&'r self, out: &mut ProviderOut<'t, 'r>) {
		for ancestor in self.ancestors() {
			if out.did_provide() {
				return;
			}
			ancestor.provide_raw(out);
		}
	}
}

pub struct ObjCxAncestryIter<'chain, 'obj, O: ?Sized> {
	pub next: Option<&'chain AncestryChain<'chain, 'obj, O>>,
}

impl<'chain, 'obj, O: ?Sized> Iterator for ObjCxAncestryIter<'chain, 'obj, O> {
	type Item = &'obj O;

	fn next(&mut self) -> Option<Self::Item> {
		let next = self.next?;
		self.next = next.parent;
		Some(next.node)
	}
}
