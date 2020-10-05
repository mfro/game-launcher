use crate as flat;

pub trait LoadExt<'a> {
    fn load<T: Flat>(&mut self) -> T;
    fn load_slice<T: Flat>(&mut self, len: usize) -> &'a [T];
}

pub trait StoreExt<'a> {
    fn load<T: FlatMut>(&'a mut self) -> T;
    fn store<T: FlatMut>(&'a mut self, value: T);
}

impl<'a> LoadExt<'a> for &'a [u8] {
    fn load<T: Flat>(&mut self) -> T {
        let (raw, rest) = self.split_at(T::SIZE);
        *self = rest;
        T::load(raw)
    }

    fn load_slice<T: Flat>(&mut self, len: usize) -> &'a [T] {
        let (raw, rest) = self.split_at(len * T::SIZE);
        *self = rest;
        unsafe { std::slice::from_raw_parts(raw.as_ptr() as *const _, len) }
    }
}

impl<'a> StoreExt<'a> for &'a mut [u8] {
    fn load<T: FlatMut>(&'a mut self) -> T {
        let (raw, rest) = self.split_at_mut(T::SIZE);
        *self = rest;
        T::load_mut(raw)
    }

    fn store<T: FlatMut>(&'a mut self, value: T) {
        let (raw, rest) = self.split_at_mut(T::SIZE);
        *self = rest;
        T::store(value, raw)
    }
}

impl<'a> StoreExt<'a> for Vec<u8> {
    fn load<T: FlatMut>(&'a mut self) -> T {
        let len = self.len();
        self.resize_with(self.len() + T::SIZE, Default::default);
        let raw = &mut self[len..];
        T::load_mut(raw)
    }

    fn store<T: FlatMut>(&'a mut self, value: T) {
        let len = self.len();
        self.resize_with(self.len() + T::SIZE, Default::default);
        let raw = &mut self[len..];
        T::store(value, raw)
    }
}

pub trait FlatBase {
    const SIZE: usize;
}

pub unsafe trait Flat: FlatBase {
    fn load(src: &[u8]) -> Self;
}

pub unsafe trait FlatMut: FlatBase {
    fn load_mut(src: &mut [u8]) -> Self;
    fn store(self, dst: &mut [u8]);
}

#[macro_export]
macro_rules! flat_data {
    ( $ty:ty ) => {
        impl flat::FlatBase for $ty {
            const SIZE: usize = std::mem::size_of::<$ty>();
        }

        unsafe impl flat::Flat for $ty {
            fn load(src: &[u8]) -> Self {
                assert!(src.len() >= <Self as flat::FlatBase>::SIZE);
                unsafe { *(src as *const _ as *const $ty) }
            }
        }

        unsafe impl flat::FlatMut for $ty {
            fn load_mut(src: &mut [u8]) -> Self {
                assert!(src.len() >= <Self as flat::FlatBase>::SIZE);
                unsafe { *(src as *const _ as *const $ty) }
            }

            fn store(self, dst: &mut [u8]) {
                assert!(dst.len() >= <Self as flat::FlatBase>::SIZE);
                unsafe { *(dst as *mut _ as *mut $ty) = self }
            }
        }

        impl flat::FlatBase for &$ty {
            const SIZE: usize = <$ty>::SIZE;
        }

        unsafe impl flat::Flat for &$ty {
            fn load(src: &[u8]) -> Self {
                assert!(src.len() >= <Self as flat::FlatBase>::SIZE);
                unsafe { &*(src as *const _ as *const $ty) }
            }
        }

        unsafe impl flat::FlatMut for &$ty {
            fn load_mut(src: &mut [u8]) -> Self {
                assert!(src.len() >= <Self as flat::FlatBase>::SIZE);
                unsafe { &*(src as *const _ as *const $ty) }
            }

            fn store(self, dst: &mut [u8]) {
                assert!(dst.len() >= <Self as flat::FlatBase>::SIZE);
                unsafe { *(dst as *mut _ as *mut $ty) = *self }
            }
        }

        impl flat::FlatBase for &mut $ty {
            const SIZE: usize = <$ty>::SIZE;
        }

        unsafe impl flat::FlatMut for &mut $ty {
            fn load_mut(src: &mut [u8]) -> Self {
                assert!(src.len() >= <Self as flat::FlatBase>::SIZE);
                unsafe { &mut *(src as *mut _ as *mut $ty) }
            }

            fn store(self, dst: &mut [u8]) {
                assert!(dst.len() >= <Self as flat::FlatBase>::SIZE);
                unsafe { *(dst as *mut _ as *mut $ty) = *self }
            }
        }
    };
}

macro_rules! num_impl {
    ( $num:ty, $lename:ident, $bename:ident ) => {
        flat_data!($num);

        flat_data!($lename);
        #[repr(C)]
        #[derive(Copy, Clone, Default)]
        pub struct $lename {
            raw: $num,
        }

        impl $lename {
            pub fn new(value: $num) -> Self {
                let raw = value.to_le();
                Self { raw }
            }

            pub fn get(self) -> $num {
                <$num>::from_le(self.raw)
            }

            pub fn set(&mut self, value: $num) {
                self.raw = value.to_le()
            }
        }

        impl std::fmt::Debug for $lename {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.get())
            }
        }

        flat_data!($bename);
        #[repr(C)]
        #[derive(Copy, Clone, Default)]
        pub struct $bename {
            raw: $num,
        }

        impl $bename {
            pub fn new(value: $num) -> Self {
                let raw = value.to_be();
                Self { raw }
            }

            pub fn get(self) -> $num {
                <$num>::from_be(self.raw)
            }

            pub fn set(&mut self, value: $num) {
                self.raw = value.to_be()
            }
        }

        impl std::fmt::Debug for $bename {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.get())
            }
        }
    };
}

macro_rules! flat_data_array {
    ( $len:expr ) => {
        impl<T: Copy + Clone> flat::FlatBase for [T; $len] {
            const SIZE: usize = std::mem::size_of::<[T; $len]>();
        }

        unsafe impl<T: Copy + Clone + Flat> flat::Flat for [T; $len] {
            fn load(src: &[u8]) -> Self {
                assert!(src.len() >= std::mem::size_of::<[T; $len]>());
                unsafe { *(src as *const _ as *const [T; $len]) }
            }
        }

        unsafe impl<T: Copy + Clone + FlatMut> flat::FlatMut for [T; $len] {
            fn load_mut(src: &mut [u8]) -> Self {
                assert!(src.len() >= std::mem::size_of::<[T; $len]>());
                unsafe { *(src as *const _ as *const [T; $len]) }
            }

            fn store(self, dst: &mut [u8]) {
                assert!(dst.len() >= std::mem::size_of::<[T; $len]>());
                unsafe { *(dst as *mut _ as *mut [T; $len]) = self }
            }
        }

        impl<T: Copy + Clone> flat::FlatBase for &[T; $len] {
            const SIZE: usize = <[T; $len]>::SIZE;
        }

        unsafe impl<T: Copy + Clone + Flat> flat::Flat for &[T; $len] {
            fn load(src: &[u8]) -> Self {
                assert!(src.len() >= std::mem::size_of::<[T; $len]>());
                unsafe { &*(src as *const _ as *const [T; $len]) }
            }
        }

        unsafe impl<T: Copy + Clone + FlatMut> flat::FlatMut for &[T; $len] {
            fn load_mut(src: &mut [u8]) -> Self {
                assert!(src.len() >= std::mem::size_of::<[T; $len]>());
                unsafe { &*(src as *const _ as *const [T; $len]) }
            }

            fn store(self, dst: &mut [u8]) {
                assert!(dst.len() >= std::mem::size_of::<[T; $len]>());
                unsafe { *(dst as *mut _ as *mut [T; $len]) = *self }
            }
        }

        impl<T: Copy + Clone> flat::FlatBase for &mut [T; $len] {
            const SIZE: usize = <[T; $len]>::SIZE;
        }

        unsafe impl<T: Copy + Clone + FlatMut> flat::FlatMut for &mut [T; $len] {
            fn load_mut(src: &mut [u8]) -> Self {
                assert!(src.len() >= std::mem::size_of::<[T; $len]>());
                unsafe { &mut *(src as *mut _ as *mut [T; $len]) }
            }

            fn store(self, dst: &mut [u8]) {
                assert!(dst.len() >= std::mem::size_of::<[T; $len]>());
                unsafe { *(dst as *mut _ as *mut [T; $len]) = *self }
            }
        }
    };
}

flat_data_array!(0);
flat_data_array!(1);
flat_data_array!(2);
flat_data_array!(3);
flat_data_array!(4);
flat_data_array!(5);
flat_data_array!(6);
flat_data_array!(7);
flat_data_array!(8);
flat_data_array!(9);

flat_data_array!(10);
flat_data_array!(11);
flat_data_array!(12);
flat_data_array!(13);
flat_data_array!(14);
flat_data_array!(15);
flat_data_array!(16);
flat_data_array!(17);
flat_data_array!(18);
flat_data_array!(19);

flat_data_array!(20);
flat_data_array!(21);
flat_data_array!(22);
flat_data_array!(23);
flat_data_array!(24);
flat_data_array!(25);
flat_data_array!(26);
flat_data_array!(27);
flat_data_array!(28);
flat_data_array!(29);

pub use prelude::*;
pub mod prelude {
    pub use crate::{self as flat, Flat, FlatBase, FlatMut, LoadExt, StoreExt};

    num_impl!(u8, u8le, u8be);
    num_impl!(u16, u16le, u16be);
    num_impl!(u32, u32le, u32be);
    num_impl!(u64, u64le, u64be);
    num_impl!(u128, u128le, u128be);
    num_impl!(usize, usizele, usizebe);

    num_impl!(i8, i8le, i8be);
    num_impl!(i16, i16le, i16be);
    num_impl!(i32, i32le, i32be);
    num_impl!(i64, i64le, i64be);
    num_impl!(i128, i128le, i128be);
    num_impl!(isize, isizele, isizebe);
}
