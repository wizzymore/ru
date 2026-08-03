use clap::{Parser, crate_description, crate_version};
use ignore::gitignore::Gitignore;
use rayon::iter::*;
#[cfg(target_os = "linux")]
use std::{collections::HashSet, sync::Mutex};
use std::{fmt::Debug, fs, io::IsTerminal, path::Path};

mod humanformat;

#[cfg(target_os = "linux")]
type Visited = Mutex<HashSet<(u64, u64)>>;

#[cfg(not(target_os = "linux"))]
type Visited = ();

#[derive(Parser, Debug)]
#[command(version = crate_version!(), about = crate_description!(), long_about = None, color = clap::ColorChoice::Always)]
struct Args {
    /// files/directories to analyze
    #[arg(value_name = "path", default_value = ".")]
    files: Vec<String>,

    /// maximum print depth
    #[arg(short, default_value = None)]
    depth: Option<usize>,

    /// print bytes
    #[arg(short, default_value_t = false)]
    bytes: bool,

    /// use .gitignore file for printing sizes
    #[arg(long, short, default_value_t = false)]
    ignore: bool,

    /// don't display colored output
    #[arg(long, short = 'c', default_value_t = false)]
    no_color: bool,
}

struct Options {
    max_depth: Option<usize>,
    bytes: bool,
    ignore: bool,
    no_color: bool,
}

fn main() {
    let args = Args::parse();
    let options = Options {
        max_depth: args.depth,
        bytes: args.bytes,
        ignore: args.ignore,
        no_color: !std::io::stdout().is_terminal() || !args.no_color,
    };

    args.files.iter().for_each(|path| {
        let (gitignore, _) = Gitignore::new(Path::new(path).join(".gitignore"));
        visit_path(path, &options, &gitignore);
    });
}

fn visit_path<P: AsRef<Path>>(path: P, options: &Options, gitignore: &Gitignore) {
    #[cfg(target_os = "linux")]
    let visited = Visited::new(HashSet::new());
    #[cfg(not(target_os = "linux"))]
    let visited = ();

    compute_usage(path, 0, options, gitignore, &visited);
}

#[cfg(target_os = "linux")]
fn should_count(meta: &fs::Metadata, visited: &Visited) -> bool {
    use std::os::unix::fs::MetadataExt;
    if meta.nlink() <= 1 {
        return true;
    }

    let key = (meta.dev(), meta.ino());

    visited.lock().unwrap().insert(key)
}

#[cfg(not(target_os = "linux"))]
fn should_count(_: &fs::Metadata, _: &Visited) -> bool {
    true
}

fn compute_usage<P: AsRef<Path>>(
    path: P,
    depth: usize,
    options: &Options,
    gitignore: &Gitignore,
    visited: &Visited,
) -> u64 {
    let path = path.as_ref();
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e) => {
            eprintln!("{}: {}", path.display(), e);
            return 0;
        }
    };

    if !should_count(&meta, visited) {
        return 0;
    }

    if options.ignore {
        #[cfg(windows)]
        let hidden = gitignore.matched(path, meta.is_dir()).is_ignore() || is_hidden(&meta);
        #[cfg(not(windows))]
        let hidden = gitignore.matched(path, meta.is_dir()).is_ignore() || is_hidden(path);

        if hidden {
            return 0;
        }
    };

    let mut size = get_file_size(&meta, path);

    if meta.is_dir() {
        match fs::read_dir(path) {
            Ok(entries) => {
                size += entries
                    .par_bridge()
                    .map(|entry| match entry {
                        Ok(entry) => {
                            compute_usage(entry.path(), depth + 1, options, gitignore, visited)
                        }
                        Err(e) => {
                            eprintln!("{}", e);
                            0
                        }
                    })
                    .sum::<u64>();
                if options.max_depth.is_none_or(|d| d >= depth) {
                    print_size(size, path.display(), options.bytes, options.no_color);
                }
                return size;
            }
            Err(e) => {
                match e.kind() {
                    std::io::ErrorKind::PermissionDenied => {
                        eprintln!("{}: Permission denied", path.display())
                    }
                    std::io::ErrorKind::NotFound => {
                        eprintln!("{}: Not found", path.display())
                    }
                    _ => {
                        eprintln!("{}: {}", path.display(), e)
                    }
                }
                return 0;
            }
        }
    }

    if depth == 0 {
        std::hint::cold_path();
        print_size(size, path.display(), options.bytes, options.no_color);
    }

    size
}

fn get_file_size(meta: &fs::Metadata, path: &Path) -> u64 {
    let size;
    #[cfg(not(windows))]
    {
        _ = path;
        use std::os::unix::fs::MetadataExt;
        size = meta.blocks() * 512;
    }
    #[cfg(windows)]
    {
        size = get_size_on_disk(path);
    }
    size
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
            core::mem::size_of::<FILE_STANDARD_INFO>() as u32,
        );

        if success != 0 {
            size_on_disk = file_info.AllocationSize as u64;
        }
    }

    size_on_disk
}

#[cfg(windows)]
fn is_hidden(meta: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    meta.file_attributes() & 0x2 != 0
}

#[cfg(not(windows))]
fn is_hidden(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.starts_with("."))
        .unwrap_or(false)
}

#[cfg(target_os = "linux")]
const HUMAN_FORMAT: humanformat::Kilo = humanformat::Kilo::Binary;

#[cfg(not(target_os = "linux"))]
const HUMAN_FORMAT: humanformat::Kilo = humanformat::Kilo::Decimal;

fn print_size<T: std::fmt::Display>(size: u64, path: T, print_bytes: bool, print_no_color: bool) {
    if print_bytes {
        if !print_no_color {
            println!("\x1b[1;33m{:<10}\x1b[0m \x1b[1;36m{}\x1b[0m", size, path);
        } else {
            println!("{:<10} {}", size, path);
        }
    } else {
        let formatted_size = humanformat::format_size(size as f64, HUMAN_FORMAT);
        if !print_no_color {
            println!(
                "\x1b[1;33m{:<10}\x1b[0m \x1b[1;36m{}\x1b[0m",
                formatted_size, path
            );
        } else {
            println!("{:<10} {}", formatted_size, path);
        }
    }
}
