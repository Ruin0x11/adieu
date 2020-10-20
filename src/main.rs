extern crate avg32;
extern crate anyhow;

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
    // readall()?;
    println!("{:x?}", avg32::load(&"SEEN/SEEN011.TXT"));
    Ok(())
}
