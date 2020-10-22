#[macro_use] extern crate nom;
#[macro_use] extern crate nom_trace;
extern crate serde;
#[macro_use] extern crate serde_derive;
extern crate encoding_rs;
extern crate byteorder;

#[cfg(test)]
#[macro_use] extern crate pretty_assertions;

pub mod parser;
pub mod write;

pub use parser::AVG32Scene;
use std::fs::File;
use std::io::Read;

pub fn load<T: AsRef<str>>(filename: &T) -> Result<AVG32Scene, &'static str> {
    println!("{}", filename.as_ref());
    match File::open(filename.as_ref()) {
        Ok(mut f) => {
            let mut buffer = Vec::new();
            f.read_to_end(&mut buffer).expect("Unable to read file");
            load_bytes(&buffer)
        }
        Err(_) => Err("Unable to load file"),
    }
}

pub fn load_bytes(bytes: &[u8]) -> Result<AVG32Scene, &'static str> {
    let res = match parser::avg32_scene(bytes) {
        Ok((_, parsed)) => Ok(parsed),
        Err(_) => Err("Not a valid AVG32 scene"),
    };

    print_trace!();

    res
}

pub fn load_ops(bytes: &[u8]) -> Result<Vec<parser::Opcode>, &'static str> {
    let res = match parser::opcodes(bytes) {
        Ok((_, parsed)) => Ok(parsed),
        Err(_) => Err("Not a valid AVG32 scene"),
    };

    print_trace!();

    res
}
