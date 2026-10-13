mod common;

use common::{TempDir, arg, command, run};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Output;

fn byte_entries(output: Output) -> BTreeMap<String, u64> {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        !stdout.contains('\x1b'),
        "redirected output should not contain ANSI escapes"
    );
    let mut entries = BTreeMap::new();
    for line in stdout.lines() {
        let (size, path) = line.split_once(' ').expect("expected a size and a path");
        let size: u64 = size.parse().expect("expected a byte count");
        let path = path.trim_start();
        assert_eq!(line, format!("{size:<10} {path}"));
        assert!(
            entries.insert(path.to_owned(), size).is_none(),
            "duplicate path: {path}"
        );
    }
    entries
}

#[test]
fn omitted_path_analyzes_the_current_directory() {
    let temp = TempDir::new();
    fs::create_dir(temp.path().join("sub")).unwrap();
    fs::write(temp.path().join("sub/file"), b"data").unwrap();

    let implicit = byte_entries(
        command(&[arg("-b")])
            .current_dir(temp.path())
            .output()
            .unwrap(),
    );
    let explicit = byte_entries(
        command(&[arg("-b"), arg(".")])
            .current_dir(temp.path())
            .output()
            .unwrap(),
    );
    assert_eq!(implicit, explicit);
    assert_eq!(implicit.len(), 2);
    assert!(implicit.contains_key("."));
    assert!(implicit.contains_key(&Path::new(".").join("sub").display().to_string()));
}

#[test]
fn directories_are_printed_but_child_files_are_only_included_in_totals() {
    let temp = TempDir::new();
    let root = temp.path().join("tree with spaces");
    let child = root.join("child");
    fs::create_dir_all(&child).unwrap();
    let file = child.join("file with spaces");
    fs::write(&file, vec![1; 8192]).unwrap();

    let entries = byte_entries(run(&[arg("-b"), root.as_os_str()]));
    assert_eq!(entries.len(), 2);
    assert!(entries.contains_key(&root.display().to_string()));
    assert!(entries.contains_key(&child.display().to_string()));
    let file_entries = byte_entries(run(&[arg("-b"), file.as_os_str()]));
    assert_eq!(file_entries.len(), 1);
    assert!(file_entries.contains_key(&file.display().to_string()));

    #[cfg(unix)]
    {
        assert_eq!(
            file_entries[&file.display().to_string()],
            allocated_bytes(&file)
        );
        assert_eq!(
            entries[&child.display().to_string()],
            allocated_bytes(&child) + allocated_bytes(&file)
        );
        assert_eq!(
            entries[&root.display().to_string()],
            allocated_bytes(&root) + allocated_bytes(&child) + allocated_bytes(&file)
        );
    }
}

#[test]
fn depth_limits_printing_without_changing_totals() {
    let temp = TempDir::new();
    let root = temp.path().join("tree");
    let child = root.join("child");
    let grandchild = child.join("grandchild");
    fs::create_dir_all(&grandchild).unwrap();
    let file = grandchild.join("file");
    fs::write(&file, vec![1; 8192]).unwrap();

    let full = byte_entries(run(&[arg("-b"), root.as_os_str()]));
    assert_eq!(full.len(), 3);
    for (depth, expected_paths) in [("0", vec![&root]), ("1", vec![&root, &child])] {
        let limited = byte_entries(run(&[arg("-b"), arg("-d"), arg(depth), root.as_os_str()]));
        let expected = expected_paths
            .into_iter()
            .map(|path| {
                let path = path.display().to_string();
                let size = full[&path];
                (path, size)
            })
            .collect();
        assert_eq!(limited, expected);
    }
    let explicit_file = byte_entries(run(&[arg("-b"), arg("-d"), arg("0"), file.as_os_str()]));
    assert_eq!(explicit_file.len(), 1);
}

#[test]
fn ignore_filters_gitignore_matches_and_keeps_visible_directories() {
    let temp = TempDir::new();
    let root = temp.path().join("tree");
    let visible = root.join("visible");
    let ignored = root.join("ignored");
    fs::create_dir_all(&visible).unwrap();
    fs::create_dir_all(&ignored).unwrap();
    fs::write(root.join(".gitignore"), "ignored/\n*.skip\n").unwrap();
    fs::write(visible.join("file"), b"data").unwrap();
    fs::write(visible.join("large.skip"), vec![1; 16384]).unwrap();
    fs::write(ignored.join("file"), vec![1; 16384]).unwrap();

    let all = byte_entries(run(&[arg("-b"), root.as_os_str()]));
    assert_eq!(all.len(), 3);
    let filtered = byte_entries(run(&[arg("-b"), arg("--ignore"), root.as_os_str()]));
    assert_eq!(filtered.len(), 2);
    assert!(filtered.contains_key(&root.display().to_string()));
    assert!(filtered.contains_key(&visible.display().to_string()));
    assert!(!filtered.contains_key(&ignored.display().to_string()));
    #[cfg(unix)]
    {
        let visible_size = allocated_bytes(&visible) + allocated_bytes(&visible.join("file"));
        assert_eq!(filtered[&visible.display().to_string()], visible_size);
        assert_eq!(
            filtered[&root.display().to_string()],
            allocated_bytes(&root) + visible_size
        );
    }
}

#[test]
fn multiple_roots_and_byte_and_color_flags_keep_the_same_text() {
    let temp = TempDir::new();
    let first = temp.path().join("first");
    let second = temp.path().join("second");
    fs::write(&first, b"first").unwrap();
    fs::write(&second, b"second").unwrap();
    let short = run(&[arg("-b"), first.as_os_str(), second.as_os_str()]);
    let long = run(&[
        arg("--bytes"),
        arg("--no-color"),
        first.as_os_str(),
        second.as_os_str(),
    ]);
    assert_eq!(short.stdout, long.stdout);
    assert!(long.status.success());
    let entries = byte_entries(short);
    assert_eq!(entries.len(), 2);
    assert!(entries.contains_key(&first.display().to_string()));
    assert!(entries.contains_key(&second.display().to_string()));
}

#[test]
fn missing_paths_report_to_stderr_and_keep_the_existing_text_exit_status() {
    let temp = TempDir::new();
    let missing = temp.path().join("missing");
    let valid = temp.path().join("valid");
    fs::write(&valid, b"").unwrap();

    let error_only = run(&[missing.as_os_str()]);
    assert!(error_only.status.success());
    assert!(error_only.stdout.is_empty());
    assert!(String::from_utf8_lossy(&error_only.stderr).contains(&missing.display().to_string()));

    let mixed = run(&[arg("-b"), missing.as_os_str(), valid.as_os_str()]);
    assert!(mixed.status.success());
    assert_eq!(
        String::from_utf8(mixed.stdout).unwrap(),
        format!("{:<10} {}\n", 0, valid.display())
    );
    assert!(String::from_utf8_lossy(&mixed.stderr).contains(&missing.display().to_string()));
}

#[cfg(unix)]
fn allocated_bytes(path: &Path) -> u64 {
    use std::os::unix::fs::MetadataExt;
    fs::symlink_metadata(path).unwrap().blocks() * 512
}

#[cfg(unix)]
#[test]
fn ignore_skips_hidden_files_and_directories() {
    let temp = TempDir::new();
    let root = temp.path().join("tree");
    let hidden = root.join(".hidden");
    fs::create_dir_all(&hidden).unwrap();
    fs::write(root.join(".hidden-file"), vec![1; 8192]).unwrap();
    fs::write(hidden.join("file"), vec![1; 8192]).unwrap();

    let all = byte_entries(run(&[arg("-b"), root.as_os_str()]));
    assert_eq!(all.len(), 2);
    let filtered = byte_entries(run(&[arg("-b"), arg("-i"), root.as_os_str()]));
    assert_eq!(
        filtered,
        BTreeMap::from([(root.display().to_string(), allocated_bytes(&root))])
    );
}

#[cfg(unix)]
#[test]
fn symlinks_are_counted_without_following_their_targets() {
    use std::os::unix::fs::symlink;
    let temp = TempDir::new();
    let root = temp.path().join("tree");
    let target = temp.path().join("target");
    fs::create_dir(&root).unwrap();
    fs::create_dir(&target).unwrap();
    fs::write(target.join("file"), vec![1; 16384]).unwrap();
    let link = root.join("link");
    symlink(&target, &link).unwrap();

    let entries = byte_entries(run(&[arg("-b"), root.as_os_str()]));
    assert_eq!(
        entries,
        BTreeMap::from([(
            root.display().to_string(),
            allocated_bytes(&root) + allocated_bytes(&link)
        )])
    );
    let explicit = byte_entries(run(&[arg("-b"), link.as_os_str()]));
    assert_eq!(
        explicit,
        BTreeMap::from([(link.display().to_string(), allocated_bytes(&link))])
    );
}

#[cfg(target_os = "linux")]
#[test]
fn hard_links_are_counted_once_per_root() {
    let temp = TempDir::new();
    let first = temp.path().join("first");
    let second = temp.path().join("second");
    fs::write(&first, vec![1; 8192]).unwrap();
    fs::hard_link(&first, &second).unwrap();

    let directory = byte_entries(run(&[arg("-b"), temp.path().as_os_str()]));
    assert_eq!(
        directory,
        BTreeMap::from([(
            temp.path().display().to_string(),
            allocated_bytes(temp.path()) + allocated_bytes(&first)
        )])
    );
    let roots = byte_entries(run(&[arg("-b"), first.as_os_str(), second.as_os_str()]));
    assert_eq!(roots.len(), 2);
    assert_eq!(roots[&first.display().to_string()], allocated_bytes(&first));
    assert_eq!(
        roots[&second.display().to_string()],
        allocated_bytes(&second)
    );
}
