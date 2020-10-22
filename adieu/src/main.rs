#![feature(map_into_keys_values)]

extern crate avg32;
extern crate serde;
#[macro_use] extern crate serde_derive;
extern crate lexpr;
extern crate serde_lexpr;
extern crate anyhow;
#[macro_use] extern crate log;
extern crate env_logger;

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
            let scene = avg32::load(&path.to_str().unwrap()).unwrap();
            disasm::disassemble(&scene)?;
        }
    }

    Ok(())
}

fn main() -> Result<()> {
    env_logger::init();

    readall()?;
    let scene = avg32::load(&"SEEN/SEEN020.TXT").unwrap();
    // println!("{}", disasm::disassemble(&scene)?);

    Ok(())
}
