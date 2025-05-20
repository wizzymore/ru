use clap::Parser;
use number_prefix::NumberPrefix;
use std::{fmt::Debug, fs, path::Path};

#[derive(Parser, Debug)]
#[command(version = None, about = "Estimate file space usage", long_about = None)]
struct Args {
    /// files/directories to analyze
    #[arg(value_name = "file")]
    files: Vec<String>,

    /// maximum print depth
    #[arg(short, value_parser = clap::value_parser!(u32).range(0..))]
    depth: Option<u32>,

    /// print bytes
    #[arg(short, default_value_t = false)]
    bytes: bool,
}

struct Options {
    max_depth: Option<u32>,
    bytes: bool,
}

fn main() {
    let args = Args::parse();
    let options = Options {
        max_depth: args.depth,
        bytes: args.bytes,
    };
    if args.files.is_empty() {
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
                #[cfg(not(target_os = "windows"))]
                use std::os::unix::fs::MetadataExt;
                #[cfg(not(target_os = "windows"))]
                let size = meta.blocks() * 512;
                #[cfg(target_os = "windows")]
                let size = get_size_on_disk(dir.as_ref());
                // Only if we are giving as an argument a file print the file stats
                if depth == 0 {
                    if options.bytes {
                        println!(
                            "{:<10} {}",
                            size,
                            dir.as_ref().as_os_str().to_str().unwrap()
                        )
                    } else {
                        print_size(size, dir.as_ref().as_os_str().to_str().unwrap());
                    }
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
                if options.bytes {
                    println!(
                        "{:<10} {}",
                        size,
                        dir.as_ref().as_os_str().to_str().unwrap()
                    )
                } else {
                    print_size(size, dir.as_ref().as_os_str().to_str().unwrap());
                }
            }

            size
        }
        Err(_) => 0,
    }
}

fn print_size(size: u64, path: &str) {
    // Possible loss of digits
    match NumberPrefix::binary(size as f64) {
        NumberPrefix::Standalone(bytes) => {
            println!("{:<10} {}", format!("{}", bytes), path)
        }
        NumberPrefix::Prefixed(prefix, n) => {
            println!("{:<10} {}", format!("{:.1}{}B", n, prefix), path)
        }
    };
}

#[cfg(windows)]
fn get_size_on_disk(path: &Path) -> u64 {
    use std::os::windows::io::AsRawHandle;

    use windows_sys::Win32::{
        Foundation::HANDLE,
        Storage::FileSystem::{FILE_STANDARD_INFO, FileStandardInfo, GetFileInformationByHandleEx},
    };

    let mut size_on_disk = 0;

    // bind file so it stays in scope until end of function
    // if it goes out of scope the handle below becomes invalid
    let Ok(file) = std::fs::File::open(path) else {
        return size_on_disk; // opening directories will fail
    };

    unsafe {
        let mut file_info: FILE_STANDARD_INFO = core::mem::zeroed();
        let file_info_ptr: *mut FILE_STANDARD_INFO = &mut file_info;

        let success = GetFileInformationByHandleEx(
            file.as_raw_handle() as HANDLE,
            FileStandardInfo,
            file_info_ptr.cast(),
            size_of::<FILE_STANDARD_INFO>() as u32,
        );

        if success != 0 {
            size_on_disk = file_info.AllocationSize as u64;
        }
    }

    size_on_disk
}
