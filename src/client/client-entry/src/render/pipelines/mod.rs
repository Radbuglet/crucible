macro_rules! include_shader {
    ($name:expr) => {
        include_str!(concat!(env!("OUT_DIR"), "/shaders/", $name))
    };
}

pub mod actor;
pub mod skybox;
pub mod voxel;
