use std::{
    io::{prelude::*, Result, SeekFrom},
    ops::Range,
};

fn floor(offset: i64, alignment: u64) -> i64 {
    if offset < 0 {
        (offset - (alignment as i64 - 1)) / alignment as i64 * alignment as i64
    } else {
        offset / alignment as i64 * alignment as i64
    }
}

pub struct AlignedFile<F> {
    file: F,
    buffer: Vec<u8>,
    buffer_align: Range<usize>,
    buffer_valid: Range<usize>,
    alignment: u64,
    skip: u64,
}

impl<F> AlignedFile<F> {
    pub fn new(file: F, alignment: usize, blocks: usize) -> AlignedFile<F> {
        let buffer = vec![0; (blocks + 1) * alignment];
        // how much is before the first aligned block
        let prefix = alignment - buffer.as_ptr() as usize % alignment;
        // align subsection of the buffer
        let buffer_align = prefix..prefix + blocks * alignment;

        assert!(buffer_align.end <= buffer.len());

        let buffer_filled = 0..0;

        AlignedFile {
            file,
            buffer,
            buffer_align,
            buffer_valid: buffer_filled,
            alignment: alignment as u64,
            skip: 0,
        }
    }
}

impl<F: Read + Seek> Seek for AlignedFile<F> {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        // println!("aligned seek {:?}", pos);

        let result = match pos {
            SeekFrom::Start(o) => {
                let floored = floor(o as i64, self.alignment) as u64;
                let result = self.file.seek(SeekFrom::Start(floored))?;
                self.skip = o - floored;
                result
            }
            SeekFrom::Current(o) => {
                let o = o + self.skip as i64;
                let floored = floor(o, self.alignment);
                let result = self.file.seek(SeekFrom::Current(floored))?;
                self.skip = (o - floored) as u64;
                result
            }
            SeekFrom::End(o) => {
                let floored = floor(o, self.alignment);
                let result = self.file.seek(SeekFrom::End(floored))?;
                self.skip = (o - floored) as u64;
                result
            }
        };

        // println!("  {} {}", result, self.skip);
        self.buffer_valid = 0..0;
        Ok(result + self.skip)
    }
}

impl<F: Read + Seek> Read for AlignedFile<F> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        if self.buffer_valid.len() == 0 {
            let aligned = &mut self.buffer[self.buffer_align.clone()];
            let count = self.file.read(aligned)?;
            self.buffer_valid = self.buffer_align.start..self.buffer_align.start + count;
        }

        self.buffer_valid.start += self.skip as usize;
        self.skip = 0;

        let count = buf.len().min(self.buffer_valid.len());
        let src = &self.buffer[self.buffer_valid.start..self.buffer_valid.start + count];
        buf[0..count].copy_from_slice(src);
        self.buffer_valid.start += count;

        Ok(count)
    }
}
