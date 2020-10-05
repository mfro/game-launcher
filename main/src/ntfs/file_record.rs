use flat::prelude::*;
use std::{
    ffi::OsString,
    io::{prelude::*, Error, ErrorKind, Result, SeekFrom},
};

use super::{file_name, FileName, NTFileSystem};

pub struct FileRecord<'a> {
    raw: &'a [u8],
    pub header: &'a FileRecordHeader,
}

impl<'a> FileRecord<'a> {
    pub fn parse(src: &mut &'a [u8]) -> FileRecord<'a> {
        let header: &FileRecordHeader = Flat::load(&src);
        let x = header.magic;
        assert_eq!(x, 0x454c4946);

        let raw = &src[0..header.real_size.get() as usize];
        *src = &src[header.allocated_size.get() as usize..];

        FileRecord { header, raw }
    }

    pub fn attributes(&self) -> AttributeIterator<'a> {
        let attrs_offset = self.header.attribute_array_offset.get() as usize;
        let attrs_end = self.header.real_size.get() as usize;
        let attrs = &self.raw[attrs_offset..attrs_end];

        AttributeIterator::new(attrs)
    }

    pub fn win32_file_name<F: Read + Seek>(
        &self,
        fs: &mut NTFileSystem<F>,
    ) -> Result<Option<(file_name::FileNameHeader, OsString)>> {
        for attr in self.attributes() {
            if attr.ty() != 0x30 {
                continue;
            }

            let mut content = vec![];
            fs.open(attr.content()).read_to_end(&mut content)?;

            let name = FileName::parse(&content);

            if name.header.namespace == 2 {
                continue;
            }

            return Ok(Some((name.header.clone(), name.os_string())));
        }

        Ok(None)
    }
}

flat_data!(FileRecordHeader);
#[repr(C, packed)]
#[derive(Copy, Clone, Debug)]
pub struct FileRecordHeader {
    pub magic: u32,
    pub update_sequence_offset: u16le,
    pub update_sequence_size: u16le,
    pub lsn: u64le,
    pub sequence_number: u16le,
    pub hard_link_count: u16le,
    pub attribute_array_offset: u16le,
    pub flags: u16le,
    pub real_size: u32le,
    pub allocated_size: u32le,
    pub base_record: FileReference,
    pub next_attribute_id: u16le,
    pub _padding: u16le,
    pub number: u32le,
}

pub struct AttributeIterator<'a> {
    data: &'a [u8],
}

impl AttributeIterator<'_> {
    pub fn new(data: &[u8]) -> AttributeIterator {
        AttributeIterator { data }
    }
}

impl<'a> Iterator for AttributeIterator<'a> {
    type Item = Attribute<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.data.len() > 8 {
            Some(Attribute::parse(&mut self.data))
        } else {
            None
        }
    }
}

pub struct Attribute<'a> {
    raw: &'a [u8],
    pub header: &'a AttributeHeader,
}

impl<'a> Attribute<'a> {
    pub fn parse(data: &mut &'a [u8]) -> Attribute<'a> {
        let header: &AttributeHeader = Flat::load(data);
        let len = header.len.get() as usize;

        let raw = &data[..len];
        *data = &data[len..];

        Attribute { raw, header }
    }

    pub fn ty(&self) -> u32 {
        self.header.ty.get()
    }

    pub fn name(&self) -> Option<std::result::Result<String, std::string::FromUtf16Error>> {
        match self.header.name_len {
            0 => None,
            l => {
                let o = self.header.name_offset.get() as usize;
                assert!(self.raw.len() >= o + 2 * l as usize);
                let mut src = &self.raw[o..o + 2 * l as usize];
                let chars = src.load_slice(l as usize);
                Some(String::from_utf16(chars))
            }
        }
    }

    pub fn content(&self) -> AttributeContent<'a> {
        match self.header.non_resident {
            0 => {
                let info: &ResidentHeader = Flat::load(&self.raw[AttributeHeader::SIZE..]);
                let o = info.value_offset.get() as usize;
                let l = info.value_len.get() as usize;
                AttributeContent::Resident(&self.raw[o..o + l])
            }
            1 => {
                let info: &NonResidentHeader = Flat::load(&self.raw[AttributeHeader::SIZE..]);
                let o = info.data_runs_offset.get() as usize;
                let raw = &self.raw[o..];

                // println!(
                //     "{} {} {} {} {} {} {} {}",
                //     info.first_vcn.get(),
                //     info.last_vcn.get(),
                //     info.data_runs_offset.get(),
                //     info.compression_unit_size.get(),
                //     info.allocated_size.get(),
                //     info.real_size.get(),
                //     info.initialized_size.get(),
                //     raw.len()
                // );

                // assert_eq!(info.first_vcn.get(), 0);

                AttributeContent::NonResident(
                    raw,
                    1 + info.last_vcn.get() as u64 - info.first_vcn.get() as u64,
                    info.real_size.get(),
                )
            }
            x => panic!("invalid residence state: {}", x),
        }
    }
}

flat_data!(AttributeHeader);
#[repr(C, packed)]
#[derive(Copy, Clone, Debug)]
pub struct AttributeHeader {
    pub ty: u32le,
    pub len: u16le,
    pub _unknown: u16le,
    pub non_resident: u8,
    pub name_len: u8,
    pub name_offset: u16le,
    pub flags: u16le,
    pub id: u16le,
}

flat_data!(ResidentHeader);
#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct ResidentHeader {
    pub value_len: u32le,
    pub value_offset: u16le,
    pub indexed: u8,
    pub _padding: u8,
}

flat_data!(NonResidentHeader);
#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct NonResidentHeader {
    pub first_vcn: u32le,
    pub _unk1: u32le,
    pub last_vcn: u32le,
    pub _unk2: u32le,
    pub data_runs_offset: u16le,
    pub compression_unit_size: u16le,
    pub _padding: u32le,
    pub allocated_size: u64le,
    pub real_size: u64le,
    pub initialized_size: u64le,
}

// enum AttributeValue<'a> {
//     StandardInformation(&'a StandardInformation),
//     FileName(&'a FileName, &'a [u8]),
// }

#[derive(Debug, Copy, Clone)]
pub enum AttributeContent<'a> {
    Resident(&'a [u8]),
    NonResident(&'a [u8], u64, u64),
}

// impl<'a> AttributeContent<'a> {
//     pub fn open<F: Read + Seek>(self, src: F, cluster_size: u64) -> AttributeReader<'a, F> {
//         match self {
//             Self::Resident(raw) => AttributeReader::Resident(ResidentReader {
//                 data: raw,
//                 offset: 0,
//             }),
//             Self::NonResident(runs, limit) => {
//                 let runs = DataRuns::parse(runs, cluster_size, limit);
//                 AttributeReader::NonResident(runs.open(src))
//             }
//         }
//     }

//     pub fn open_resident(self) -> Option<ResidentReader<'a>> {
//         match self {
//             Self::Resident(raw) => Some(ResidentReader {
//                 data: raw,
//                 offset: 0,
//             }),
//             Self::NonResident(..) => None,
//         }
//     }
// }

pub enum AttributeReader<'a, F> {
    Resident(ResidentReader<'a>),
    NonResident(DataRunReader<F>),
}

impl<'a, F: Read + Seek> Read for AttributeReader<'a, F> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        match self {
            Self::Resident(base) => base.read(buf),
            Self::NonResident(base) => base.read(buf),
        }
    }
}

impl<'a, F: Read + Seek> Seek for AttributeReader<'a, F> {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        match self {
            Self::Resident(base) => base.seek(pos),
            Self::NonResident(base) => base.seek(pos),
        }
    }
}

pub struct ResidentReader<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> ResidentReader<'a> {
    pub fn new(data: &'a [u8]) -> ResidentReader<'a> {
        let offset = 0;
        ResidentReader { data, offset }
    }
}

impl<'a> Read for ResidentReader<'a> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let mut x = &self.data[self.offset..];
        let count = x.read(buf)?;
        self.offset += count;
        Ok(count)
    }
}

impl<'a> Seek for ResidentReader<'a> {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        match pos {
            SeekFrom::Start(o) => self.offset = o as usize,
            SeekFrom::End(o) => self.offset = (self.data.len() as i64 + o) as usize,
            SeekFrom::Current(o) => self.offset = (self.offset as i64 + o) as usize,
        }

        Ok(self.offset as u64)
    }
}

#[derive(Clone)]
pub struct DataRuns(pub Vec<(u64, u64)>);

impl DataRuns {
    pub fn parse(src: &[u8], cluster_size: u64, vcns: u64, length: u64) -> DataRuns {
        let mut src = src;
        let mut absolute = 0;

        let mut runs = vec![];
        let mut total_vcns = 0;
        let mut total_length = 0;

        // println!("{} {:02x?}", vcns, src);
        while total_vcns < vcns && src.len() > 0 && src[0] != 0 {
            let head = src.load::<u8>();

            // print!("{:x} ", head);

            fn little_endian(src: &[u8]) -> u64 {
                src.into_iter().cloned().enumerate().fold(0, |v, (i, b)| {
                    v | ((b as u64) << (i * 8)) //
                })
            }

            let len_bytes: &[u8] = src.load_slice((head & 0x0F) as usize);
            let len = little_endian(len_bytes);

            let offset_bytes: &[u8] = src.load_slice((head & 0xF0) as usize >> 4);
            let offset = little_endian(offset_bytes);

            if runs.is_empty() {
                absolute = absolute + offset;
            } else {
                let shift = (8 - offset_bytes.len() as u32) * 8;
                let offset = (offset as i64).wrapping_shl(shift).wrapping_shr(shift);
                absolute = (absolute as i64 + offset) as u64;
            }

            total_vcns += len;

            // println!("{:x} {:x}", len, offset);
            // println!("   {} {} {:x}", len, offset, absolute);
            // println!("   {} {}", total_vcns, vcns);

            let mut bytes = len * cluster_size;
            total_length += bytes;
            if total_length > length {
                let empty = total_length - length;
                // println!("trim run {} {} {}", length, total_length, empty);
                bytes -= empty;
                total_length -= empty;
            }

            runs.push((absolute * cluster_size, bytes))
        }

        DataRuns(runs)
    }

    pub fn open<F: Read + Seek>(self, disk: F) -> DataRunReader<F> {
        DataRunReader::new(disk, self)
    }
}

pub struct DataRunReader<F> {
    raw: F,

    runs: DataRuns,
    index: usize,
    offset: u64,
}

impl<F: Read + Seek> DataRunReader<F> {
    pub fn new(raw: F, runs: DataRuns) -> DataRunReader<F> {
        let index = 0;
        let offset = 0;

        // println!("{:?}", runs.0);

        DataRunReader {
            raw,
            runs,
            index,
            offset,
        }
    }

    fn ready(&self) -> u64 {
        let (_, limit) = self.runs.0[self.index];
        limit - self.offset
    }

    fn next_run(&mut self) -> bool {
        if self.index + 1 == self.runs.0.len() {
            false
        } else {
            self.index += 1;
            self.offset = 0;
            true
        }
    }

    fn prev_run(&mut self) -> bool {
        if self.index == 0 {
            false
        } else {
            self.index -= 1;
            self.offset = self.runs.0[self.index].1;
            true
        }
    }

    fn seek_helper(&mut self, count: i64) -> Result<()> {
        if count > 0 {
            let mut count = count as u64;
            while count > 0 {
                let ready = self.ready();
                if ready >= count {
                    self.offset += count;
                    count = 0;
                } else {
                    count -= ready;
                    if !self.next_run() {
                        return Err(Error::new(ErrorKind::UnexpectedEof, "invalid seek"));
                    }
                }
            }
        } else if count < 0 {
            let mut count = -count as u64;
            while count > 0 {
                if self.offset >= count {
                    self.offset -= count;
                    count = 0;
                } else {
                    count -= self.offset;
                    if !self.prev_run() {
                        return Err(Error::new(ErrorKind::UnexpectedEof, "invalid seek"));
                    }
                }
            }
        }

        let (global, _) = self.runs.0[self.index];
        // println!("seek {} {}", global, self.offset);
        self.raw.seek(SeekFrom::Start(global + self.offset))?;
        // println!("seek");

        Ok(())
    }
}

impl<F: Read + Seek> Read for DataRunReader<F> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        if self.ready() == 0 && !self.next_run() {
            return Ok(0);
        }

        let limit = self.ready().min(buf.len() as u64);

        // println!("{:?}", backtrace::Backtrace::new());
        // if buf.len() == 32 {
        //     panic!();
        // }

        self.seek_helper(0)?;
        let actual = self.raw.read(&mut buf[0..limit as usize])?;
        // println!(
        //     "read {} {:?} {} {} {:04x}",
        //     limit,
        //     buf.as_ptr(),
        //     buf.len(),
        //     self.ready(),
        //     buf.as_ptr() as usize % 512
        // );

        // let actual = match self.raw.read(&mut buf[..limit as usize]) {
        //     Ok(l) => l,
        //     Err(e) => {
        //         println!("read error");
        //         if e.kind() == ErrorKind::InvalidInput {
        //             let sectors = (limit as usize + 511) / 512;
        //             let mut align = vec![0; (sectors + 1) * 512];
        //             let error = 512 - (align.as_ptr() as usize % 512);
        //             let aligned = &mut align[error..error + sectors * 512];
        //             println!(
        //                 "{:?} {}",
        //                 aligned.as_ptr(),
        //                 aligned.as_mut_ptr() as usize % 512
        //             );
        //             let actual = self.raw.read(aligned)?;
        //             buf[0..limit as usize].copy_from_slice(aligned);
        //             actual
        //         } else {
        //             return Err(e);
        //         }
        //     }
        // };

        // println!("read done {}", actual);

        self.offset += actual as u64;
        Ok(actual)
    }
}

impl<F: Read + Seek> Seek for DataRunReader<F> {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        match pos {
            SeekFrom::Start(o) => {
                self.index = 0;
                self.offset = 0;
                self.seek_helper(o as i64)?;
            }
            SeekFrom::End(o) => {
                self.index = self.runs.0.len() - 1;
                self.offset = self.runs.0[self.index].1;
                self.seek_helper(o)?;
            }
            SeekFrom::Current(o) => {
                self.seek_helper(o)?;
            }
        }

        let global = self.runs.0[0..self.index]
            .iter()
            .fold(0, |acc, (_, len)| acc + *len);

        Ok(global + self.offset)
    }
}

flat_data!(FileReference);
#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct FileReference {
    raw: u64le,
}

impl FileReference {
    pub fn new(record: u64, sequence: u16) -> FileReference {
        if record > 1 << 48 {
            panic!("invalid file reference")
        }

        let raw = record | ((sequence as u64) << 48);
        let raw = u64le::new(raw);
        FileReference { raw }
    }

    pub fn record_number(&self) -> u64 {
        self.raw.get() & 0xFFFF_FFFF_FFFF
    }

    pub fn sequence_number(&self) -> u16 {
        (self.raw.get() >> 48) as u16
    }
}

impl std::fmt::Debug for FileReference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({} {})", self.record_number(), self.sequence_number())
    }
}
