use bytemuck::{Pod, Zeroable};
use cgmath::{Matrix4, Vector3};
use std::ops::{Deref, DerefMut};

macro def_pod_adapters($($name:ident of $wrapped:tt),*) {$(
    unsafe impl<N: Zeroable> Zeroable for $name<N> {}
    unsafe impl<N: 'static + Copy + Pod> Pod for $name<N> {}

    impl<N> Deref for $name<N> {
        type Target = $wrapped<N>;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl<N> DerefMut for $name<N> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }
)*}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
#[repr(transparent)]
pub struct Vec3PodAdapter<N>(pub Vector3<N>);

#[derive(Debug, Copy, Clone, PartialEq)]
#[repr(transparent)]
pub struct Mat4PodAdapter<N>(pub Matrix4<N>);

def_pod_adapters!(Vec3PodAdapter of Vector3, Mat4PodAdapter of Matrix4);
