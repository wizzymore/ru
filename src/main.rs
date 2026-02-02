use clap::{Parser, crate_description, crate_version};
use colored::Colorize;
use ignore::gitignore::Gitignore;
use rayon::iter::*;
#[cfg(windows)]
use std::fs::Metadata;
use std::{
    fmt::Debug,
    fs,
    path::{Path, PathBuf},
};

#[derive(Parser, Debug)]
#[command(version = crate_version!(), about = crate_description!(), long_about = None, color = clap::ColorChoice::Always)]
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

    /// use .gitignore file for printing sizes
    #[arg(long, short = 'c', default_value_t = false)]
    no_color: bool,
}

struct Options {
    max_depth: usize,
    bytes: bool,
    sort: bool,
    ignore: bool,
}

#[derive(Debug)]
enum EntryKind {
    File(u64),
    Dir(Vec<Entry>),
}

#[derive(Debug)]
struct Entry {
    path: PathBuf,
    hidden: bool,
    kind: EntryKind,
}

impl Entry {
    fn size(&self) -> u64 {
        match &self.kind {
            &EntryKind::File(size) => size,
            EntryKind::Dir(children) => children.iter().map(|e| e.size()).sum(),
        }
    }
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

    if args.no_color {
        colored::control::set_override(false);
    }

    args.files
        .iter()
        .filter_map(|path| {
            let (gitignore, _) = Gitignore::new(Path::new(&path).join(".gitignore"));

            compute_size(path, &options, &gitignore)
        })
        .collect::<Vec<_>>()
        .iter_mut()
        .for_each(|root_entry| {
            print_entry(root_entry, &options, 0);
        });
}

fn compute_size<P: AsRef<Path>>(
    path: P,
    options: &Options,
    gitignore: &Gitignore,
) -> Option<Entry> {
    let path = path.as_ref();
    let meta = fs::symlink_metadata(path).ok()?;

    let hidden = if options.ignore {
        #[cfg(windows)]
        let hidden = gitignore.matched(path, meta.is_dir()).is_ignore() || is_hidden(&meta);
        #[cfg(not(windows))]
        let hidden = gitignore.matched(path, meta.is_dir()).is_ignore() || is_hidden(path);

        hidden
    } else {
        false
    };

    if meta.is_file() {
        let size;
        #[cfg(not(windows))]
        {
            use std::os::unix::fs::MetadataExt;
            size = meta.blocks() * 512;
        }
        #[cfg(windows)]
        {
            size = get_size_on_disk(path);
        }

        return Some(Entry {
            path: path.to_path_buf(),
            hidden,
            kind: EntryKind::File(size),
        });
    }

    if meta.is_dir() {
        let entries = fs::read_dir(path).ok()?;

        let children: Vec<Entry> = entries
            .par_bridge()
            .filter_map(|res| {
                let entry = res.ok()?;
                compute_size(entry.path(), options, gitignore)
            })
            .collect();

        return Some(Entry {
            path: path.to_path_buf(),
            hidden,
            kind: EntryKind::Dir(children),
        });
    }

    None
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
fn is_hidden(meta: &Metadata) -> bool {
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

fn print_entry(entry: &mut Entry, options: &Options, depth: usize) {
    if options.max_depth < depth {
        return;
    }

    if let EntryKind::Dir(children) = &mut entry.kind {
        if options.sort && children.len() > 1 {
            children.sort_unstable_by_key(|a| a.size());
        }

        for child in children {
            if !child.hidden {
                print_entry(child, options, depth + 1);
            }
        }
    }
    print_size(entry.size(), entry.path.display(), options.bytes);
}

fn print_size<T: std::fmt::Display>(size: u64, path: T, print_bytes: bool) {
    if print_bytes {
        println!("{size:<10} {path}");
    } else {
        #[cfg(target_os = "linux")]
        let options = humansize::BINARY;
        #[cfg(not(target_os = "linux"))]
        let options = humansize::DECIMAL;

        println!(
            "{:<10} {}",
            humansize::format_size(size, options.space_after_value(false))
                .to_string()
                .yellow()
                .bold(),
            path.to_string().cyan().bold()
        );
    }
}
