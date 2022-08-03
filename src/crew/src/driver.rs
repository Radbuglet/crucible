use crate::tokenizer::{
	file::{FileDescBundle, FileDescBundleCtor, LoadedFile, SourceFileInfo},
	generic::ForkableCursor,
};
use geode::prelude::*;

pub fn entry() {
	let main_lock = Lock::new(NoLabel);

	let session = LocalSessionGuard::new();
	let s = session.handle();
	s.acquire_locks([main_lock.weak_copy()]);

	let file_desc = FileDescBundle::spawn(
		s,
		FileDescBundleCtor {
			info: SourceFileInfo {
				name: "testing file".to_string(),
			}
			.box_obj(s)
			.into(),
		},
	);

	let file = LoadedFile {
		file_desc: file_desc.weak_copy(),
		contents: "whee\rwoo\r\nwaz\n\rmaz".as_bytes().to_owned(),
	};

	let mut reader = file.reader();

	for (_, atom) in reader.drain() {
		dbg!(atom);
	}
}
