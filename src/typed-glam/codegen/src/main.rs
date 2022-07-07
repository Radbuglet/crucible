mod util;
mod vec_derive;

fn main() {
	use clipboard::*;

	let mut clip = ClipboardContext::new().unwrap();
	let vec_derives = vec_derive::derive_entry_all().to_file_string().unwrap();

	clip.set_contents(vec_derives).unwrap();
	println!("Done.");
}
