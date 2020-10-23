use std::fs::File;
use std::io::{Read, Cursor};
use std::path::Path;
use anyhow::{Result, anyhow};

const NUM_CHARS: usize = 4418;

pub type FontChar = [u8; 576];

pub struct Font {
    /// Mapping of JIS code -> 24x24 4bpp glyph
    pub chars: Vec<FontChar>
}

pub fn load<T: AsRef<Path>>(filepath: T) -> Result<Font> {
    match File::open(filepath.as_ref()) {
        Ok(mut f) => {
            let mut buffer = Vec::new();
            f.read_to_end(&mut buffer).expect("Unable to read file");
            load_bytes(&buffer)
        }
        Err(e) => Err(anyhow!("Unable to load file: {}", e)),
    }
}

pub fn load_bytes(bytes: &[u8]) -> Result<Font> {
    if bytes.len() != NUM_CHARS * 576 {
        return Err(anyhow!("Wrong number of bytes for FN.DAT"));
    }

    let mut cursor = Cursor::new(bytes);
    let mut chars = Vec::new();

    for _ in 0..NUM_CHARS {
        let mut char = [0; 576];
        cursor.read(&mut char)?;
        chars.push(char);
    }

    Ok(Font { chars: chars })
}
