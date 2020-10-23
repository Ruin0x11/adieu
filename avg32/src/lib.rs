#[macro_use] extern crate nom;
#[macro_use] extern crate nom_trace;
extern crate serde;
#[macro_use] extern crate serde_derive;
extern crate encoding_rs;
extern crate byteorder;
extern crate anyhow;

#[cfg(test)]
#[macro_use] extern crate pretty_assertions;

pub mod archive;
pub mod parser;
pub mod write;

use std::fs::File;
use std::io::Read;
use std::path::Path;
use anyhow::{anyhow, Result};

pub use parser::AVG32Scene;

pub fn load<T: AsRef<Path>>(filepath: T) -> Result<AVG32Scene> {
    match File::open(filepath.as_ref()) {
        Ok(mut f) => {
            let mut buffer = Vec::new();
            f.read_to_end(&mut buffer).expect("Unable to read file");
            load_bytes(&buffer)
        }
        Err(e) => Err(anyhow!("Unable to load file: {}", e)),
    }
}

pub fn load_bytes(bytes: &[u8]) -> Result<AVG32Scene> {
    let res = match parser::avg32_scene(bytes) {
        Ok((_, parsed)) => Ok(parsed),
        Err(e) => Err(anyhow!("Not a valid AVG32 scene: {}", e)),
    };

    print_trace!();

    res
}
