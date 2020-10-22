#![feature(map_into_keys_values)]

extern crate avg32;
extern crate serde;
#[macro_use] extern crate serde_derive;
extern crate lexpr;
extern crate serde_lexpr;
extern crate anyhow;
#[macro_use] extern crate log;
extern crate env_logger;
extern crate clap;

#[cfg(test)]
extern crate pretty_assertions;

mod disasm;

use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use anyhow::Result;
use clap::{Arg, App, SubCommand};
use avg32::write::Writeable;

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

fn get_app<'a, 'b>() -> App<'a, 'b> {
    App::new("My Super Program")
        .version("1.0")
        .author("Kevin K. <kbknapp@gmail.com>")
        .about("Does awesome things")
        .subcommand(SubCommand::with_name("disasm")
                    .about("Disassemble an AVG32 scene")
                    .arg(Arg::with_name("output-dir")
                         .short("o")
                         .long("output-dir")
                         .help("output directory")
                         .takes_value(true)
                         .value_name("DIR"))
                    .arg(Arg::with_name("FILE")
                         .required(true)
                         .help("SEEN<nnn>.txt file")
                         .index(1))
        )
        .subcommand(SubCommand::with_name("asm")
                    .about("Assemble an adieu disassembly into an AVG32 scene")
                    .arg(Arg::with_name("output-dir")
                         .short("o")
                         .long("output-dir")
                         .help("output directory")
                         .takes_value(true)
                         .value_name("DIR"))
                    .arg(Arg::with_name("FILE")
                         .required(true)
                         .help("SEEN<nnn>.adieu file")
                         .index(1)))
}


fn main() -> Result<()> {
    env_logger::init();

    let app = get_app();
    let matches = app.get_matches();

    match matches.subcommand() {
        ("disasm", Some(sub_matches)) => {
            let input_file = Path::new(sub_matches.value_of("FILE").unwrap());
            let output_dir = match sub_matches.value_of("output-dir") {
                Some(dir) => Path::new(dir),
                None => input_file.parent().unwrap()
            };
            let output_file = output_dir.join(input_file.with_extension("adieu").file_name().unwrap());

            let scene = avg32::load(&input_file.to_str().unwrap())?;
            let sexp = disasm::disassemble(&scene)?;

            let mut file = File::create(&output_file)?;
            file.write_all(&sexp.as_bytes())?;

            println!("Wrote to {:?}.", output_file);
        },
        ("asm", Some(sub_matches)) => {
            let input_file = Path::new(sub_matches.value_of("FILE").unwrap());
            let output_dir = match sub_matches.value_of("output-dir") {
                Some(dir) => Path::new(dir),
                None => input_file.parent().unwrap()
            };
            let output_file = output_dir.join(input_file.with_extension("TXT").file_name().unwrap());

            let sexp = fs::read_to_string(&input_file)?;
            let scene = disasm::assemble(&sexp)?;

            let mut file = File::create(&output_file)?;
            scene.write(&mut file)?;

            println!("Wrote to {:?}.", output_file);
        },
        _ => println!("{}", matches.usage())
    }

    Ok(())
}
