use std::{error::Error, fs::File, io::prelude::*, path::Path};

use flat::{flat_data, prelude::*};

fn main() -> Result<(), Box<dyn Error>> {
    let dir = Path::new(r"C:\Users\Max\Downloads\iconsext\icons");
    let in_path = dir.join("lghub_1.ico");

    // let mut data = vec![];
    // File::open(&dir.join(&in_path))?.read_to_end(&mut data)?;

    // let mut cursor: &[u8] = &data;
    // let header: &IconHeader = cursor.load();
    // println!("{}", header.image_count.get());
    // for i in 0..header.image_count.get() {
    //     let header: &IconImageHeader = cursor.load();
    //     let offset = header.offset.get() as usize;
    //     let length = header.size.get() as usize;
    //     let data = &data[offset..offset + length];
    //     if data[0..3] == [0x80, 0x50, 0x4e, 0x47] {
    //         File::create(dir.join(format!("{}.png", i)))?.write_all(&data)?;
    //     } else {
    //         let mut full = vec![];
    //         full.store(b"BM");
    //         full.store(data.len() as u32 + 14);
    //         full.store(0u32);
    //         full.store(104u32);
    //         full.extend_from_slice(data);
    //         File::create(dir.join(format!("{}.bmp", i)))?.write_all(&full)?;
    //     }
    // }

    let data = image::open(&in_path)?;

    let rgba = data.to_rgba();

    rgba.save(dir.join("out.ico"))?;
    rgba.save(dir.join("out.png"))?;

    Ok(())
}

flat_data!(IconHeader);
#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct IconHeader {
    pub _reserved: u16le,
    pub image_type: u16le,
    pub image_count: u16le,
}

flat_data!(IconImageHeader);
#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct IconImageHeader {
    pub width: u8,
    pub height: u8,
    pub colors: u8,
    pub _reserved: u8,
    pub color_planes: u16le,
    pub bits_per_pixel: u16le,
    pub size: u32le,
    pub offset: u32le,
}
