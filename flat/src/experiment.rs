use crate as flat;

pub unsafe trait Flat {
    const SIZE: usize;

    fn from(src: &[u8]) -> &Self;
    fn from_mut(src: &mut [u8]) -> &mut Self;
}

#[macro_export]
macro_rules! flat {
    ( #[repr(C, packed)] $vis:vis struct $name:ident { $( $fieldvis:vis $fieldname:ident: $ty:ty ),* } ) => {
        paste::paste! {
            #[repr(C, packed)]
            $vis struct $name {
                raw: [u8]
            }

            unsafe impl flat::Flat for $name {
                const SIZE: usize = 0 $( + (<$ty>::SIZE) )*;

                fn from(src: &[u8]) -> &Self {
                    assert!(src.len() == Self::SIZE);
                    unsafe { std::mem::transmute(src) }
                }

                fn from_mut(src: &mut [u8]) -> &mut Self {
                    assert!(src.len() == Self::SIZE);
                    unsafe { std::mem::transmute(src) }
                }
            }

            trait [< $name Ref >]<'a> {
                $( $fieldvis fn $fieldname(self) -> &'a $ty; )*
            }

            trait [< $name Mut >]<'a> {
                $( $fieldvis fn $fieldname(self) -> &'a mut $ty; )*
            }

            impl<'a> [< $name Ref >]<'a> for &'a $name {
                $( $fieldvis fn $fieldname(self) -> &'a $ty { From::from(&self.raw[0..8]) } )*
            }

            impl<'a> [< $name Mut >]<'a> for &'a mut $name {
                $( $fieldvis fn $fieldname(self) -> &'a mut $ty { From::from(&mut self.raw[0..8]) } )*
            }
        }
    };
    ( #[repr(C, packed)] struct $name:ident { $( $field:ident: $ty:ty ),* , } ) => {
        flat!( #[repr(C, packed)] struct $name { $( $field: $ty ),* } );
    };
}

macro_rules! num_impl {
    ( $num:ty, $lename:ident, $bename:ident ) => {
        #[repr(C, packed)]
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

        unsafe impl flat::Flat for $lename {
            const SIZE: usize = std::mem::size_of::<$num>();

            fn from(src: &[u8]) -> &Self {
                assert!(src.len() == Self::SIZE);
                unsafe { std::mem::transmute(src.as_ptr()) }
            }

            fn from_mut(src: &mut [u8]) -> &mut Self {
                assert!(src.len() == Self::SIZE);
                unsafe { std::mem::transmute(src.as_ptr()) }
            }
        }

        #[repr(C, packed)]
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

        unsafe impl flat::Flat for $bename {
            const SIZE: usize = std::mem::size_of::<$num>();

            fn from(src: &[u8]) -> &Self {
                assert!(src.len() == Self::SIZE);
                unsafe { std::mem::transmute(src.as_ptr()) }
            }

            fn from_mut(src: &mut [u8]) -> &mut Self {
                assert!(src.len() == Self::SIZE);
                unsafe { std::mem::transmute(src.as_ptr()) }
            }
        }
    };
}

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

macro_rules! flat_data_array {
    ( $len:expr ) => {
        unsafe impl<T: Flat> flat::Flat for [T; $len] {
            const SIZE: usize = T::SIZE * $len;

            fn from(src: &[u8]) -> &Self {
                assert!(src.len() == Self::SIZE);
                unsafe { std::mem::transmute(src.as_ptr()) }
            }

            fn from_mut(src: &mut [u8]) -> &mut Self {
                assert!(src.len() == Self::SIZE);
                unsafe { std::mem::transmute(src.as_ptr()) }
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
