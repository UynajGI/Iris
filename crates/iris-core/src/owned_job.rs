//! Windows ownership for worker descendants (including ExifTool's Perl child).
use std::{
    io,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    process::Child,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};

pub struct OwnedJob(OwnedHandle);

impl OwnedJob {
    /// Attach before supplying input that lets a child create more processes.
    /// Fail closed: on assignment failure, terminate the owned direct child.
    pub fn attach(child: &mut Child) -> io::Result<Self> {
        let result = (|| unsafe {
            let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if handle.is_null() {
                return Err(io::Error::last_os_error());
            }
            let handle = OwnedHandle::from_raw_handle(handle);
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                handle.as_raw_handle(),
                JobObjectExtendedLimitInformation,
                &limits as *const _ as _,
                std::mem::size_of_val(&limits) as u32,
            ) == 0
                || AssignProcessToJobObject(handle.as_raw_handle(), child.as_raw_handle()) == 0
            {
                return Err(io::Error::last_os_error());
            }
            Ok(Self(handle))
        })();
        if result.is_err() {
            let _ = child.kill();
            let _ = child.wait();
        }
        result
    }
    pub fn terminate(&self) {
        unsafe {
            windows_sys::Win32::System::JobObjects::TerminateJobObject(self.0.as_raw_handle(), 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{BufRead, BufReader, Write},
        os::windows::process::CommandExt,
        process::{Command, Stdio},
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, WaitForSingleObject, SYNCHRONIZATION_SYNCHRONIZE,
    };

    #[test]
    fn closing_job_terminates_owned_descendants() {
        let script = "$null = [Console]::ReadLine(); $p = Start-Process powershell.exe -WindowStyle Hidden -ArgumentList '-NoProfile -Command Start-Sleep -Seconds 60' -PassThru; Write-Output $p.Id; Start-Sleep -Seconds 60";
        let mut child = Command::new("powershell.exe")
            .args(["-NoProfile", "-Command", script])
            .creation_flags(0x08000000)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let job = OwnedJob::attach(&mut child).unwrap();
        child.stdin.take().unwrap().write_all(b"go\n").unwrap();
        let mut line = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        let pid = line.trim().parse::<u32>().unwrap();
        let descendant = unsafe { OpenProcess(SYNCHRONIZATION_SYNCHRONIZE, 0, pid) };
        assert!(!descendant.is_null());
        let descendant = unsafe { OwnedHandle::from_raw_handle(descendant) };
        drop(job);
        assert_eq!(
            unsafe { WaitForSingleObject(descendant.as_raw_handle(), 3000) },
            0
        );
        child.wait().unwrap();
    }
}
