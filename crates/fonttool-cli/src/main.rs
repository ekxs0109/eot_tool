use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = env::args().skip(1);

    match args.next().as_deref() {
        None | Some("-h") | Some("--help") => {
            print_help();
            ExitCode::SUCCESS
        }
        Some(_) => {
            eprintln!("fonttool: functionality not implemented yet");
            ExitCode::from(2)
        }
    }
}

fn print_help() {
    println!("fonttool");
    println!();
    println!("Usage: fonttool [OPTIONS]");
    println!();
    println!("Options:");
    println!("  -h, --help  Print help");
}
