//! Stable Windows file identity for consumers that cache data across reads.

use std::io;
use std::os::windows::io::AsRawHandle;

/// Returns the volume serial and file index for the already-open file handle.
///
/// Reading identity from the handle avoids path replacement races and provides
/// a stable discriminator when creation timestamps have coarse resolution.
pub fn windows_file_identity(file: &std::fs::File) -> io::Result<(u32, u64)> {
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    };

    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    let success = unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) };
    if success == 0 {
        return Err(io::Error::last_os_error());
    }

    let index = (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow);
    Ok((info.dwVolumeSerialNumber, index))
}
