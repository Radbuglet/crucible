use crate::{
	tokenizer::{
		file::{FileDescBundle, FileDescBundleCtor, LoadedFile, SourceFileInfo},
		token_parser::tokenize,
	},
	util::intern::Interner,
};
use geode::prelude::*;

pub fn entry() {
	// Create Geode session
	let main_lock = Lock::new(NoLabel);

	let session = LocalSessionGuard::new();
	let s = session.handle();
	s.acquire_locks([main_lock.weak_copy()]);

	// Create file
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
		contents: include_bytes!("../samples/driver_example.crew").as_slice().to_owned(),
	};

	// Tokenize
	let mut interner = Interner::default();
	tokenize(&mut interner, &file);
	println!("Done!");
}
