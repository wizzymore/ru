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
    // If dir is actually a file
    match fs::metadata(&dir) {
        Ok(meta) => {
            if !meta.is_dir() {
                let size = meta.len();

                // Only if we are giving as an argument a file print the file stats
                if depth == 0 {
                    print_size(size, dir.as_ref().as_os_str().to_str().unwrap());
                }

                return size;
            }
        }
        Err(_) => {
            eprintln!("Could not get metadata for {:?}", dir);
            return 0;
        }
    }

    // If actually a dir
    match fs::read_dir(&dir) {
        Ok(paths) => {
            let size = paths
                .map(|path| {
                    if let Ok(path) = path {
                        return get_size(path.path(), options, depth + 1);
                    }
                    0
                })
                .sum();

            // If no limit is set
            // If we are still within limit
            // Then print the size of the current folder
            if options.max_depth.is_none() || options.max_depth.unwrap() >= depth {
                print_size(size, dir.as_ref().as_os_str().to_str().unwrap());
            }

            size
        }
        Err(_) => 0,
    }
}

fn print_size(size: u64, path: &str) {
    // Possible loss of digits
    match NumberPrefix::decimal(size as f64) {
        NumberPrefix::Standalone(bytes) => {
            println!("{:<10} {}", format!("{} bytes", bytes), path)
        }
        NumberPrefix::Prefixed(prefix, n) => {
            println!("{:<10} {}", format!("{:.1}{}B", n, prefix), path)
        }
    };
}
