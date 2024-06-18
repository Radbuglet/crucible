mod dummy_game;
mod main_loop;
mod render;

fn main() {
    // Install early (infallible) services
    color_backtrace::install();
    tracing_subscriber::fmt::init();

    tracing::info!("Hello!");

    // Run main (fallible) app logic
    if let Err(err) = main_loop::main_inner() {
        tracing::error!("Fatal error ocurred during engine startup:\n{err:?}");
        std::process::exit(1);
    }

    tracing::info!("Goodbye!");
}
