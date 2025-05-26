use clap::Parser;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::{fmt::Debug, fs, path::Path, usize};

#[derive(Parser, Debug)]
#[command(version = None, about = "Estimate file space usage", long_about = None)]
struct Args {
    /// files/directories to analyze
    #[arg(value_name = "file")]
    files: Vec<String>,

    /// maximum print depth
    #[arg(short, default_value_t = usize::MAX)]
    depth: usize,

    /// print bytes
    #[arg(short, default_value_t = false)]
    bytes: bool,
}

struct Options {
    max_depth: usize,
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

fn get_size<P: AsRef<Path> + Debug>(dir: P, options: &Options, depth: usize) -> u64 {
    // If dir is actually a file
    let m = match fs::symlink_metadata(&dir) {
        Ok(meta) => {
            if meta.is_file() {
                #[cfg(not(windows))]
                use std::os::unix::fs::MetadataExt;
                #[cfg(not(windows))]
                let size = meta.blocks() * 512;
                #[cfg(windows)]
                let size = get_size_on_disk(dir.as_ref());
                // Only if we are giving as an argument a file print the file stats
                if depth == 0 {
                    print_size(size, dir.as_ref().display(), options.bytes);
                }

                return size;
            }
            meta
        }
        Err(_) => {
            return 0;
        }
    };

    // If actually a dir
    if m.is_dir() {
        let entries = match fs::read_dir(&dir) {
            Ok(paths) => paths.collect::<Vec<_>>(),
            Err(_) => return 0,
        };

        let size = entries
            .par_iter()
            .filter_map(|res| res.as_ref().ok())
            .map(|entry| get_size(entry.path(), &options, depth + 1))
            .sum();

        if options.max_depth >= depth {
            print_size(size, dir.as_ref().display(), options.bytes);
        }

        return size;
    }

    0
}

fn print_size<T: std::fmt::Display>(size: u64, path: T, print_bytes: bool) {
    if print_bytes {
        println!("{:<10} {}", size, path);
    } else {
        #[cfg(target_os = "linux")]
        println!(
            "{:<8} {}",
            humansize::format_size(size, humansize::BINARY),
            path
        );
        #[cfg(not(target_os = "linux"))]
        println!(
            "{:<8} {}",
            humansize::format_size(size, humansize::DECIMAL),
            path
        );
    }
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
