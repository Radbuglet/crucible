fn main() {
    dbg!(naga::front::wgsl::parse_str(include_str!("demo.wgsl")));
}
