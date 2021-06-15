#![allow(incomplete_features)]
#![feature(const_type_id)]
#![feature(decl_macro)]
#![feature(specialization)]

pub mod core;
pub mod engine;
pub mod game;

fn main() {
    game::OApplication::start();
}
