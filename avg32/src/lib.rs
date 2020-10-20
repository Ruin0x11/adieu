#[macro_use] extern crate nom;

pub mod parser;

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
    match parser::avg32_scene(bytes) {
        Ok((_, parsed)) => Ok(parsed),
        Err(_) => Err("Not a valid AVG32 scene"),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
