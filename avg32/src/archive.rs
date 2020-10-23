use std::fs::File;
use std::io::{self, Read, Write, Cursor};
use std::path::Path;
use std::mem;
use anyhow::{Result, anyhow};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use crate::write::Writeable;

#[derive(Debug)]
pub struct ArchiveData {
    pub entries: u32,
    pub orgsize: u32,
    pub arcsize: u32,
    pub data: Vec<u8>,
}

impl ArchiveData {
    pub fn decompress(&self) -> Result<Vec<u8>> {
        decompress(&self.data, self.orgsize as usize)
    }
}

#[derive(Debug)]
pub struct ArchiveEntry {
    pub filename: String,
    pub offset: u32,
    pub arcsize: u32,
    pub filesize: u32,
    pub unk1: u32
}

#[derive(Debug)]
pub struct Archive {
    pub unk1: Vec<u8>,
    pub unk2: Vec<u8>,
    pub entries: Vec<ArchiveEntry>,
    pub data: Vec<ArchiveData>
}

impl Archive {
    pub fn new() -> Self {
        Archive {
            unk1: vec![0; 0x0C],
            unk2: vec![0; 0x0C],
            entries: Vec::new(),
            data: Vec::new()
        }
    }

    pub fn add_entry(&mut self, filename: String, data: Vec<u8>) -> Result<()> {
        let compressed = compress(&data)?;

        let entry = ArchiveEntry {
            filename: filename,
            offset: self.byte_size() as u32,
            arcsize: compressed.len() as u32 + 0x10,
            filesize: data.len() as u32,
            unk1: 1
        };

        let data = ArchiveData {
            entries: 0,
            orgsize: data.len() as u32,
            arcsize: compressed.len() as u32 + 0x10,
            data: compressed
        };

        self.entries.push(entry);
        self.data.push(data);

        Ok(())
    }

    pub fn finalize(&mut self) {
        let mut offset = b"PACL".len() + self.unk1.byte_size() + mem::size_of::<u32>() + self.unk2.byte_size() + self.entries.byte_size();
        for (i, entry) in self.entries.iter_mut().enumerate() {
            entry.offset = offset as u32;
            offset += self.data[i].byte_size();
        }
    }
}

pub mod parser {
    use super::*;
    use nom::number::streaming::le_u32;
    use crate::parser::{c_string, CustomError};

    named!(archive_data<&[u8], ArchiveData, CustomError<&[u8]>>,
           do_parse!(
               dbg_dmp!(tag!("PACK")) >>
                   entries: le_u32 >>
                   orgsize: le_u32 >>
                   arcsize: le_u32 >>
                   data: take!(arcsize - 0x10) >>
                   (ArchiveData {
                       entries: entries,
                       orgsize: orgsize,
                       arcsize: arcsize,
                       data: data.to_vec()
                   })
           )
    );

    named!(archive_entry<&[u8], ArchiveEntry, CustomError<&[u8]>>,
           do_parse!(
               filename: map_res!(take!(0x10), c_string) >>
                   offset: le_u32 >>
                   arcsize: le_u32 >>
                   filesize: le_u32 >>
                   unk1: le_u32 >>
                   (ArchiveEntry {
                       filename: filename.1,
                       offset: offset,
                       arcsize: arcsize,
                       filesize: filesize,
                       unk1: unk1
                   })
           )
    );

    named!(pub archive<&[u8], Archive, CustomError<&[u8]>>,
           do_parse!(
               tag!("PACL") >>
                   unk1: take!(0x0C) >>
                   entry_count: le_u32 >>
                   unk2: take!(0x0C) >>
                   entries: count!(archive_entry, entry_count as usize) >>
                   data: count!(archive_data, entries.len()) >>
                   eof!() >>
                   (Archive {
                       unk1: unk1.to_vec(),
                       unk2: unk2.to_vec(),
                       entries: entries,
                       data: data
                   })
           )
    );
}

impl Writeable for ArchiveData {
    fn byte_size(&self) -> usize {
        b"PACK".len()
            + self.entries.byte_size()
            + self.orgsize.byte_size()
            + self.arcsize.byte_size()
            + self.data.byte_size()
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        writer.write_all(b"PACK")?;
        self.entries.write(writer)?;
        self.orgsize.write(writer)?;
        self.arcsize.write(writer)?;
        self.data.write(writer)
    }
}

impl Writeable for ArchiveEntry {
    fn byte_size(&self) -> usize {
        0x10
            + self.offset.byte_size()
            + self.arcsize.byte_size()
            + self.filesize.byte_size()
            + self.unk1.byte_size()
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        if self.filename.len() > 0x10 {
            return Err(io::Error::new(io::ErrorKind::Other, "Cannot fit filename into 16 bytes"));
        }

        let mut bytes = vec![];
        bytes.write_all(self.filename.as_bytes())?;
        while bytes.len() < 0x10 {
            bytes.push(0);
        }

        bytes.write(writer)?;
        self.offset.write(writer)?;
        self.arcsize.write(writer)?;
        self.filesize.write(writer)?;
        self.unk1.write(writer)
    }
}

impl Writeable for Archive {
    fn byte_size(&self) -> usize {
        b"PACL".len()
            + self.unk1.byte_size()
            + mem::size_of::<u32>()
            + self.unk2.byte_size()
            + self.entries.byte_size()
            + self.data.byte_size()
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<(), io::Error> {
        if self.entries.len() != self.data.len() {
            return Err(io::Error::new(io::ErrorKind::Other, "Number of entries and data do not match"));
        }

        writer.write_all(b"PACL")?;
        self.unk1.write(writer)?;
        (self.entries.len() as u32).write(writer)?;
        self.unk2.write(writer)?;
        for entry in self.entries.iter() {
            entry.write(writer)?;
        }
        for data in self.data.iter() {
            data.write(writer)?;
        }
        Ok(())
    }
}

pub fn load<T: AsRef<Path>>(filepath: T) -> Result<Archive> {
    match File::open(filepath.as_ref()) {
        Ok(mut f) => {
            let mut buffer = Vec::new();
            f.read_to_end(&mut buffer).expect("Unable to read file");
            load_bytes(&buffer)
        }
        Err(e) => Err(anyhow!("Unable to load file: {}", e)),
    }
}

pub fn load_bytes(bytes: &[u8]) -> Result<Archive> {
    let res = match parser::archive(bytes) {
        Ok((_, parsed)) => Ok(parsed),
        Err(_) => Err(anyhow!("Not a valid AVG32 archive")),
    };

    print_trace!();

    res
}

pub fn decompress(input: &[u8], orgsize: usize) -> Result<Vec<u8>> {
    let mut res = vec![];
    let mut f = 0;
    let mut cur = Cursor::new(input);
    let mut i = 0;

    while res.len() < orgsize {
        let cnt = i % 8;

        if cnt == 0 {
            f = cur.read_u8()?;
        }

        if f & (0x80 >> cnt) != 0 {
            let b = cur.read_u8()?;
            res.write_u8(b)?;
        } else {
            let w = cur.read_u16::<LittleEndian>()?;
            let l = (w & 0xF) + 2;
            let d = (w >> 4) as usize;
            for _ in 0..l {
                let b = res[res.len()-d-1];
                res.write_u8(b)?;
                if res.len() >= orgsize {
                    break;
                }
            }
        }

        i = i + 1;
    }

    if res.len() != orgsize {
        return Err(anyhow!("Decompressed size != orgsize: {} != {}", res.len(), orgsize));
    }

    Ok(res)
}

pub fn compress(input: &[u8]) -> Result<Vec<u8>> {
    let mut res = vec![];

    // TODO: This cheats, it doesn't compress anything but instead outputs data
    // in a format that can be read succesfully by the LZ77 algorithm.
    for (i, b) in input.iter().enumerate() {
        if i % 8 == 0 {
            res.write_u8(0xFF)?;
        }
        res.write_u8(*b)?;
    }

    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_decompress() {
        let bytes = vec![0xFC, 0x54, 0x50, 0x43, 0x33, 0x32, 0x00, 0x0F, 0x00, 0x0F, 0x00, 0x85, 0x01, 0x1F, 0x01, 0x0F];
        let expected = vec![0x54, 0x50, 0x43, 0x33, 0x32, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        assert_eq!(&expected, &decompress(&bytes, expected.len()).unwrap());
    }

    #[test]
    fn test_decompress_compress_seen() {
        let arc = super::load("../SEEN.TXT").unwrap();

        for (i, entry) in arc.entries.iter().enumerate() {
            let data = &arc.data[i];
            let decomp = decompress(&data.data, data.orgsize as usize).unwrap();
            // let comp = compress(&decomp).unwrap()

            assert_eq!(data.orgsize as usize, decomp.len());
            // assert_eq!(&data.data, &comp);
        }
    }
}
