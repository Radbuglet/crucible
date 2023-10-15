mod entry;
mod game;

fn main() {
	use crucible_util::debug::error::ErrorFormatExt;

	// Initialize the logger
	env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info,wgpu=warn"))
		.init();

	color_backtrace::install();
	log::info!("Hello!");

	// Delegate to the inner entry function
	if let Err(err) = entry::main_inner() {
		log::error!("Error during initialization: {}", err.format_error());
	}
}
