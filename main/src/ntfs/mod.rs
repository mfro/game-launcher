use std::{
    cell::RefCell,
    ffi::OsString,
    fs::File,
    io::{prelude::*, BufReader, Error, ErrorKind, Result, SeekFrom},
    path::Path,
    path::PathBuf,
    rc::Rc,
};

use flat::prelude::*;

use winapi::um::{
    errhandlingapi::GetLastError, fileapi::CreateFileW, fileapi::OPEN_EXISTING,
    ioapiset::DeviceIoControl, winioctl::FSCTL_GET_NTFS_VOLUME_DATA,
    winioctl::NTFS_VOLUME_DATA_BUFFER, winnt::FILE_SHARE_READ, winnt::FILE_SHARE_WRITE,
    winnt::GENERIC_READ,
};

macro_rules! win32_check {
    ( $value:expr ) => {
        win32_check!(0usize, $value)
    };
    ( $check:expr, $value:expr ) => {{
        let x = $value;
        if $check == x as usize {
            panic!("win32 error: {:x}", GetLastError());
        }
        x
    }};
}

pub use aligned::*;
mod aligned;

pub use attributes::*;
mod attributes;

pub use file_record::{FileRecord, FileReference};
mod file_record;
use file_record::DataRunReader;

flat_data!(StandardInformation);
#[repr(C, packed)]
#[derive(Copy, Clone)]
struct StandardInformation {
    c_time: u64le,
    a_time: u64le,
    m_time: u64le,
    r_time: u64le,
    dos_permissions: u32le,
    maximum_versions: u32le,
    version: u32le,
    class_id: u32le,
    owner_id: u32le,
    security_id: u32le,
    quota_charged: u32le,
    usn: u32le,
}

struct SharedFileInner<F> {
    file: F,
    position: u64,
}

pub struct SharedFile<F> {
    inner: Rc<RefCell<SharedFileInner<F>>>,
    position: u64,
}

impl<F: Seek> SharedFile<F> {
    pub fn new(file: F) -> SharedFile<F> {
        let mut file = file;
        let position = file.seek(SeekFrom::Current(0)).unwrap();

        let inner = SharedFileInner { file, position };
        let inner = Rc::new(RefCell::new(inner));

        SharedFile { inner, position }
    }
}

impl<F: Seek> Seek for SharedFile<F> {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        let mut inner = self.inner.borrow_mut();

        let elide = match pos {
            SeekFrom::Start(x) => inner.position == x,
            _ => false,
        };

        if !elide {
            // println!("seek for seek {} -> {}", inner.position, self.position);
            inner.position = inner.file.seek(pos)?;
            self.position = inner.position;
        }

        Ok(self.position)
    }
}

impl<F: Read + Seek> Read for SharedFile<F> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let mut inner = self.inner.borrow_mut();
        if inner.position != self.position {
            // println!("seek for read {} -> {}", inner.position, self.position);
            inner.file.seek(SeekFrom::Start(self.position))?;
        }
        let count = inner.file.read(buf)?;
        inner.position += count as u64;
        self.position = inner.position;
        Ok(count)
    }
}

impl<F: Write + Seek> Write for SharedFile<F> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let mut inner = self.inner.borrow_mut();
        if inner.position != self.position {
            // println!("seek for write {} -> {}", inner.position, self.position);
            inner.file.seek(SeekFrom::Start(self.position))?;
        }
        let count = inner.file.write(buf)?;
        inner.position += count as u64;
        self.position = inner.position;
        Ok(count)
    }

    fn flush(&mut self) -> Result<()> {
        let mut inner = self.inner.borrow_mut();
        inner.file.flush()
    }
}

impl<F> Clone for SharedFile<F> {
    fn clone(&self) -> Self {
        let inner = self.inner.clone();
        let position = self.position;
        // println!("clone {}", self.position);
        Self { inner, position }
    }
}

#[derive(Clone)]
pub struct NTFileSystem<F> {
    disk: F,
    record_size: u64,
    cluster_size: u64,
    mft_data: file_record::DataRuns,
}

impl<F: Read + Seek> NTFileSystem<F> {
    pub fn shared(self) -> NTFileSystem<SharedFile<F>> {
        let disk = SharedFile::new(self.disk);
        NTFileSystem {
            disk,
            record_size: self.record_size,
            cluster_size: self.cluster_size,
            mft_data: self.mft_data.clone(),
        }
    }

    pub fn open<'a>(
        &mut self,
        content: file_record::AttributeContent<'a>,
    ) -> file_record::AttributeReader<'a, &mut F> {
        match content {
            file_record::AttributeContent::Resident(raw) => {
                file_record::AttributeReader::Resident(file_record::ResidentReader::new(raw))
            }
            file_record::AttributeContent::NonResident(runs, vcns, length) => {
                let runs = file_record::DataRuns::parse(runs, self.cluster_size, vcns, length);
                file_record::AttributeReader::NonResident(runs.open(&mut self.disk))
            }
        }
    }

    pub fn open_mft(&mut self, buffer: usize) -> MFTReader<DataRunReader<&mut F>> {
        let reader = DataRunReader::new(&mut self.disk, self.mft_data.clone());
        MFTReader::new(reader, self.record_size, buffer)
    }

    pub fn inode_name(
        &mut self,
        inode: &FileReference,
    ) -> Result<(attributes::file_name::FileNameHeader, OsString)> {
        let mut buffer = vec![0; self.record_size as usize];
        let mut mft = self.open_mft(1);
        let (_, record) = mft.read(inode, &mut buffer)?.unwrap();

        match record.win32_file_name(self)? {
            Some(v) => Ok(v),
            None => Err(Error::new(ErrorKind::InvalidData, "no filename attribute")),
        }
    }

    pub fn inode_path(&mut self, inode: &FileReference) -> Result<PathBuf> {
        let (info, name) = self.inode_name(inode)?;
        let base = match info.parent_reference.record_number() {
            5 => std::path::MAIN_SEPARATOR.to_string().into(),
            _ => self.inode_path(&info.parent_reference)?,
        };

        return Ok(base.join(name));
    }
}

impl NTFileSystem<File> {
    pub fn open_drive<P: AsRef<Path>>(
        drive_name: P,
    ) -> Result<NTFileSystem<BufReader<AlignedFile<File>>>> {
        use std::os::windows::ffi::OsStrExt;
        use std::os::windows::io::FromRawHandle;

        let name = crate::common::to_wstr(drive_name.as_ref().as_os_str().encode_wide());
        let drive = unsafe {
            win32_check!(CreateFileW(
                name.as_ptr(),
                GENERIC_READ,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null_mut(),
                OPEN_EXISTING,
                0,
                std::ptr::null_mut(),
            ))
        };

        let mut volume_data: NTFS_VOLUME_DATA_BUFFER = unsafe { std::mem::zeroed() };
        let mut result_size = 0;

        unsafe {
            win32_check!(DeviceIoControl(
                drive,
                FSCTL_GET_NTFS_VOLUME_DATA,
                std::ptr::null_mut(),
                0,
                &mut volume_data as *mut _ as *mut _,
                std::mem::size_of_val(&volume_data) as _,
                &mut result_size,
                std::ptr::null_mut(),
            ))
        };

        let mut disk: File = unsafe { File::from_raw_handle(drive) };

        println!("{}", volume_data.BytesPerSector);

        let record_size = volume_data.BytesPerFileRecordSegment;
        let cluster_size = volume_data.BytesPerCluster as u64;
        let mft_offset_cluster = unsafe { *volume_data.MftStartLcn.QuadPart() } as u64;
        let mft_offset = mft_offset_cluster * cluster_size;

        disk.seek(SeekFrom::Start(mft_offset)).unwrap();

        let mut mft_record = vec![0; record_size as usize];
        disk.read_exact(&mut mft_record).unwrap();
        let mft = FileRecord::parse(&mut mft_record.as_ref());

        let mft_data = mft
            .attributes()
            .find(|a| a.ty() == 0x80 && a.name().is_none())
            .unwrap();

        let mft_data = match mft_data.content() {
            file_record::AttributeContent::Resident(..) => panic!("resident mft????"),
            file_record::AttributeContent::NonResident(a, vcns, length) => {
                file_record::DataRuns::parse(a, cluster_size, vcns, length)
            }
        };

        let disk = AlignedFile::new(disk, 512, 64);
        let disk = BufReader::with_capacity(4096, disk);

        Ok(NTFileSystem {
            disk,
            cluster_size,
            record_size: record_size as u64,
            mft_data,
        })
    }
}

pub struct MFTReader<F> {
    raw: F,
    buffer: Vec<u8>,
    record_size: u64,
    current_index: u64,
    buffered: std::ops::Range<usize>,
}

impl<F: Read + Seek> MFTReader<F> {
    pub fn new(raw: F, record_size: u64, buffer: usize) -> MFTReader<F> {
        let buffer = vec![0; buffer * record_size as usize];
        MFTReader {
            raw,
            buffer,
            record_size,
            current_index: 0,
            buffered: 0..0,
        }
    }

    pub fn seek(&mut self, inode: &FileReference) -> Result<()> {
        self.current_index = inode.record_number();
        let offset = inode.record_number() * self.record_size;
        self.raw.seek(SeekFrom::Start(offset))?;
        self.buffered = 0..0;
        Ok(())
    }

    pub fn next(&mut self) -> Result<Option<(FileReference, FileRecord)>> {
        loop {
            if self.buffered.len() == 0 {
                match self.raw.read(&mut self.buffer)? {
                    0 => return Ok(None),
                    l => self.buffered = 0..l,
                }
            }

            if self.buffered.len() < self.record_size as usize {
                panic!("{:?}", self.buffered);
            }

            if u32::load(&self.buffer[self.buffered.clone()]) != 0 {
                break;
            } else {
                self.current_index += 1;
                self.buffered.start += self.record_size as usize;
            }
        }

        let record = FileRecord::parse(&mut &self.buffer[self.buffered.clone()]);

        let reference = FileReference::new(self.current_index, record.header.sequence_number.get());
        self.current_index += 1;
        self.buffered.start += record.header.allocated_size.get() as usize;
        assert_eq!(record.header.allocated_size.get() as u64, self.record_size);

        Ok(Some((reference, record)))
    }

    pub fn read<'a>(
        &mut self,
        inode: &FileReference,
        buffer: &'a mut [u8],
    ) -> Result<Option<(FileReference, FileRecord<'a>)>> {
        self.seek(inode)?;

        loop {
            match self.raw.read(&mut buffer[0..self.record_size as usize])? {
                0 => return Ok(None),
                l => self.buffered = 0..l,
            }

            if u32::load(&buffer) != 0 {
                break;
            } else {
                self.current_index += 1;
            }
        }

        let mut buffer = buffer as &'a [u8];
        let record = FileRecord::parse(&mut buffer);

        let reference = FileReference::new(self.current_index, record.header.sequence_number.get());
        self.current_index += 1;
        assert_eq!(record.header.allocated_size.get() as u64, self.record_size);

        Ok(Some((reference, record)))
    }
}

pub fn test() {
    let mut fs = NTFileSystem::open_drive(r"\\.\C:").unwrap().shared();

    let mut lnks = vec![];
    let mut exes = vec![];

    let mut mft = fs.clone();
    let mut mft = mft.open_mft(32);

    while let Some((inode, record)) = mft.next().unwrap() {
        // println!("{:?}", record.header);

        let name = record.win32_file_name(&mut fs);

        if let Ok(Some((_, name))) = name {
            if let Some(name) = name.to_str() {
                if name.ends_with(".lnk") {
                    lnks.push(fs.inode_path(&inode));
                } else if name.ends_with(".exe") {
                    exes.push(fs.inode_path(&inode));
                }
            }
        } else {
            for attr in record.attributes() {
                if attr.ty() == 0x20 {
                    println!("{}", inode.record_number());
                    println!("{:?}", attr.name());
                    println!("{:?}", attr.header);
                    println!("{:02x?}", attr.content());

                    let mut content = vec![];
                    fs.open(attr.content()).read_to_end(&mut content).unwrap();
                    let mut content: &[u8] = content.as_ref();
                    println!("$ATTRIBUTE_LIST: {}", content.len());

                    println!("{:02x?}", content);

                    while content.len() > 0 {
                        println!("{}", content.len());
                        let entry: &AttributeListEntry = Flat::load(content);
                        let len = entry.len.get() as usize;

                        // let ty = content.load::<u32le>().get();
                        // let len = content.load::<u16le>().get() as usize;
                        println!("{:?}", entry);
                        if entry.ty.get() == 0 {
                            break;
                        }

                        if len > content.len() {
                            println!("invalid $ATTRIBUTE_LIST: {:?} {:?}", inode, record.header);
                        }

                        let mut mft2 = fs.clone();
                        let mut mft2 = mft2.open_mft(1);

                        mft2.seek(&entry.base_file_reference).unwrap();
                        let (_, base) = mft2.next().unwrap().unwrap();

                        for attr2 in record.attributes() {
                            if attr2.header.id.get() == entry.attribute_id.get() {
                                println!("found attribute");
                            }
                        }

                        content = &content[len..];
                    }
                }
            }
        }
    }

    for path in lnks {
        println!("{:?}", path)
    }

    for path in exes {
        println!("{:?}", path)
    }

    // unsafe {
    //     let name = crate::to_wstr(r"\\.\C:".encode_utf16());
    //     let drive = win32_check!(CreateFileW(
    //         name.as_ptr(),
    //         GENERIC_READ,
    //         FILE_SHARE_READ | FILE_SHARE_WRITE,
    //         std::ptr::null_mut(),
    //         OPEN_EXISTING,
    //         0,
    //         std::ptr::null_mut(),
    //     ));

    //     let mut volume_data: NTFS_VOLUME_DATA_BUFFER = std::mem::zeroed();
    //     let mut result_size = 0;

    //     win32_check!(DeviceIoControl(
    //         drive,
    //         FSCTL_GET_NTFS_VOLUME_DATA,
    //         std::ptr::null_mut(),
    //         0,
    //         &mut volume_data as *mut _ as *mut _,
    //         std::mem::size_of_val(&volume_data) as _,
    //         &mut result_size,
    //         std::ptr::null_mut(),
    //     ));

    //     println!(
    //         "{} +{}",
    //         volume_data.MftStartLcn.QuadPart(),
    //         volume_data.MftValidDataLength.QuadPart()
    //     );
    //     println!(
    //         "{} {}",
    //         result_size,
    //         std::mem::size_of::<NTFS_VOLUME_DATA_BUFFER>(),
    //     );
    //     println!(
    //         "{} {} {} {}",
    //         volume_data.BytesPerSector,
    //         volume_data.BytesPerCluster,
    //         volume_data.BytesPerFileRecordSegment,
    //         volume_data.ClustersPerFileRecordSegment,
    //     );

    //     let mut file: File = File::from_raw_handle(drive);

    //     let cluster_len = volume_data.BytesPerCluster as u64;
    //     let mft_offset_cluster = *volume_data.MftStartLcn.QuadPart() as u64;
    //     let mft_offset = mft_offset_cluster * cluster_len;
    //     let mft_len: usize = *volume_data.MftValidDataLength.QuadPart() as _;

    //     file.seek(SeekFrom::Start(mft_offset)).unwrap();

    //     const RECORD_SIZE: usize = 1024;
    //     const SCAN: usize = 10;

    //     let mut mft_record = [0; RECORD_SIZE];
    //     file.read_exact(&mut mft_record).unwrap();
    //     let mft = FileRecord::parse(&mut mft_record.as_ref());

    //     let data = mft
    //         .attributes()
    //         .find(|a| a.ty() == 0x80 && a.name().is_none())
    //         .unwrap();

    //     let mut src = data.content().open(file, cluster_len);

    //     let mut counter = 0;
    //     let mut records = [0; RECORD_SIZE * SCAN];
    //     for _ in 0..mft_len / (RECORD_SIZE * SCAN) {
    //         src.read_exact(&mut records).unwrap();
    //         let mut records = records.as_ref();

    //         while records.len() > 0 {
    //             if u32::load(records) == 0 {
    //                 records = &records[1024..];
    //                 continue;
    //             }

    //             let record = FileRecord::parse(&mut records);
    //             counter += 1;

    //             let name = record
    //                 .attributes()
    //                 .find(|a| a.ty() == 0x30 && a.name().is_none());

    //             if let Some(attr) = name {
    //                 let mut content = vec![];
    //                 attr.content()
    //                     .open(&mut src, cluster_len)
    //                     .read_to_end(&mut content)
    //                     .unwrap();

    //                 let name = FileName::parse(&content);

    //                 if let Ok(name) = name.name() {
    //                     if name == "$MFT" {
    //                         let data = record
    //                             .attributes()
    //                             .find(|a| a.ty() == 0x80 && a.name().is_none())
    //                             .unwrap();

    //                         println!("{:?}", data.content());
    //                     }
    //                 }
    //             }

    //             for attr in record.attributes() {
    //                 match attr.ty() {
    //                     0x30 => {
    //                         if attr.name().is_none() {
    //                             let mut content = vec![];
    //                             attr.content()
    //                                 .open(&mut src, cluster_len)
    //                                 .read_to_end(&mut content)
    //                                 .unwrap();

    //                             let x = FileName::parse(&content);
    //                             if x.header.namespace == 2 {
    //                                 continue;
    //                             }

    //                             let name = match x.name() {
    //                                 Ok(s) => s,
    //                                 Err(_) => continue,
    //                             };

    //                             if name.ends_with(".lnk") {
    //                                 println!(
    //                                     "{} file name {:08x} {:02x}: {:?}",
    //                                     counter,
    //                                     x.header.flags.get(),
    //                                     x.header.namespace,
    //                                     x.name()
    //                                 );
    //                             }
    //                         }
    //                     }
    //                     0x80 => {}
    //                     _ => {}
    //                 }
    //             }
    //         }
    //     }
    // }
}
