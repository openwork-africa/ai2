#[cfg(target_os = "macos")]
pub fn start_time(pid: u32) -> Option<String> {
    let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<libc::proc_bsdinfo>() as libc::c_int;
    let read = unsafe {
        libc::proc_pidinfo(
            pid as libc::c_int,
            libc::PROC_PIDTBSDINFO,
            0,
            (&mut info as *mut libc::proc_bsdinfo).cast(),
            size,
        )
    };
    (read == size).then(|| format!("{}.{:06}", info.pbi_start_tvsec, info.pbi_start_tvusec))
}

#[cfg(target_os = "linux")]
pub fn start_time(pid: u32) -> Option<String> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let rest = &stat[stat.rfind(')')? + 1..];
    rest.split_whitespace().nth(19).map(str::to_string)
}

#[cfg(target_os = "macos")]
pub fn boot_id() -> String {
    let name = c"kern.bootsessionuuid";
    let mut buffer = [0u8; 64];
    let mut len = buffer.len();
    let done = unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            buffer.as_mut_ptr().cast(),
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    if done != 0 {
        return String::new();
    }
    String::from_utf8_lossy(&buffer[..len])
        .trim_end_matches('\0')
        .trim()
        .to_string()
}

#[cfg(target_os = "linux")]
pub fn boot_id() -> String {
    std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
        .map(|id| id.trim().to_string())
        .unwrap_or_default()
}

pub fn alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    let done = unsafe { libc::kill(pid as libc::pid_t, 0) };
    done == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

pub fn same(pid: u32, started_at: &str, boot: &str) -> bool {
    alive(pid) && boot == boot_id() && start_time(pid).as_deref() == Some(started_at)
}

pub fn signal_group(pid: u32, signal: libc::c_int) {
    if pid <= 1 || pid == std::process::id() {
        return;
    }
    let pid = pid as libc::pid_t;
    unsafe {
        if libc::getpgid(pid) == pid {
            libc::killpg(pid, signal);
        } else {
            libc::kill(pid, signal);
        }
    }
}
