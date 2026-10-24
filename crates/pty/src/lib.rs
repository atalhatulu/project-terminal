use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, Once};
use std::thread;

use nix::pty::{openpty, Winsize};
use nix::sys::signal::{signal, SigHandler, Signal};
use nix::unistd::{close, dup2, execvp, fork, setsid, ForkResult, Pid};
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
                            Err(_) => break,
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
                // Çocuk süreç: Yeni oturum başlat
                let _ = setsid();

                if let Some(ref d) = cwd {
                    let _ = std::env::set_current_dir(d);
                }

                unsafe {
                    // Ebeveyn süreç çökerse veya kapanırsa çekirdeğin çocuğu sonlandırmasını sağla
                    libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGHUP as libc::c_ulong, 0, 0, 0);

                    // SIGCHLD ve diğer sinyalleri varsayılana çek
                    let _ = signal(Signal::SIGCHLD, SigHandler::SigDfl);

                    // PTY slave'i kontrol terminali yap (TIOCSCTTY)
                    libc::ioctl(slave_fd.as_raw_fd(), libc::TIOCSCTTY, 0);

                    // stdin, stdout, stderr'i slave_fd'ye yönlendir
                    let s_raw = slave_fd.as_raw_fd();
                    let _ = dup2(s_raw, 0);
                    let _ = dup2(s_raw, 1);
                    let _ = dup2(s_raw, 2);

                    if s_raw > 2 {
                        let _ = close(s_raw);
                    }
                    let _ = close(master_fd.as_raw_fd());
                }

                // Ortam değişkenlerini hazırla
                std::env::set_var("TERM", "xterm-256color");
                std::env::set_var("COLORTERM", "truecolor");
                std::env::set_var("TERMINAL", "project-terminal");
                std::env::set_var("TERM_PROGRAM", "project-terminal");
                std::env::set_var("TERM_PROGRAM_VERSION", "0.1.0");

                // Özel bir komut verildiyse (örn: project-terminal -e htop) doğrudan çalıştır
                if let Some(cmd_args) = command {
                    if let Some(first) = cmd_args.first() {
                        let c_prog = CString::new(first.clone()).unwrap();
                        let c_args: Vec<CString> = cmd_args
                            .iter()
                            .map(|s| CString::new(s.clone()).unwrap())
                            .collect();
                        let _ = execvp(&c_prog, &c_args);
                    }
                    std::process::exit(1);
                }

                // OSC 133 Shell Entegrasyon Betiğini bul ve BASH_ENV/ENV içine ekle
                let mut bash_args = Vec::new();
                let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());

                let integration_script = resolve_shell_integration_script();

                if let Some(script) = integration_script {
                    if shell.ends_with("bash") {
                        if let Some(script_str) = script.to_str() {
                            std::env::set_var("BASH_ENV", script_str);
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

                let shell_c = CString::new(shell.clone()).unwrap();
                let _ = execvp(&shell_c, &bash_args);
                std::process::exit(1);
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

