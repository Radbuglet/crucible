// use crate::{core::session::Session, RawObj};
//
// use super::key::RawTypedKey;
//
// pub struct Entity {}
//
// impl Entity {
// 	pub fn add<L: ComponentList>(&self, session: &Session, components: L) {}
// }
//
// pub struct ComponentAttachTarget<'a> {
// 	slots: &'a mut [Option<RawObj>],
// 	archetype: (),
// }
//
// pub trait ComponentList: Sized {
// 	type KeyList: IntoIterator<Item = RawTypedKey> + Clone;
//
// 	fn keys(&self) -> Self::KeyList;
// 	fn push_values(self, registry: &mut ComponentAttachTarget);
// }
//
// impl<T: ?Sized> ComponentList for Obj<T> {}
