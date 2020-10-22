extern crate avg32;
extern crate serde;
extern crate lexpr;
extern crate serde_lexpr;
extern crate anyhow;

#[cfg(test)]
#[macro_use] extern crate pretty_assertions;

#[cfg(test)]
use pretty_assertions::{assert_eq, assert_ne};

mod disasm;

use std::fs;
use anyhow::Result;

#[allow(dead_code)]
fn readall() -> Result<()> {
    for entry in fs::read_dir("SEEN")? {
        let entry = entry?;
        let path = entry.path();

        let metadata = fs::metadata(&path)?;
        if metadata.is_file() {
            //println!("{:?}", avg32::load(&path.to_str().unwrap()));
            avg32::load(&path.to_str().unwrap()).unwrap();
        }
    }

    Ok(())
}

fn main() -> Result<()> {
    let scene = avg32::load(&"SEEN/SEEN020.TXT").unwrap();
    println!("{}", disasm::disassemble(&scene));

    Ok(())
}
