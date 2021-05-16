#![feature(const_type_id)]
#![feature(decl_macro)]

pub mod core;
pub mod engine;
pub mod game;

fn main() {
    game::OApplication::start();
}
