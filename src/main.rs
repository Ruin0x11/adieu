extern crate avg32;
extern crate anyhow;

use anyhow::Result;

// use std::fs;
// fn readall() -> Result<()> {
//     for entry in fs::read_dir("SEEN")? {
//         let entry = entry?;
//         let path = entry.path();

//         let metadata = fs::metadata(&path)?;
//         if metadata.is_file() {
//             println!("{:?}", avg32::load(&path.to_str().unwrap()));
//         }
//     }

//     Ok(())
// }

fn main() -> Result<()> {
    // readall();
    println!("{:?}", avg32::load(&"SEEN/SEEN604.TXT"));
    Ok(())
}
