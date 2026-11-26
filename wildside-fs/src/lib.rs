//! Shared filesystem helpers built on `cap-std` and `camino`.
#![forbid(unsafe_code)]

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8};
use std::ffi::OsString;
use std::io;
use std::path::{Component, Path, PathBuf};

/// Open a UTF-8 file path using ambient authority.
pub fn open_utf8_file(path: &Utf8Path) -> io::Result<fs_utf8::File> {
    fs_utf8::File::open_ambient(path, ambient_authority())
}

/// Resolve an ambient directory for the given path and return the directory with the file name.
pub fn open_dir_and_file(path: &Utf8Path) -> io::Result<(fs_utf8::Dir, String)> {
    let parent = match path.parent() {
        Some(parent) if !parent.as_str().is_empty() => parent,
        _ => Utf8Path::new("."),
    };
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

/// Maximum symlink hops followed when resolving the final component of a path.
const MAX_SYMLINK_HOPS: usize = 40;

/// Split `path` into an ambient base directory and the entry name inside it.
///
/// Native `std` paths are accepted, including ones that are not valid UTF-8, and
/// a bare file name (`popularity.bin`) resolves against the current directory.
fn native_dir_and_name(path: &Path) -> io::Result<(cap_std::fs::Dir, OsString)> {
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::other("target should include a file name"))?
        .to_owned();
    let parent = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let dir = cap_std::fs::Dir::open_ambient_dir(parent, ambient_authority())?;
    Ok((dir, name))
}

/// Resolve `path` to the directory and entry name that hold its final target.
///
/// A symlink as the last component is followed even when it leads outside the
/// directory that holds it, as `std::fs` follows it: each hop is re-opened as an
/// ambient directory of its own, so the caller keeps the ambient authority the
/// helpers already take. At most [`MAX_SYMLINK_HOPS`] links are followed.
fn resolve_target(path: &Path) -> io::Result<(cap_std::fs::Dir, OsString)> {
    let mut current = path.to_path_buf();
    for _ in 0..MAX_SYMLINK_HOPS {
        let (dir, name) = native_dir_and_name(&current)?;
        let is_link = dir
            .symlink_metadata(&name)
            .map(|meta| meta.file_type().is_symlink())
            .unwrap_or(false);
        if !is_link {
            return Ok((dir, name));
        }
        let target = dir.read_link_contents(&name)?;
        current = current
            .parent()
            .map_or_else(|| target.clone(), |parent| parent.join(&target));
    }
    Err(io::Error::other("too many levels of symbolic links"))
}

/// Create (or truncate) the file at `path` using ambient authority.
pub fn create_file(path: impl AsRef<Path>) -> io::Result<fs_utf8::File> {
    let (dir, name) = resolve_target(path.as_ref())?;
    dir.create(&name).map(fs_utf8::File::from_cap_std)
}

/// Open the existing file at `path` for reading using ambient authority.
pub fn open_file(path: impl AsRef<Path>) -> io::Result<fs_utf8::File> {
    let (dir, name) = resolve_target(path.as_ref())?;
    dir.open(&name).map(fs_utf8::File::from_cap_std)
}

/// Write `contents` to the file at `path`, creating or truncating it.
pub fn write_file(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> io::Result<()> {
    let (dir, name) = resolve_target(path.as_ref())?;
    dir.write(&name, contents)
}

/// Read the whole file at `path` into a byte vector.
pub fn read_file(path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
    let (dir, name) = resolve_target(path.as_ref())?;
    dir.read(&name)
}

/// Read the whole file at `path` into a string.
pub fn read_to_string(path: impl AsRef<Path>) -> io::Result<String> {
    let (dir, name) = resolve_target(path.as_ref())?;
    dir.read_to_string(&name)
}

/// Create the directory at `path` and any missing ancestors.
///
/// Each missing component is created inside its parent, which is opened by
/// ambient path, so `..` and directory symlinks that lead outside the current
/// directory resolve as they do for `std::fs::create_dir_all`.
pub fn create_dir_all(path: impl AsRef<Path>) -> io::Result<()> {
    let path = path.as_ref();
    if path.as_os_str().is_empty() {
        return Ok(());
    }
    match cap_std::fs::Dir::open_ambient_dir(path, ambient_authority()) {
        Ok(_) => return Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => return Err(err),
    }
    if let Some(parent) = path.parent() {
        create_dir_all(parent)?;
    }
    let (dir, name) = native_dir_and_name(path)?;
    match dir.create_dir(&name) {
        Err(err) if err.kind() != io::ErrorKind::AlreadyExists => Err(err),
        Ok(()) | Err(_) => Ok(()),
    }
}

/// List the paths of the entries directly inside the directory `dir`.
pub fn list_dir(dir: impl AsRef<Path>) -> io::Result<Vec<PathBuf>> {
    let dir = dir.as_ref();
    let handle = cap_std::fs::Dir::open_ambient_dir(dir, ambient_authority())?;
    handle
        .entries()?
        .map(|entry| entry.map(|entry| dir.join(entry.file_name())))
        .collect()
}

/// Remove the file at `path`, or the symlink itself when it is one.
pub fn remove_file(path: impl AsRef<Path>) -> io::Result<()> {
    let (dir, name) = native_dir_and_name(path.as_ref())?;
    dir.remove_file(&name)
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

#[cfg(test)]
mod tests {
    //! Unit coverage for the native-path helpers.

    use std::ffi::OsStr;
    use std::path::Path;

    use super::*;
    use tempfile::TempDir;

    fn temp() -> TempDir {
        match TempDir::new() {
            Ok(dir) => dir,
            Err(err) => panic!("create temporary directory: {err}"),
        }
    }

    #[test]
    fn a_bare_file_name_resolves_against_the_current_directory() {
        let (_dir, name) = native_dir_and_name(Path::new("popularity.bin"))
            .unwrap_or_else(|err| panic!("a bare name should resolve: {err}"));
        assert_eq!(name, OsStr::new("popularity.bin"), "the name is kept whole");
        let (_dir, name) = open_dir_and_file(Utf8Path::new("popularity.bin"))
            .unwrap_or_else(|err| panic!("a bare UTF-8 name should resolve: {err}"));
        assert_eq!(name, "popularity.bin", "the UTF-8 name is kept whole");
    }

    #[test]
    fn a_file_round_trips_through_write_and_read() {
        let dir = temp();
        let path = dir.path().join("data.bin");
        write_file(&path, b"bytes").unwrap_or_else(|err| panic!("write: {err}"));
        let read = read_file(&path).unwrap_or_else(|err| panic!("read: {err}"));
        assert_eq!(read, b"bytes", "the bytes written are the bytes read");
    }

    #[cfg(unix)]
    #[test]
    fn a_non_utf8_directory_name_is_accepted() {
        use std::os::unix::ffi::OsStrExt;

        let dir = temp();
        let odd = dir.path().join(OsStr::from_bytes(b"out\xff"));
        create_dir_all(&odd).unwrap_or_else(|err| panic!("create non-UTF-8 dir: {err}"));
        write_file(odd.join("file"), b"x").unwrap_or_else(|err| panic!("write: {err}"));
        assert!(
            read_file(odd.join("file")).is_ok(),
            "a file in a non-UTF-8 directory should read back"
        );
        remove_file(odd.join("file")).unwrap_or_else(|err| panic!("remove: {err}"));
    }

    #[test]
    fn a_relative_path_may_climb_out_of_the_current_directory() {
        // Tests run with the crate directory as the current one, so `..` is the
        // workspace root: a scratch directory made there is reached as `../name`.
        let scratch = TempDir::new_in("..").unwrap_or_else(|err| panic!("scratch dir: {err}"));
        let Some(name) = scratch.path().file_name() else {
            panic!("scratch dir has a name")
        };
        let relative = Path::new("..").join(name).join("nested/dir");
        create_dir_all(&relative).unwrap_or_else(|err| panic!("create via ..: {err}"));
        assert!(
            scratch.path().join("nested/dir").is_dir(),
            "the directory should exist under the scratch dir"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_directory_symlink_leading_outside_is_followed_when_creating() {
        let outside = temp();
        let inside = temp();
        let handle = cap_std::fs::Dir::open_ambient_dir(inside.path(), ambient_authority())
            .unwrap_or_else(|err| panic!("open dir: {err}"));
        handle
            .symlink_contents(outside.path(), "link")
            .unwrap_or_else(|err| panic!("symlink: {err}"));
        create_dir_all(inside.path().join("link/sub/dir"))
            .unwrap_or_else(|err| panic!("create through link: {err}"));
        assert!(
            outside.path().join("sub/dir").is_dir(),
            "the directory should be created at the link's target"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_leading_outside_its_directory_is_followed() {
        let target_dir = temp();
        let link_dir = temp();
        let target = target_dir.path().join("real.bin");
        write_file(&target, b"payload").unwrap_or_else(|err| panic!("write: {err}"));
        let handle = cap_std::fs::Dir::open_ambient_dir(link_dir.path(), ambient_authority())
            .unwrap_or_else(|err| panic!("open link dir: {err}"));
        handle
            .symlink_contents(&target, "link.bin")
            .unwrap_or_else(|err| panic!("symlink: {err}"));
        let read = read_file(link_dir.path().join("link.bin"))
            .unwrap_or_else(|err| panic!("read through link: {err}"));
        assert_eq!(read, b"payload", "the link target's contents are read");
    }
}
