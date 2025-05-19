use clap::Parser;
use number_prefix::NumberPrefix;
use std::{fmt::Debug, fs, path::Path};

#[derive(Parser, Debug)]
#[command(version = "1.0", about, long_about = None)]
struct Args {
    /// Vector of files/directories to analyze
    #[arg(value_name = "file")]
    files: Vec<String>,

    /// The maximum print depth
    #[arg(short, value_parser = clap::value_parser!(u32).range(0..))]
    depth: Option<u32>,
}

struct Options {
    max_depth: Option<u32>,
}

fn main() {
    let args = Args::parse();
    let options = Options {
        max_depth: args.depth,
    };
    if args.files.len() == 0 {
        get_size(".", &options, 0);
    }

    for path in args.files {
        get_size(path, &options, 0);
    }
}

fn get_size<P: AsRef<Path> + Debug>(dir: P, options: &Options, depth: u32) -> u64 {
    if let Ok(meta) = fs::metadata(&dir) {
        if !meta.is_dir() {
            let size = meta.len();

            if depth == 0 {
                print_size(size as f64, dir.as_ref().as_os_str().to_str().unwrap());
            }

            return size;
        }
    } else {
        eprintln!("Could not get metadata for {:?}", dir);
        return 0;
    }

    let mut size: u64 = 0;

    let paths = fs::read_dir(&dir);
    if paths.is_err() {
        return size;
    }
    for path in paths.unwrap() {
        match path {
            Ok(path) => {
                size += get_size(path.path(), options, depth + 1);
            }
            Err(_) => {}
        }
    }

    if options.max_depth.is_none() || options.max_depth.unwrap() >= depth {
        print_size(size as f64, dir.as_ref().as_os_str().to_str().unwrap());
    }

    size
}

fn print_size(size: f64, path: &str) {
    match NumberPrefix::decimal(size) {
        NumberPrefix::Standalone(bytes) => {
            println!("{:<10} {}", format!("{} bytes", bytes), path)
        }
        NumberPrefix::Prefixed(prefix, n) => {
            println!("{:<10} {}", format!("{:.1}{}B", n, prefix), path)
        }
    };
}
