pub use file_name::FileName;
pub mod file_name {
    use std::ffi::OsString;

    use super::super::FileReference;
    use flat::prelude::*;

    pub struct FileName<'a> {
        pub raw: &'a [u16],
        pub header: &'a FileNameHeader,
    }

    impl<'a> FileName<'a> {
        pub fn parse(mut raw: &'a [u8]) -> FileName<'a> {
            let header: &FileNameHeader = raw.load();
            assert_eq!(raw.len() / 2, header.len as usize);
            let name = raw.load_slice(header.len as usize);

            FileName { raw: name, header }
        }

        pub fn os_string(&self) -> OsString {
            std::os::windows::ffi::OsStringExt::from_wide(self.raw)
        }
    }

    flat_data!(FileNameHeader);
    #[repr(C, packed)]
    #[derive(Copy, Clone, Debug)]
    pub struct FileNameHeader {
        pub parent_reference: FileReference,
        pub c_time: u64le,
        pub a_time: u64le,
        pub m_time: u64le,
        pub r_time: u64le,
        pub allocated_size: u64le,
        pub real_size: u64le,
        pub flags: u32le,
        pub _unk: u32le,
        pub len: u8,
        pub namespace: u8,
    }
}


pub use attribute_list::AttributeListEntry;
pub mod attribute_list {
    use flat::prelude::*;
    use super::super::FileReference;

    flat_data!(AttributeListEntry);
    #[repr(C, packed)]
    #[derive(Copy, Clone, Debug)]
    pub struct AttributeListEntry {
        pub ty: u32le,
        pub len: u16le,
        pub name_len: u8,
        pub name_offset: u8,
        pub starting_vcn: u64le,
        pub base_file_reference: FileReference,
        pub attribute_id: u16le,
    }
}
