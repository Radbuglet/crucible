use crate::exec::obj::read::RawObj;
use crate::exec::obj::ProviderOut;
use std::fmt::{Debug, Formatter};

pub struct ObjCx<'borrow, 'obj, O: ?Sized = dyn RawObj> {
	backing: ObjCxBacking<'borrow, 'obj, O>,
	length: usize,
}

enum ObjCxBacking<'borrow, 'obj, O: ?Sized> {
	Root(Vec<&'obj O>),
	Child(&'borrow mut Vec<&'obj O>),
}

impl<'borrow, 'obj, O: ?Sized + Debug> Debug for ObjCx<'borrow, 'obj, O> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("ObjCx")
			.field("children", &self.path())
			.finish_non_exhaustive()
	}
}

impl<'borrow, 'obj, O: ?Sized> ObjCx<'borrow, 'obj, O> {
	pub fn with_root(root: &'obj O) -> Self {
		Self {
			backing: ObjCxBacking::Root(vec![root]),
			length: 1,
		}
	}

	fn backing_ref(&self) -> &Vec<&'obj O> {
		match &self.backing {
			ObjCxBacking::Root(root) => root,
			ObjCxBacking::Child(root) => *root,
		}
	}

	pub fn path(&self) -> &[&'obj O] {
		&self.backing_ref()[0..self.length]
	}

	pub fn ancestors(&self, include_self: bool) -> impl Iterator<Item = &'obj O> + '_ {
		let path = self.path();
		let path = if include_self {
			path
		} else {
			&path[0..path.len()]
		};

		path.iter().copied().rev()
	}

	pub fn me(&self) -> &'obj O {
		self.path().last().unwrap()
	}

	pub fn with<'new_borrow>(&'new_borrow mut self, child: &'obj O) -> ObjCx<'new_borrow, 'obj, O> {
		let root = match &mut self.backing {
			ObjCxBacking::Root(root) => root,
			ObjCxBacking::Child(root) => *root,
		};

		root.truncate(self.length);
		root.push(child);

		ObjCx {
			backing: ObjCxBacking::Child(root),
			length: self.length + 1,
		}
	}

	pub fn owned(&self) -> ObjCx<'static, 'obj, O> {
		ObjCx {
			backing: ObjCxBacking::Root(self.backing_ref().clone()),
			length: self.length,
		}
	}
}

impl<'borrow, 'obj, O: ?Sized + RawObj> RawObj for ObjCx<'borrow, 'obj, O> {
	fn provide_raw<'r>(&'r self, out: &mut ProviderOut<'r>) {
		for ancestor in self.ancestors(true) {
			ancestor.provide_raw(out);
			if out.did_provide() {
				return;
			}
		}
	}
}
