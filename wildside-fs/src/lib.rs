//! Shared filesystem helpers built on `cap-std` and `camino`.
#![forbid(unsafe_code)]

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8};
use std::io;
use std::path::Component;

/// Open a UTF-8 file path using ambient authority.
pub fn open_utf8_file(path: &Utf8Path) -> io::Result<fs_utf8::File> {
    fs_utf8::File::open_ambient(path, ambient_authority())
}

/// Resolve an ambient directory for the given path and return the directory with the file name.
pub fn open_dir_and_file(path: &Utf8Path) -> io::Result<(fs_utf8::Dir, String)> {
    let parent = path.parent().unwrap_or_else(|| Utf8Path::new("."));
    let file_name = path
        .file_name()
        .ok_or_else(|| io::Error::other("target should include a file name"))?
        .to_string();
    let dir = fs_utf8::Dir::open_ambient_dir(parent, ambient_authority())?;
    Ok((dir, file_name))
}

/// Ensure the parent directory for `path` exists, handling absolute paths safely for cap-std.
pub fn ensure_parent_dir(path: &Utf8Path) -> io::Result<()> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    if parent.as_os_str().is_empty() || parent == Utf8Path::new("/") {
        return Ok(());
    }

    let (base_dir, relative) = base_dir_and_relative(parent)?;
    if relative.as_os_str().is_empty() {
        return Ok(());
    }
    base_dir.create_dir_all(&relative)?;
    Ok(())
}

/// Return whether a path exists and is a regular file using capability-based IO.
pub fn file_is_file(path: &Utf8Path) -> io::Result<bool> {
    let (dir, name) = open_dir_and_file(path)?;
    dir.metadata(name.as_str()).map(|meta| meta.is_file())
}

/// Split an absolute or relative parent path into an ambient base directory and a relative suffix.
pub fn base_dir_and_relative(parent: &Utf8Path) -> io::Result<(fs_utf8::Dir, Utf8PathBuf)> {
    let std_parent = parent.as_std_path();

    let (base, relative) = match std_parent.components().next() {
        // Windows absolute path with a drive or UNC prefix.
        Some(Component::Prefix(prefix)) => {
            let prefix_str = prefix
                .as_os_str()
                .to_str()
                .ok_or_else(|| io::Error::other("non-UTF-8 path prefix"))?;

            let base = Utf8PathBuf::from(prefix_str).join(std::path::MAIN_SEPARATOR.to_string());
            let relative = std_parent
                .strip_prefix(base.as_std_path())
                .or_else(|_| std_parent.strip_prefix(prefix.as_os_str()))
                .map_err(|_| io::Error::other("failed to strip prefix from parent path"))?
                .to_path_buf();
            (base, relative)
        }
        // Unix-style absolute path.
        Some(Component::RootDir) => {
            let base = Utf8PathBuf::from(std::path::MAIN_SEPARATOR.to_string());
            let relative = std_parent
                .strip_prefix(base.as_std_path())
                .map_err(|_| io::Error::other("failed to strip root from absolute path"))?
                .to_path_buf();
            (base, relative)
        }
        // Relative path: resolve from the current directory.
        _ => (Utf8PathBuf::from("."), std_parent.to_path_buf()),
    };

    let dir = fs_utf8::Dir::open_ambient_dir(&base, ambient_authority())?;
    let relative = Utf8PathBuf::from_path_buf(relative)
        .map_err(|_| io::Error::other("non-UTF-8 parent path"))?;

    Ok((dir, relative))
}
