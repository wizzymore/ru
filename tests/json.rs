mod common;

use common::{TempDir, arg, run};
use std::fs;

#[test]
fn json_is_sorted_and_respects_depth_and_ignore() {
    let temp = TempDir::new();
    let root = temp.path().join("tree");
    fs::create_dir_all(root.join("b")).unwrap();
    fs::create_dir_all(root.join("a").join("child")).unwrap();
    fs::create_dir_all(root.join("ignored-dir").join("child")).unwrap();
    fs::write(root.join(".gitignore"), "ignored-dir/\n").unwrap();
    fs::write(root.join("a").join("file"), "a").unwrap();

    let root_arg = root.as_os_str();
    let output = run(&[
        arg("--json"),
        arg("--ignore"),
        arg("-d"),
        arg("1"),
        root_arg,
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let entries: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).unwrap();
    let paths: Vec<String> = entries
        .iter()
        .map(|entry| {
            let object = entry.as_object().unwrap();
            assert_eq!(object.len(), 2);
            assert!(object["size_bytes"].as_u64().is_some());
            object["path"].as_str().unwrap().to_owned()
        })
        .collect();

    let mut sorted = paths.clone();
    sorted.sort_unstable();
    assert_eq!(paths, sorted);
    assert_eq!(
        paths,
        vec![
            root.display().to_string(),
            root.join("a").display().to_string(),
            root.join("b").display().to_string()
        ]
    );
}

#[test]
fn bytes_and_color_options_do_not_change_json() {
    let temp = TempDir::new();
    let file = temp.path().join("file.txt");
    fs::write(&file, b"data").unwrap();

    let json_output = run(&[arg("--json"), file.as_os_str()]);
    let json_with_display_options = run(&[
        arg("--json"),
        arg("--bytes"),
        arg("--no-color"),
        file.as_os_str(),
    ]);
    assert!(json_output.status.success());
    assert!(json_with_display_options.status.success());
    assert_eq!(json_output.stdout, json_with_display_options.stdout);

    let entries: Vec<serde_json::Value> = serde_json::from_slice(&json_output.stdout).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["path"], file.display().to_string());
    assert!(entries[0]["size_bytes"].as_u64().is_some());
}

#[cfg(unix)]
#[test]
fn json_serializes_special_path_characters() {
    let temp = TempDir::new();
    let file = temp.path().join("quote\" backslash\\ newline\n.txt");
    fs::write(&file, b"").unwrap();

    let output = run(&[arg("--json"), file.as_os_str()]);
    assert!(output.status.success());
    let entries: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(entries[0]["path"], file.display().to_string());
    assert!(String::from_utf8_lossy(&output.stdout).contains("\\n"));
}

#[test]
fn errors_keep_stdout_valid_json_and_return_failure() {
    let temp = TempDir::new();
    let missing = temp.path().join("missing");

    let output = run(&[arg("--json"), missing.as_os_str()]);
    assert!(!output.status.success());
    let entries: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert!(entries.is_empty());
    assert!(!output.stderr.is_empty());
}

#[test]
fn default_text_output_keeps_its_existing_format() {
    let temp = TempDir::new();
    let file = temp.path().join("empty");

    fs::write(&file, b"").unwrap();
    let json = run(&[arg("--json"), arg("--bytes"), file.as_os_str()]);
    let entries: Vec<serde_json::Value> = serde_json::from_slice(&json.stdout).unwrap();
    let size_bytes = entries[0]["size_bytes"].as_u64().unwrap();

    let text = run(&[arg("--bytes"), arg("--no-color"), file.as_os_str()]);
    assert!(text.status.success());
    assert_eq!(
        String::from_utf8(text.stdout).unwrap(),
        format!("{size_bytes:<10} {}\n", file.display())
    );

    let default_text = run(&[file.as_os_str()]);
    assert!(default_text.status.success());
    assert_eq!(
        String::from_utf8(default_text.stdout).unwrap(),
        format!("{:<10} {}\n", "0", file.display())
    );
}
