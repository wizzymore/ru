## ru

[<img alt="github" src="https://img.shields.io/badge/github-wizzymore/ru-8da0cb?style=for-the-badge&labelColor=555555&logo=github" height="20">](https://github.com/wizzymore/ru)
[<img alt="build status" src="https://img.shields.io/github/actions/workflow/status/wizzymore/ru/ci.yml?branch=master&style=for-the-badge" height="20">](https://github.com/wizzymore/ru/actions?query=branch%3Amaster)
[<img alt="crates.io" src="https://img.shields.io/crates/v/ru.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20">](https://crates.io/crates/ru)

ru is a simple and easy-to-use cli tool inspired by `du` to find out the disk usage of files/directories.

The motivation behind this project is to have a safe, performant and cross platform way to measure disk usage of folders and files.

### Usage

Install the latest prebuilt binary (Linux, macOS):

```shell
curl -fsSL https://releases.eptic.ro/ru.sh | sh
```

Or build it with cargo:

```shell
cargo install ru
```

Use the tool with the `ru` binary:

```shell
ru .
```

Use `--json` to print a JSON array instead of the default text output:

```shell
ru --json .
```

Each array entry has a `path` string and an integer `size_bytes` value. Paths use
the same display form as text output, and entries are sorted by path. The byte
count uses the same on-disk usage measurement as the text output. Depth and
`--ignore` options apply in JSON mode; `--bytes` and color options do not change
the JSON data.

Filesystem diagnostics are written to stderr. If traversal encounters an error,
`ru --json` still writes a valid JSON array to stdout and exits with a nonzero
status.

### Contributing

Issues are welcome, if you find a bug and want to report it I will be pleased to fix it if i have the time for it.  
PRs are not welcome, only the changes I want or need will be implemented, if you feel that this is not "right", fork it.
