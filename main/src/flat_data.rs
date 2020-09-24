pub trait LoadExt<'a> {
    fn load<T: FlatDataImpl>(&mut self) -> T;
}

pub trait StoreExt<'a> {
    fn load<T: FlatDataImplMut>(&'a mut self) -> T;
    fn store<T: FlatDataImplMut>(&'a mut self, value: T);
}

impl<'a> LoadExt<'a> for &'a [u8] {
    fn load<T: FlatDataImpl>(&mut self) -> T {
        let (raw, rest) = self.split_at(T::SIZE);
        *self = rest;
        T::load(raw)
    }
}

impl<'a> StoreExt<'a> for &'a mut [u8] {
    fn load<T: FlatDataImplMut>(&'a mut self) -> T {
        let (raw, rest) = self.split_at_mut(T::SIZE);
        *self = rest;
        T::load(raw)
    }

    fn store<T: FlatDataImplMut>(&'a mut self, value: T) {
        let (raw, rest) = self.split_at_mut(T::SIZE);
        *self = rest;
        T::store(value, raw)
    }
}

impl<'a> StoreExt<'a> for Vec<u8> {
    fn load<T: FlatDataImplMut>(&'a mut self) -> T {
        let len = self.len();
        self.resize_with(self.len() + T::SIZE, Default::default);
        let raw = &mut self[len..];
        T::load(raw)
    }

    fn store<T: FlatDataImplMut>(&'a mut self, value: T) {
        let len = self.len();
        self.resize_with(self.len() + T::SIZE, Default::default);
        let raw = &mut self[len..];
        T::store(value, raw)
    }
}

pub unsafe trait FlatDataImpl {
    const SIZE: usize;
    fn load(src: &[u8]) -> Self;
}

pub unsafe trait FlatDataImplMut {
    const SIZE: usize;
    fn load(src: &mut [u8]) -> Self;
    fn store(self, dst: &mut [u8]);
}

macro_rules! flat_data {
    ( $ty:ty ) => {
        unsafe impl crate::flat_data::FlatDataImpl for $ty {
            const SIZE: usize = std::mem::size_of::<$ty>();

            fn load(src: &[u8]) -> Self {
                assert!(src.len() >= std::mem::size_of::<$ty>());
                unsafe { *(src as *const _ as *const $ty) }
            }
        }

        unsafe impl crate::flat_data::FlatDataImplMut for $ty {
            const SIZE: usize = std::mem::size_of::<$ty>();

            fn load(src: &mut [u8]) -> Self {
                assert!(src.len() >= std::mem::size_of::<$ty>());
                unsafe { *(src as *const _ as *const $ty) }
            }

            fn store(self, dst: &mut [u8]) {
                assert!(dst.len() >= std::mem::size_of::<$ty>());
                unsafe { *(dst as *mut _ as *mut $ty) = self }
            }
        }

        unsafe impl crate::flat_data::FlatDataImpl for &$ty {
            const SIZE: usize = std::mem::size_of::<$ty>();

            fn load(src: &[u8]) -> Self {
                assert!(src.len() >= std::mem::size_of::<$ty>());
                unsafe { &*(src as *const _ as *const $ty) }
            }
        }

        unsafe impl crate::flat_data::FlatDataImplMut for &$ty {
            const SIZE: usize = std::mem::size_of::<$ty>();

            fn load(src: &mut [u8]) -> Self {
                assert!(src.len() >= std::mem::size_of::<$ty>());
                unsafe { &*(src as *const _ as *const $ty) }
            }

            fn store(self, dst: &mut [u8]) {
                assert!(dst.len() >= std::mem::size_of::<$ty>());
                unsafe { *(dst as *mut _ as *mut $ty) = *self }
            }
        }

        unsafe impl crate::flat_data::FlatDataImplMut for &mut $ty {
            const SIZE: usize = std::mem::size_of::<$ty>();

            fn load(src: &mut [u8]) -> Self {
                assert!(src.len() >= std::mem::size_of::<$ty>());
                unsafe { &mut *(src as *mut _ as *mut $ty) }
            }

            fn store(self, dst: &mut [u8]) {
                assert!(dst.len() >= std::mem::size_of::<$ty>());
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
            pub fn get(self) -> $num {
                self.raw.to_le()
            }

            pub fn set(&mut self, value: $num) {
                self.raw = value.to_le()
            }
        }

        flat_data!($bename);
        #[repr(C)]
        #[derive(Copy, Clone, Default)]
        pub struct $bename {
            raw: $num,
        }

        impl $bename {
            pub fn get(self) -> $num {
                self.raw.to_be()
            }

            pub fn set(&mut self, value: $num) {
                self.raw = value.to_be()
            }
        }
    };
}

pub use num::*;
pub mod num {
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

macro_rules! flat_data_array {
    ( $len:expr ) => {
        unsafe impl<T: Copy + Clone + FlatDataImpl> crate::flat_data::FlatDataImpl for [T; $len] {
            const SIZE: usize = std::mem::size_of::<[T; $len]>();

            fn load(src: &[u8]) -> Self {
                assert!(src.len() >= std::mem::size_of::<[T; $len]>());
                unsafe { *(src as *const _ as *const [T; $len]) }
            }
        }

        unsafe impl<T: Copy + Clone + FlatDataImplMut> crate::flat_data::FlatDataImplMut
            for [T; $len]
        {
            const SIZE: usize = std::mem::size_of::<[T; $len]>();

            fn load(src: &mut [u8]) -> Self {
                assert!(src.len() >= std::mem::size_of::<[T; $len]>());
                unsafe { *(src as *const _ as *const [T; $len]) }
            }

            fn store(self, dst: &mut [u8]) {
                assert!(dst.len() >= std::mem::size_of::<[T; $len]>());
                unsafe { *(dst as *mut _ as *mut [T; $len]) = self }
            }
        }

        unsafe impl<T: Copy + Clone + FlatDataImpl> crate::flat_data::FlatDataImpl for &[T; $len] {
            const SIZE: usize = std::mem::size_of::<[T; $len]>();

            fn load(src: &[u8]) -> Self {
                assert!(src.len() >= std::mem::size_of::<[T; $len]>());
                unsafe { &*(src as *const _ as *const [T; $len]) }
            }
        }

        unsafe impl<T: Copy + Clone + FlatDataImplMut> crate::flat_data::FlatDataImplMut
            for &[T; $len]
        {
            const SIZE: usize = std::mem::size_of::<[T; $len]>();

            fn load(src: &mut [u8]) -> Self {
                assert!(src.len() >= std::mem::size_of::<[T; $len]>());
                unsafe { &*(src as *const _ as *const [T; $len]) }
            }

            fn store(self, dst: &mut [u8]) {
                assert!(dst.len() >= std::mem::size_of::<[T; $len]>());
                unsafe { *(dst as *mut _ as *mut [T; $len]) = *self }
            }
        }

        unsafe impl<T: Copy + Clone + FlatDataImplMut> crate::flat_data::FlatDataImplMut
            for &mut [T; $len]
        {
            const SIZE: usize = std::mem::size_of::<[T; $len]>();

            fn load(src: &mut [u8]) -> Self {
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
