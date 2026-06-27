// src/main.rs
use clap::Parser;
use std::path::PathBuf;

mod git;
mod ui;

#[derive(Parser)]
struct Args {
    directory: Option<PathBuf>,
}

pub fn main() -> iced::Result {
    let args = Args::parse();
    let path = args.directory
        .unwrap_or_else(|| std::env::current_dir().expect("Cannot read current directory"));

    ui::run(path)
}
