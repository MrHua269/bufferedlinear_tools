use crate::region_file::{ParseError, Region};
use chrono::Local;
use clap::{Parser, ValueEnum};
use rayon::iter::IntoParallelRefIterator;
use rayon::iter::ParallelIterator;
use std::error::Error;
use std::fs;
use std::fs::read;
use std::path::PathBuf;
use thiserror::Error;

mod region_file;
mod chunk;
mod nbt;

#[derive(Parser)]
#[command(
    name = "bufferedlinear_tools",
    version = "2.0",
    about = "Buffered linear region format convertor.",
    long_about = None,
)]
pub struct Cli {
    /// Convertor mode (mca2blinear, blinear2mca, linear2mca, linear2blinear, blinear2mca, blinear2linear)
    #[arg(value_enum, required = true)]
    pub mode: Mode,

    #[arg(value_enum, required = true)]
    pub region_type: RegionType,

    /// Path to your Minecraft Worlds containing `regions` or `entities` or `poi` file
    #[arg(required = true)]
    pub world_path: PathBuf,

    #[arg(required = true)]
    pub output_path: PathBuf,

    /// Compression level when writing region files
    #[arg(short, long, default_value = "6", value_parser = validate_compression_level)]
    pub compression_level: u32,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum Mode {
    LinearMca,
    McaLinear,
    McaBlinear,
    BlinearMca,
    BlinearLinear,
    LinearBlinear
}

#[derive(Error, Debug)]
pub enum ConverseError {
    #[error("I/O error")]
    ReadError,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum RegionType{
    REGION,
    POI,
    ENTITIES
}

fn validate_compression_level(s: &str) -> Result<u32, String> {
    match s.parse::<u32>() {
        Ok(level) if level <= 22 => Ok(level),
        _ => Err("Compression level must be an integer between 0 and 22".to_string()),
    }
}

fn folder_name(region_type: RegionType) -> String {
    match region_type {
        RegionType::REGION => String::from("region"),
        RegionType::POI => String::from("poi"),
        RegionType::ENTITIES => String::from("entities")
    }
}

fn output_file_extension_by_mode(mode: Mode) -> String{
    match mode {
        Mode::McaBlinear => String::from(".blinear"),
        Mode::LinearBlinear => String::from("blinear"),
        _ => todo!("toto") // TODO: MCA和Linear的一坨
    }
}

fn scan_region_files(region_folder: PathBuf) -> Vec<PathBuf>{
    fs::read_dir(region_folder)
        .map(|dir| {
            dir.flatten()
                .map(|entry| entry.path())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn get_input_call<'a>(mode: Mode, data: &'a [u8]) -> Box<dyn FnMut() -> Result<Region, ParseError> + 'a> {
    match mode {
        Mode::LinearMca => Box::new(|| Region::from_bytes_linear_v2(data)),
        Mode::LinearBlinear => Box::new(|| Region::from_bytes_linear_v2(data)),
        Mode::BlinearLinear => Box::new(|| Region::from_bytes_blinear(data)),
        Mode::BlinearMca => Box::new(|| Region::from_bytes_blinear(data)),
        _ => Box::new(|| todo!()), // TODO: MCA的一坨
    }
}

fn get_output_call<'a>(mode: Mode, region: &'a Region, timestamp: i64, compression_level: &'a u8) -> Box<dyn FnMut() -> Vec<u8> + 'a> {
    match mode {
        Mode::LinearBlinear => Box::new(move || Region::to_bytes_blinear(region, timestamp, *compression_level)),
        Mode::McaBlinear => Box::new(move || Region::to_bytes_blinear(region, timestamp, *compression_level)),
        _ => Box::new(|| todo!()), // TODO: MCA和Linear的一坨
    }
}


fn do_converse_single(input: &PathBuf, output: &PathBuf, mode: Mode, compression_level: u8) -> Result<(), Box<dyn Error>>{
    let read_bytes = read(&input)?;
    let mut reader_processor = get_input_call(mode, &read_bytes);

    let region_result: Result<Region, ParseError> = reader_processor();
    let region = region_result?;

    let new_timestamp = Local::now().timestamp_millis();

    let mut output_processor = get_output_call(mode, &region, new_timestamp, &compression_level);
    let converted_bytes = output_processor();

    fs::write(output, converted_bytes)?;

    Ok(())
}

fn do_converse_all(mode: Mode, world_folder: PathBuf, output_folder: PathBuf, region_type: RegionType, compression_level: u8) {
    let region_folder = folder_name(region_type);
    let input_folder_actual = world_folder.join(&region_folder);


    if !output_folder.exists() {
        fs::create_dir_all(&output_folder).expect("Failed to create dirs!");
    }

    let scanned = scan_region_files(input_folder_actual);
    let actual_output_folder = output_folder.join(&region_folder);

    if !actual_output_folder.exists() {
        fs::create_dir_all(&actual_output_folder).expect("Failed to create region typed dirs!");
    }

    scanned.par_iter().for_each(|region_file| {
        let file_name = String::from(region_file.file_stem().unwrap().to_str().unwrap());
        let output_file = file_name + "." + &*output_file_extension_by_mode(mode);

        let output_pathbuf = actual_output_folder.join(output_file);

        let convert_result = do_converse_single(region_file, &output_pathbuf, mode, compression_level);

        if convert_result.is_err() {
            let err = convert_result.err().unwrap();

            eprintln!("Failed to convert file {} !, error : {}", region_file.as_path().display(), err);
            return;
        }
        
        if convert_result.is_ok() {
            println!("Done conversation for file {}", region_file.as_path().display());
        }
    })
}

fn main() {
    let cli = Cli::parse();

    do_converse_all(cli.mode, cli.world_path, cli.output_path, cli.region_type, cli.compression_level as u8);
}
