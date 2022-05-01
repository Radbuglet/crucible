use crate::exec::key::TypedKey;
use crate::exec::obj::obj::Obj;
use crate::exec::obj::read::{ComponentMissingError, ObjFlavor, ObjRead, SendSyncFlavor};
use std::fmt::{Debug, Formatter};
use std::ptr::NonNull;

pub struct ObjCx<'borrow, 'obj, F: ObjFlavor = SendSyncFlavor> {
	backing: ObjCxBacking<'borrow, 'obj, F>,
	length: usize,
}

enum ObjCxBacking<'borrow, 'obj, F: ObjFlavor> {
	Root(Vec<&'obj Obj<F>>),
	Child(&'borrow mut Vec<&'obj Obj<F>>),
}

impl<'borrow, 'obj, F: ObjFlavor> Debug for ObjCx<'borrow, 'obj, F> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("ObjCx")
			.field("children", &self.path())
			.finish_non_exhaustive()
	}
}

impl<'obj, F: ObjFlavor> ObjCx<'_, 'obj, F> {
	pub fn root(root: &'obj Obj<F>) -> Self {
		Self {
			backing: ObjCxBacking::Root(vec![root]),
			length: 1,
		}
	}

	fn backing_ref(&self) -> &Vec<&'obj Obj<F>> {
		match &self.backing {
			ObjCxBacking::Root(root) => root,
			ObjCxBacking::Child(root) => *root,
		}
	}

	pub fn path(&self) -> &[&'obj Obj<F>] {
		&self.backing_ref()[0..self.length]
	}

	pub fn ancestors(&self, include_self: bool) -> impl Iterator<Item = &'obj Obj<F>> + '_ {
		let path = self.path();
		let path = if include_self {
			path
		} else {
			&path[0..path.len()]
		};

		path.iter().copied().rev()
	}

	pub fn me(&self) -> &'obj Obj<F> {
		self.path().last().unwrap()
	}

	pub fn with<'borrow>(&'borrow mut self, child: &'obj Obj<F>) -> ObjCx<'borrow, 'obj, F> {
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

	pub fn owned(&self) -> ObjCx<'static, 'obj, F> {
		ObjCx {
			backing: ObjCxBacking::Root(self.backing_ref().clone()),
			length: self.length,
		}
	}
}

unsafe impl<F: ObjFlavor> ObjRead for ObjCx<'_, '_, F> {
	type AccessFlavor = F;

	fn try_get_raw<T: ?Sized + 'static>(
		&self,
		key: TypedKey<T>,
	) -> Result<NonNull<T>, ComponentMissingError> {
		for ancestor in self.ancestors(true) {
			if let Ok(value) = ancestor.try_get_raw(key) {
				return Ok(value);
			}
		}

		Err(ComponentMissingError { key: key.raw() })
	}
}
