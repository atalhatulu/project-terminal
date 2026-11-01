use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, Once};
use std::thread;

use nix::pty::{openpty, Winsize};
use nix::sys::signal::Signal;
use nix::unistd::{dup2, fork, setsid, ForkResult, Pid};
use std::ffi::CString;

const BASH_INTEGRATION_SCRIPT: &str = include_str!("../../../shell/project-terminal.bash");

static INIT_REAPER: Once = Once::new();

/// Zombi süreçleri önlemek için SIGCHLD sinyaline SA_NOCLDWAIT bayrağı tanımlar.
/// Böylece çocuk süreçler sonlandığında çekirdek tarafından otomatik olarak toplanır.
fn init_child_reaper() {
    INIT_REAPER.call_once(|| unsafe {
        let mut sa: libc::sigaction = std::mem::zeroed();
        sa.sa_sigaction = libc::SIG_DFL;
        sa.sa_flags = libc::SA_NOCLDWAIT | libc::SA_RESTART;
        libc::sigaction(libc::SIGCHLD, &sa, std::ptr::null_mut());
    });
}

/// Kabuk entegrasyon betiğini güvenli ve taşınabilir bir yoldan çözer.
fn resolve_shell_integration_script() -> Option<std::path::PathBuf> {
    let current_dir_script = std::env::current_dir()
        .ok()
        .map(|d| d.join("shell/project-terminal.bash"));
    let exe_script = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("shell/project-terminal.bash")));

    if let Some(script) = [current_dir_script, exe_script]
        .into_iter()
        .flatten()
        .find(|p| p.exists())
    {
        return Some(script);
    }

    // Disk üzerinde bulunamadıysa derleme anında ikiliye gömülmüş betiği geçici dizine yaz
    let tmp_script = std::env::temp_dir().join("project-terminal-bash.bash");
    let needs_write = match std::fs::read_to_string(&tmp_script) {
        Ok(existing) => existing != BASH_INTEGRATION_SCRIPT,
        Err(_) => true,
    };

    if needs_write {
        let _ = std::fs::write(&tmp_script, BASH_INTEGRATION_SCRIPT);
    }

    if tmp_script.exists() {
        Some(tmp_script)
    } else {
        None
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PtyError {
    #[error("PTY açma başarısız: {0}")]
    OpenPty(#[from] nix::Error),
    #[error("Girdi/Çıktı hatası: {0}")]
    Io(#[from] std::io::Error),
    #[error("Fork başarısız")]
    ForkFailed,
}

pub struct Pty {
    master_fd: OwnedFd,
    child_pid: Pid,
    writer: Arc<Mutex<File>>,
}

impl Pty {
    /// Linux native PTY açar ve varsayılan kabuğu veya belirtilen komutu başlatır.
    pub fn spawn(
        cols: u16,
        rows: u16,
        command: Option<Vec<String>>,
        cwd: Option<std::path::PathBuf>,
    ) -> Result<(Self, Receiver<Vec<u8>>), PtyError> {
        // Çekirdeğin sonlanan çocuk süreçleri otomatik temizlemesi için sinyal bayrağını kur
        init_child_reaper();

        let winsize = Winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };

        let pty_pair = openpty(Some(&winsize), None)?;
        let master_fd = pty_pair.master;
        let slave_fd = pty_pair.slave;

        // Fork öncesi hazırlıklar (K1):
        // POSIX standartlarına göre ortam_degiskenleri ve getenv async-signal-safe DEĞİLDİR.
        // Bu sebeple tüm ortam değişkenleri (envp), CString dönüşümleri ve mutlak program yolu
        // fork() çağrısından önce heap üzerinde güvenle hazırlanır.
        let cwd_c = cwd.and_then(|p| CString::new(p.into_os_string().into_encoded_bytes()).ok());

        let resolve_bin = |prog: &str| -> String {
            if prog.contains('/') {
                return prog.to_string();
            }
            if let Ok(path_var) = std::env::var("PATH") {
                for dir in path_var.split(':') {
                    let p = std::path::Path::new(dir).join(prog);
                    if p.is_file() {
                        return p.to_string_lossy().to_string();
                    }
                }
            }
            prog.to_string()
        };

        let (exec_prog_c, exec_argv_c) = if let Some(cmd_args) = command {
            let mut c_args = Vec::new();
            for arg in cmd_args {
                if let Ok(c) = CString::new(arg) {
                    c_args.push(c);
                }
            }
            if let Some(first) = c_args.first().cloned() {
                let resolved = resolve_bin(&first.to_string_lossy());
                (CString::new(resolved).unwrap_or(first), c_args)
            } else {
                let sh = CString::new("/bin/sh").unwrap();
                (sh.clone(), vec![sh])
            }
        } else {
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
            let integration_script = resolve_shell_integration_script();
            let mut bash_args = Vec::new();

            if let Some(script) = integration_script {
                if shell.ends_with("bash") {
                    if let Some(script_str) = script.to_str() {
                        bash_args.push(CString::new(shell.clone()).unwrap());
                        bash_args.push(CString::new("--init-file").unwrap());
                        bash_args.push(CString::new(script_str).unwrap());
                    } else {
                        bash_args.push(CString::new(shell.clone()).unwrap());
                    }
                } else {
                    bash_args.push(CString::new(shell.clone()).unwrap());
                }
            } else {
                bash_args.push(CString::new(shell.clone()).unwrap());
            }

            let resolved = resolve_bin(&shell);
            let shell_c = CString::new(resolved).unwrap_or_else(|_| CString::new(shell).unwrap());
            (shell_c, bash_args)
        };

        // Mevcut ortam değişkenlerini topla ve Project Terminal değişkenleriyle harmanla
        let mut env_map = std::collections::HashMap::new();
        for (k, v) in std::env::vars() {
            env_map.insert(k, v);
        }
        env_map.insert("TERM".to_string(), "xterm-256color".to_string());
        env_map.insert("COLORTERM".to_string(), "truecolor".to_string());
        env_map.insert("TERMINAL".to_string(), "project-terminal".to_string());
        env_map.insert("TERM_PROGRAM".to_string(), "project-terminal".to_string());
        env_map.insert("TERM_PROGRAM_VERSION".to_string(), "0.1.0".to_string());

        let mut envp_c = Vec::new();
        for (k, v) in env_map {
            if let Ok(entry) = CString::new(format!("{}={}", k, v)) {
                envp_c.push(entry);
            }
        }

        // argv ve envp için ham C işaretçi dizileri (NULL ile sonlanan)
        let mut argv_ptrs: Vec<*const libc::c_char> = exec_argv_c.iter().map(|s| s.as_ptr()).collect();
        argv_ptrs.push(std::ptr::null());

        let mut envp_ptrs: Vec<*const libc::c_char> = envp_c.iter().map(|s| s.as_ptr()).collect();
        envp_ptrs.push(std::ptr::null());

        match unsafe { fork() } {
            Ok(ForkResult::Parent { child }) => {
                // Ebeveyn süreç: Slave FD'yi kapat
                drop(slave_fd);

                let master_raw = master_fd.as_raw_fd();
                let master_file_write = unsafe { File::from_raw_fd(nix::unistd::dup(master_raw)?) };
                let mut master_file_read =
                    unsafe { File::from_raw_fd(nix::unistd::dup(master_raw)?) };

                let writer = Arc::new(Mutex::new(master_file_write));
                let (tx, rx): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = channel();

                // Non-blocking PTY okuyucu thread'i
                thread::spawn(move || {
                    let mut buf = [0u8; 8192];
                    loop {
                        match master_file_read.read(&mut buf) {
                            Ok(0) => break, // EOF
                            Ok(n) => {
                                if tx.send(buf[..n].to_vec()).is_err() {
                                    break;
                                }
                            }
                            Err(e) => {
                                // EIO (Input/output error) Linux PTY slave kapandığında olağan bir durumdur (O8).
                                // Diğer beklenmedik I/O hataları için tracing ile bilgi kaydedilir.
                                if e.raw_os_error() != Some(libc::EIO) {
                                    tracing::debug!("PTY master read ended: {:?}", e);
                                }
                                break;
                            }
                        }
                    }
                });

                Ok((
                    Self {
                        master_fd,
                        child_pid: child,
                        writer,
                    },
                    rx,
                ))
            }
            Ok(ForkResult::Child) => {
                // K3: OwnedFd çifte kapanma (double-close) riskini önlemek için ham FD al
                let s_raw = slave_fd.into_raw_fd();
                let m_raw = master_fd.into_raw_fd();

                // Çocuk süreç: Yeni oturum başlat
                let _ = setsid();

                if let Some(ref d) = cwd_c {
                    unsafe { libc::chdir(d.as_ptr()) };
                }

                unsafe {
                    // Ebeveyn süreç çökerse veya kapanırsa çekirdeğin çocuğu sonlandırmasını sağla
                    libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGHUP as libc::c_ulong, 0, 0, 0);

                    // SIGCHLD ve diğer sinyalleri varsayılana çek (sigaction ile, signal değil)
                    let mut sa: libc::sigaction = std::mem::zeroed();
                    sa.sa_sigaction = libc::SIG_DFL;
                    libc::sigaction(libc::SIGCHLD, &sa, std::ptr::null_mut());

                    // PTY slave kontrol terminali yap (TIOCSCTTY)
                    libc::ioctl(s_raw, libc::TIOCSCTTY, 0);

                    // stdin, stdout, stderr slave_fd ye yönlendir
                    let _ = dup2(s_raw, 0);
                    let _ = dup2(s_raw, 1);
                    let _ = dup2(s_raw, 2);

                    if s_raw > 2 {
                        libc::close(s_raw);
                    }
                    libc::close(m_raw);
                }

                // K1: execve doğrudan POSIX async-signal-safe bir sistem çağrısıdır.
                // ortam_degiskenleri/getenv çağırmadan önceden hazırlanmış envp dizisi doğrudan aktarılır.
                unsafe {
                    libc::execve(
                        exec_prog_c.as_ptr(),
                        argv_ptrs.as_ptr(),
                        envp_ptrs.as_ptr(),
                    );
                    // K2: atexit handler tetikleyen std::process::exit(1) yerine libc::_exit(127) convention
                    libc::_exit(127);
                };
            }
            Err(_) => Err(PtyError::ForkFailed),
        }
    }

    /// PTY boyutunu anında yeniden boyutlandırır (SIGWINCH tetikler).
    pub fn resize(&self, cols: u16, rows: u16) -> Result<(), PtyError> {
        let ws = Winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        unsafe {
            let res = libc::ioctl(self.master_fd.as_raw_fd(), libc::TIOCSWINSZ, &ws);
            if res < 0 {
                return Err(PtyError::Io(std::io::Error::last_os_error()));
            }
        }
        Ok(())
    }

    /// Kabuğa bayt gönderir (klavye girdisi vb.).
    pub fn write_bytes(&self, data: &[u8]) -> Result<(), PtyError> {
        let mut w = self.writer.lock().unwrap();
        w.write_all(data)?;
        w.flush()?;
        Ok(())
    }

    pub fn pid(&self) -> Pid {
        self.child_pid
    }

    pub fn raw_fd(&self) -> RawFd {
        self.master_fd.as_raw_fd()
    }

    /// Linux /proc/<pid>/cwd symlink'i üzerinden kabuğun mevcut çalışma dizinini döndürür.
    pub fn current_working_directory(&self) -> Option<std::path::PathBuf> {
        let proc_path = format!("/proc/{}/cwd", self.child_pid);
        std::fs::read_link(proc_path).ok()
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        let _ = nix::sys::signal::kill(self.child_pid, Signal::SIGHUP);
        // Çocuk süreç sonlandıysa zombi kalmasını önlemek için durumunu topla (reap)
        let _ = nix::sys::wait::waitpid(self.child_pid, Some(nix::sys::wait::WaitPidFlag::WNOHANG));
    }
}

/// Çapraz platform PTY arka uç arayüzü (Unix/Linux ve Windows ConPTY için ortak sözleşme)
pub trait PtyBackend: Send + Sync {
    fn write_bytes(&self, data: &[u8]) -> Result<(), PtyError>;
    fn resize(&self, cols: u16, rows: u16) -> Result<(), PtyError>;
    fn kill(&self);
}

impl PtyBackend for Pty {
    fn write_bytes(&self, data: &[u8]) -> Result<(), PtyError> {
        self.write_bytes(data)
    }

    fn resize(&self, cols: u16, rows: u16) -> Result<(), PtyError> {
        self.resize(cols, rows)
    }

    fn kill(&self) {
        let _ = nix::sys::signal::kill(self.child_pid, Signal::SIGHUP);
        let _ = nix::sys::wait::waitpid(self.child_pid, Some(nix::sys::wait::WaitPidFlag::WNOHANG));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pty_spawn_and_communication() {
        let (pty, rx) = Pty::spawn(
            80,
            24,
            Some(vec!["/bin/sh".to_string(), "-c".to_string(), "echo test_output_123".to_string()]),
            None,
        )
        .expect("PTY oluşturulabilmeli");

        assert!(pty.pid().as_raw() > 0);

        // Çıktıyı bekle
        let mut collected = Vec::new();
        let start = std::time::Instant::now();
        while start.elapsed() < std::time::Duration::from_secs(3) {
            if let Ok(data) = rx.recv_timeout(std::time::Duration::from_millis(100)) {
                collected.extend(data);
                let text = String::from_utf8_lossy(&collected);
                if text.contains("test_output_123") {
                    break;
                }
            }
        }

        let text = String::from_utf8_lossy(&collected);
        assert!(text.contains("test_output_123"), "PTY çıktısı alınamadı: {}", text);
    }

    #[test]
    fn test_pty_resize() {
        let (pty, _rx) = Pty::spawn(80, 24, None, None).expect("PTY açılabilmeli");
        assert!(pty.resize(120, 40).is_ok());
    }

    #[test]
    fn test_embedded_script_resolution() {
        let script = resolve_shell_integration_script();
        assert!(script.is_some());
        let path = script.unwrap();
        assert!(path.exists());
    }
}
