//! Filesystem manipulation operations.
//!
//! This module contains basic methods to manipulate the contents of the brain's
//! micro SDCard card. All methods in this module represent VEXos filesystem
//! operations.
//!
//! # VEXos Limitations
//!
//! While this module largely mimicks Rust's `std::fs` API, there are several major
//! limitations in the VEXos filesystem. This module only provides a small subset of
//! what would normally be expected in a typical Rust enviornment. Notably:
//!
//! - Files cannot be opened as read and write at the same time (only one). To read a file that you’ve written to, you’ll need to drop your written file descriptor and reopen it as readonly.
//! - Files can be created, but not deleted or renamed.
//! - Directories cannot be created or enumerated from the Brain, only top-level files.

use alloc::{ffi::CString, string::String, vec::Vec};

use no_std_io::io::{Read, Write};

use crate::{io, path::Path};

mod fs_str;

pub use fs_str::FsStr;

/// Options and flags which can be used to configure how a file is opened.
///
/// This builder exposes the ability to configure how a [`File`] is opened and
/// what operations are permitted on the open file. The [`File::open`] and
/// [`File::create`] methods are aliases for commonly used options using this
/// builder.
///
/// Generally speaking, when using `OpenOptions`, you'll first call
/// [`OpenOptions::new`], then chain calls to methods to set each option, then
/// call [`OpenOptions::open`], passing the path of the file you're trying to
/// open. This will give you a [`io::Result`] with a [`File`] inside that you
/// can further operate on.
///
/// # Limitations
///
/// - Files MUST be opened in either `read` XOR `write` mode.
/// - VEXos does not allow you to open a file configured as `read` and `write`
///   at the same time. Doing so will return an error with `File::open`. This is
///   a fundamental limtiation of the OS.
///
/// # Examples
///
/// Opening a file to read:
///
/// ```no_run
/// use vexide::core::fs::OpenOptions;
///
/// let file = OpenOptions::new().read(true).open("foo.txt");
/// ```
///
/// Opening a file for writing, as well as creating it if it doesn't exist:
///
/// ```no_run
/// use vexide::core::fs::OpenOptions;
///
/// let file = OpenOptions::new()
///             .write(true)
///             .create(true)
///             .open("foo.txt");
/// ```
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, Default)]
pub struct OpenOptions {
    read: bool,
    write: bool,
    append: bool,
    truncate: bool,
    create_new: bool,
}

impl OpenOptions {
    /// Creates a blank new set of options ready for configuration.
    ///
    /// All options are initially set to `false`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use vexide::core::fs::OpenOptions;
    ///
    /// let mut options = OpenOptions::new();
    /// let file = options.read(true).open("foo.txt");
    /// ```
    #[must_use]
    pub const fn new() -> OpenOptions {
        OpenOptions {
            read: false,
            write: false,
            append: false,
            truncate: false,
            create_new: false,
        }
    }

    /// Sets the option for read access.
    ///
    /// This option, when true, will indicate that the file should be
    /// `read`-able if opened.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use vexide::core::fs::OpenOptions;
    ///
    /// let file = OpenOptions::new().read(true).open("foo.txt");
    /// ```
    pub fn read(&mut self, read: bool) -> &mut Self {
        self.read = read;
        self
    }

    /// Sets the option for write access.
    ///
    /// This option, when true, will indicate that the file should be
    /// `write`-able if opened.
    ///
    /// If the file already exists, any write calls on it will overwrite its
    /// contents, without truncating it.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use vexide::core::fs::OpenOptions;
    ///
    /// let file = OpenOptions::new().write(true).open("foo.txt");
    /// ```
    pub fn write(&mut self, write: bool) -> &mut Self {
        self.write = write;
        self
    }

    /// Sets the option for the append mode.
    ///
    /// This option, when true, means that writes will append to a file instead
    /// of overwriting previous contents.
    /// Note that setting `.write(true).append(true)` has the same effect as
    /// setting only `.append(true)`.
    ///
    /// Append mode guarantees that writes will be positioned at the current end of file,
    /// even when there are other processes or threads appending to the same file. This is
    /// unlike <code>[seek]\([SeekFrom]::[End]\(0))</code> followed by `write()`, which
    /// has a race between seeking and writing during which another writer can write, with
    /// our `write()` overwriting their data.
    ///
    /// Keep in mind that this does not necessarily guarantee that data appended by
    /// different processes or threads does not interleave. The amount of data accepted a
    /// single `write()` call depends on the operating system and file system. A
    /// successful `write()` is allowed to write only part of the given data, so even if
    /// you're careful to provide the whole message in a single call to `write()`, there
    /// is no guarantee that it will be written out in full. If you rely on the filesystem
    /// accepting the message in a single write, make sure that all data that belongs
    /// together is written in one operation. This can be done by concatenating strings
    /// before passing them to [`write()`].
    ///
    /// ## Note
    ///
    /// This function doesn't create the file if it doesn't exist. Use the
    /// [`OpenOptions::create`] method to do so.
    ///
    /// [`write()`]: Write::write "io::Write::write"
    /// [`flush()`]: Write::flush "io::Write::flush"
    /// [stream_position]: Seek::stream_position "io::Seek::stream_position"
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::fs::OpenOptions;
    ///
    /// let file = OpenOptions::new().append(true).open("foo.txt");
    /// ```
    pub fn append(&mut self, append: bool) -> &mut Self {
        self.append = append;
        self
    }

    /// Sets the option for truncating a previous file.
    ///
    /// If a file is successfully opened with this option set it will truncate
    /// the file to 0 length if it already exists.
    ///
    /// The file must be opened with write access for truncate to work.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use vexide::core::fs::OpenOptions;
    ///
    /// let file = OpenOptions::new().write(true).truncate(true).open("foo.txt");
    /// ```
    pub fn truncate(&mut self, truncate: bool) -> &mut Self {
        self.truncate = truncate;
        self
    }

    /// Sets the option to create a new file, or open it if it already exists.
    ///
    /// In order for the file to be created, [`OpenOptions::write`] or
    /// [`OpenOptions::append`] access must be used.
    ///
    /// See also [`write()`][self::write] for a simple function to create a file
    /// with some given data.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use vexide::core::fs::OpenOptions;
    ///
    /// let file = OpenOptions::new().write(true).create(true).open("foo.txt");
    /// ```
    pub fn create(&mut self, create: bool) -> &mut Self {
        self.write = create;
        self
    }

    /// Sets the option to create a new file, failing if it already exists.
    ///
    /// No file is allowed to exist at the target location. In this way, if the call succeeds,
    /// the file returned is guaranteed to be new. If a file exists at the target location,
    /// creating a new file will fail with [`AlreadyExists`] or another error based on the
    /// situation. See [`OpenOptions::open`] for a non-exhaustive list of likely errors.
    ///
    /// If `.create_new(true)` is set, [`.create()`] and [`.truncate()`] are
    /// ignored.
    ///
    /// The file must be opened with write or append access in order to create
    /// a new file.
    ///
    /// [`.create()`]: OpenOptions::create
    /// [`.truncate()`]: OpenOptions::truncate
    /// [`AlreadyExists`]: io::ErrorKind::AlreadyExists
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use vexide::core::fs::OpenOptions;
    ///
    /// let file = OpenOptions::new().write(true)
    ///                              .create_new(true)
    ///                              .open("foo.txt");
    /// ```
    pub fn create_new(&mut self, create_new: bool) -> &mut Self {
        self.create_new = create_new;
        self
    }

    /// Opens a file at `path` with the options specified by `self`.
    ///
    /// # Errors
    ///
    /// This function will return an error under a number of different
    /// circumstances. Some of these error conditions are listed here, together
    /// with their [`io::ErrorKind`]. The mapping to [`io::ErrorKind`]s is not
    /// part of the compatibility contract of the function.
    ///
    /// * [`NotFound`]: The specified file does not exist and neither `create`
    ///   or `create_new` is set.
    /// * [`AlreadyExists`]: `create_new` was specified and the file already
    ///   exists.
    /// * [`InvalidInput`]: Invalid combinations of open options (read/write
    ///   access both specified, truncate without write access, no access mode
    ///   set, etc.).
    ///
    /// The following errors don't match any existing [`io::ErrorKind`] at the moment:
    /// * Filesystem-level errors: full disk, write permission
    ///   requested on a read-only file system, exceeded disk quota, too many
    ///   open files, too long filename.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use vexide::core::fs::OpenOptions;
    ///
    /// let file = OpenOptions::new().read(true).open("foo.txt");
    /// ```
    ///
    /// [`AlreadyExists`]: io::ErrorKind::AlreadyExists
    /// [`InvalidInput`]: io::ErrorKind::InvalidInput
    /// [`NotFound`]: io::ErrorKind::NotFound
    /// [`PermissionDenied`]: io::ErrorKind::PermissionDenied
    pub fn open<P: AsRef<Path>>(&self, path: P) -> io::Result<File> {
        // Mount sdcard volume as FAT filesystem
        map_fresult(unsafe { vex_sdk::vexFileMountSD() })?;

        let path = path.as_ref();

        let path = CString::new(path.as_fs_str().as_encoded_bytes()).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "Path contained a null byte")
        })?;

        if self.write && self.read {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Files cannot be opened with read and write access",
            ));
        }
        if self.create_new {
            let file_exists = unsafe { vex_sdk::vexFileStatus(path.as_ptr()) };
            if file_exists != 0 {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "File already exists",
                ));
            }
        }

        let file = if self.read && !self.write {
            // The second argument to this function is ignored.
            // Open in read only mode
            unsafe { vex_sdk::vexFileOpen(path.as_ptr(), c"".as_ptr()) }
        } else if self.write && self.append {
            // Open in read/write and append mode
            unsafe { vex_sdk::vexFileOpenWrite(path.as_ptr()) }
        } else if self.write && self.truncate {
            // Open in read/write mode
            unsafe { vex_sdk::vexFileOpenCreate(path.as_ptr()) }
        } else if self.write {
            // Open in read/write and overwrite mode
            unsafe {
                // Open in read/write and append mode
                let fd = vex_sdk::vexFileOpenWrite(path.as_ptr());
                // Seek to beginning of the file
                vex_sdk::vexFileSeek(fd, 0, 0);

                fd
            }
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Files cannot be opened without read or write access",
            ));
        };

        if file.is_null() {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                "Could not open file",
            ))
        } else {
            Ok(File {
                fd: file,
                write: self.write,
            })
        }
    }
}

pub struct FileType {
    is_dir: bool,
}

pub struct Metadata {
    is_dir: bool,
    size: u64,
}

impl Metadata {
    fn from_fd(fd: *mut vex_sdk::FIL) -> io::Result<Self> {
        let size = unsafe { vex_sdk::vexFileSize(fd) };

        if size >= 0 {
            Ok(Self {
                size: size as u64,
                is_dir: false,
            })
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Failed to get file size",
            ))
        }
    }

    fn from_path(path: &Path) -> io::Result<Self> {
        let c_path = CString::new(path.as_fs_str().as_encoded_bytes()).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "Path contained a null byte")
        })?;

        let file_type = unsafe { vex_sdk::vexFileStatus(c_path.as_ptr()) };
        let is_dir = file_type == 3;

        // We can't get the size if its a directory because we cant open it as a file
        if is_dir {
            Ok(Self {
                size: 0,
                is_dir: true,
            })
        } else {
            let mut opts = OpenOptions::new();
            opts.read(true);
            let file = opts.open(path)?;
            let fd = file.fd;

            Self::from_fd(fd)
        }
    }

    pub fn is_dir(&self) -> bool {
        self.is_dir
    }
    pub fn is_file(&self) -> bool {
        !self.is_dir
    }
    pub fn is_symlink(&self) -> bool {
        false
    }
    pub fn len(&self) -> Option<u64> {
        self.is_dir.then(|| self.size)
    }
}

/// Represents a file in the file system.
pub struct File {
    fd: *mut vex_sdk::FIL,
    write: bool,
}
impl File {
    fn flush(&self) {
        unsafe {
            vex_sdk::vexFileSync(self.fd);
        }
    }

    /// Opens a file in read-only mode.
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        OpenOptions::new().read(true).open(path.as_ref())
    }

    /// Opens or creates a file in write-only mode.
    /// Files cannot be read from in this mode.
    pub fn create<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path.as_ref())
    }
    /// Creates a file in write-only mode, erroring if the file already exists.
    /// Files cannot be read from in this mode.
    pub fn create_new<P: AsRef<Path>>(path: P) -> io::Result<File> {
        OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(path.as_ref())
    }

    #[must_use]
    pub fn options() -> OpenOptions {
        OpenOptions::new()
    }

    pub fn metadata(&self) -> io::Result<Metadata> {
        Metadata::from_fd(self.fd)
    }

    pub fn sync_all(&self) -> io::Result<()> {
        self.flush();
        Ok(())
    }
    pub fn sync_data(&self) -> io::Result<()> {
        self.flush();
        Ok(())
    }
}
impl io::Write for File {
    fn write(&mut self, buf: &[u8]) -> no_std_io::io::Result<usize> {
        if !self.write {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Files opened in read mode cannot be written to.",
            ));
        }

        let len = buf.len();
        let buf_ptr = buf.as_ptr();
        let written =
            unsafe { vex_sdk::vexFileWrite(buf_ptr.cast_mut().cast(), 1, len as _, self.fd) };
        if written < 0 {
            Err(io::Error::new(
                io::ErrorKind::Other,
                "Could not write to file",
            ))
        } else {
            Ok(written as usize)
        }
    }

    fn flush(&mut self) -> no_std_io::io::Result<()> {
        File::flush(self);
        Ok(())
    }
}
impl io::Read for File {
    fn read(&mut self, buf: &mut [u8]) -> no_std_io::io::Result<usize> {
        if self.write {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Files opened in write mode cannot be read from.",
            ));
        }

        let len = buf.len() as _;
        let buf_ptr = buf.as_mut_ptr();
        let read = unsafe { vex_sdk::vexFileRead(buf_ptr.cast(), 1, len, self.fd) };
        if read < 0 {
            Err(io::Error::new(
                io::ErrorKind::Other,
                "Could not read from file",
            ))
        } else {
            Ok(read as usize)
        }
    }
}

fn map_fresult(fresult: vex_sdk::FRESULT) -> io::Result<()> {
    // VEX presumably uses a derivative of FatFs (most likely the xilffs library)
    // for sdcard filesystem functions.
    //
    // Documentation for each FRESULT originates from here:
    // <http://elm-chan.org/fsw/ff/doc/rc.html>
    match fresult {
        vex_sdk::FRESULT::FR_OK => Ok(()),
        vex_sdk::FRESULT::FR_DISK_ERR => Err(io::Error::new(
            io::ErrorKind::Uncategorized,
            "internal function reported an unrecoverable hard error",
        )),
        vex_sdk::FRESULT::FR_INT_ERR => Err(io::Error::new(
            io::ErrorKind::Uncategorized,
            "assertion failed and an insanity is detected in the internal process",
        )),
        vex_sdk::FRESULT::FR_NOT_READY => Err(io::Error::new(
            io::ErrorKind::Uncategorized,
            "the storage device could not be prepared to work",
        )),
        vex_sdk::FRESULT::FR_NO_FILE => Err(io::Error::new(
            io::ErrorKind::NotFound,
            "could not find the file in the directory",
        )),
        vex_sdk::FRESULT::FR_NO_PATH => Err(io::Error::new(
            io::ErrorKind::NotFound,
            "a directory in the path name could not be found",
        )),
        vex_sdk::FRESULT::FR_INVALID_NAME => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "the given string is invalid as a path name",
        )),
        vex_sdk::FRESULT::FR_DENIED => Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "the required access for this operation was denied",
        )),
        vex_sdk::FRESULT::FR_EXIST => Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "an object with the same name already exists in the directory",
        )),
        vex_sdk::FRESULT::FR_INVALID_OBJECT => Err(io::Error::new(
            io::ErrorKind::Uncategorized,
            "invalid or null file/directory object",
        )),
        vex_sdk::FRESULT::FR_WRITE_PROTECTED => Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "a write operation was performed on write-protected media",
        )),
        vex_sdk::FRESULT::FR_INVALID_DRIVE => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "an invalid drive number was specified in the path name",
        )),
        vex_sdk::FRESULT::FR_NOT_ENABLED => Err(io::Error::new(
            io::ErrorKind::Uncategorized,
            "work area for the logical drive has not been registered",
        )),
        vex_sdk::FRESULT::FR_NO_FILESYSTEM => Err(io::Error::new(
            io::ErrorKind::Uncategorized,
            "valid FAT volume could not be found on the drive",
        )),
        vex_sdk::FRESULT::FR_MKFS_ABORTED => Err(io::Error::new(
            io::ErrorKind::Uncategorized,
            "failed to create filesystem volume",
        )),
        vex_sdk::FRESULT::FR_TIMEOUT => Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "the function was canceled due to a timeout of thread-safe control",
        )),
        vex_sdk::FRESULT::FR_LOCKED => Err(io::Error::new(
            io::ErrorKind::Uncategorized,
            "the operation to the object was rejected by file sharing control",
        )),
        vex_sdk::FRESULT::FR_NOT_ENOUGH_CORE => Err(io::Error::new(
            io::ErrorKind::Uncategorized,
            "not enough memory for the operation",
        )),
        vex_sdk::FRESULT::FR_TOO_MANY_OPEN_FILES => Err(io::Error::new(
            io::ErrorKind::Uncategorized,
            "maximum number of open files has been reached",
        )),
        vex_sdk::FRESULT::FR_INVALID_PARAMETER => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a given parameter was invalid",
        )),
        _ => unreachable!(), // C-style enum
    }
}

pub fn copy<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> io::Result<u64> {
    let from = read(from)?;
    let mut to = File::create(to)?;
    // Not completely accurate to std, but this is the best we've got
    let len = from.len() as u64;

    to.write_all(&from)?;

    Ok(len)
}

pub fn exists<P: AsRef<Path>>(path: P) -> bool {
    let file_exists = unsafe { vex_sdk::vexFileStatus(path.as_ref().as_fs_str().as_ptr()) };
    // Woop woop we've got a nullptr!
    file_exists != 0
}

pub fn metadata<P: AsRef<Path>>(path: P) -> io::Result<Metadata> {
    Metadata::from_path(path.as_ref())
}

pub fn read<P: AsRef<Path>>(path: P) -> io::Result<Vec<u8>> {
    let mut file = File::open(path)?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;
    Ok(buf)
}

pub fn read_to_string<P: AsRef<Path>>(path: P) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;
    let string = String::from_utf8(buf)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "File was not valid UTF-8"))?;
    Ok(string)
}

pub fn write<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, contents: C) -> io::Result<()> {
    let mut file = File::create(path)?;
    file.write_all(contents.as_ref())
}
