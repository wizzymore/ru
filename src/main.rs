use clap::Parser;
use ignore::gitignore::Gitignore;
use rayon::prelude::*;
use std::{
    fmt::Debug,
    fs,
    path::{Path, PathBuf},
    usize,
};

#[derive(Parser, Debug)]
#[command(version = None, about = "Estimate file space usage", long_about = None)]
struct Args {
    /// files/directories to analyze
    #[arg(value_name = "file")]
    files: Vec<String>,

    /// maximum print depth
    #[arg(short, default_value_t = 1)]
    depth: usize,

    /// print bytes
    #[arg(short, default_value_t = false)]
    bytes: bool,

    /// sort sizes
    #[arg(long, default_value_t = false)]
    sort: bool,

    /// use .gitignore file for printing sizes
    #[arg(long, short, default_value_t = false)]
    ignore: bool,
}

struct Options {
    max_depth: usize,
    bytes: bool,
    sort: bool,
    ignore: bool,
}

#[derive(Debug)]
struct Entry {
    size: u64,
    path: PathBuf,
    hidden: bool,
    children: Vec<Entry>, // empty for files
}

fn main() {
    let mut args = Args::parse();
    let options = Options {
        max_depth: args.depth,
        bytes: args.bytes,
        sort: args.sort,
        ignore: args.ignore,
    };

    if args.files.is_empty() {
        args.files.push(".".to_string());
    }

    for path in &args.files {
        let (gitignore, _) = Gitignore::new(Path::new(path).join(".gitignore").as_path());

        if let Some(mut root_entry) = compute_size(path, &options, &gitignore) {
            // Step 2: print them according to options
            print_entry(&mut root_entry, &options, 0);
        }
    }
}

fn compute_size<P: AsRef<Path>>(
    path: P,
    options: &Options,
    gitignore: &Gitignore,
) -> Option<Entry> {
    let path = path.as_ref().to_path_buf();
    let meta = fs::symlink_metadata(&path).ok()?;

    let hidden = match options.ignore {
        true => {
            let mut hidden = gitignore.matched(&path, meta.is_dir()).is_ignore();

            if !hidden {
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;

                    hidden = meta.file_attributes() & 0x2 != 0
                }
                #[cfg(not(windows))]
                {
                    hidden = path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .map(|name| name.starts_with("."))
                        .unwrap_or(false);
                }
            }

            hidden
        }
        false => false,
    };

    if meta.is_file() {
        #[cfg(not(windows))]
        use std::os::unix::fs::MetadataExt;
        #[cfg(not(windows))]
        let size = meta.blocks() * 512;
        #[cfg(windows)]
        let size = get_size_on_disk(&path);

        return Some(Entry {
            size,
            path,
            children: vec![],
            hidden,
        });
    }

    if meta.is_dir() {
        let entries = fs::read_dir(&path).ok()?;

        let children: Vec<Entry> = entries
            .par_bridge()
            .filter_map(|res| res.ok())
            .filter_map(|entry| compute_size(entry.path(), options, gitignore))
            .collect();

        let total = children.iter().map(|c| c.size).sum();

        return Some(Entry {
            size: total,
            path,
            children,
            hidden,
        });
    }

    None
}

fn print_entry(entry: &mut Entry, options: &Options, depth: usize) {
    if options.max_depth < depth {
        return;
    }

    if options.sort {
        entry.children.sort_unstable_by(|a, b| a.size.cmp(&b.size));
    }

    for child in &mut entry.children.iter_mut().filter(|entry| !entry.hidden) {
        print_entry(child, options, depth + 1);
    }
    print_size(entry.size, entry.path.display(), options.bytes);
}

fn print_size<T: std::fmt::Display>(size: u64, path: T, print_bytes: bool) {
    if print_bytes {
        println!("{:<10} {}", size, path);
    } else {
        #[cfg(target_os = "linux")]
        println!(
            "{:<10} {}",
            humansize::format_size(size, humansize::BINARY),
            path
        );
        #[cfg(not(target_os = "linux"))]
        println!(
            "{:<10} {}",
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
