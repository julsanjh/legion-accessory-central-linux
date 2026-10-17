use crate::models::{ParsedProfile, SUPPORTED_MODELS, VID_LENOVO};
use crate::protocol::parse_profile;
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use std::process::Command;
use std::thread;
use std::time::Duration;

pub fn detect_connected_devices() -> Vec<(usize, String)> {
    let mut found = Vec::new();
    let Ok(entries) = fs::read_dir("/sys/class/hidraw") else {
        return found;
    };
    let mut paths: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();

    for sys_path in paths {
        let uevent_path = sys_path.join("device/uevent");
        let Ok(uevent) = fs::read_to_string(&uevent_path) else {
            continue;
        };
        let Some(dev_name) = sys_path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        let dev_node = format!("/dev/{dev_name}");
        for (idx, model) in SUPPORTED_MODELS.iter().enumerate() {
            for &pid in model.pids {
                let target_id = format!("HID_ID=0003:{VID_LENOVO:08X}:{pid:08X}");
                let target_if = format!("/input{}", model.interface);
                if uevent.contains(&target_id) && uevent.contains(&target_if) {
                    found.push((idx, dev_node.clone()));
                }
            }
        }
    }
    found
}

pub fn unlock_with_pkexec(dev_path: &str) -> bool {
    let uid = unsafe { libc::getuid() };
    let rules = include_str!("../99-lenovo-legion-hid.rules");
    let script = format!(
        "chmod 0666 {dev_path} && setfacl -m u:{uid}:rw {dev_path} && cat << 'EOF' > /etc/udev/rules.d/99-lenovo-legion-hid.rules\n{rules}EOF\nudevadm control --reload-rules && udevadm trigger"
    );
    Command::new("pkexec")
        .arg("sh")
        .arg("-c")
        .arg(script)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub struct HidConnection {
    file: fs::File,
}

impl HidConnection {
    pub fn open(path: &str) -> Result<Self, String> {
        let open_res = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(path);

        let file = match open_res {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
                if unlock_with_pkexec(path) {
                    OpenOptions::new()
                        .read(true)
                        .write(true)
                        .custom_flags(libc::O_NONBLOCK)
                        .open(path)
                        .map_err(|e2| format!("Failed opening {path} after Polkit unlock: {e2}"))?
                } else {
                    return Err(format!(
                        "Permission denied opening {path}. Klik 'Unlock USB (Polkit)' atau jalankan dengan sudo."
                    ));
                }
            }
            Err(e) => return Err(format!("Cannot open {path}: {e}")),
        };
        let mut conn = Self { file };
        conn.clear_queue();
        Ok(conn)
    }

    pub fn clear_queue(&mut self) {
        let mut buf = [0u8; 64];
        while self.read_packet(5).is_some() {
            let _ = self.file.read(&mut buf);
        }
    }

    pub fn write_raw(&mut self, payload64: &[u8], delay_ms: u64) -> Result<(), String> {
        let mut report = [0u8; 65];
        report[0] = 0x00;
        let n = payload64.len().min(64);
        report[1..1 + n].copy_from_slice(&payload64[..n]);
        thread::sleep(Duration::from_millis(delay_ms));
        self.file
            .write_all(&report)
            .map_err(|e| format!("HID write failed: {e}"))?;
        thread::sleep(Duration::from_millis(delay_ms));
        Ok(())
    }

    pub fn read_packet(&mut self, timeout_ms: i32) -> Option<[u8; 64]> {
        let fd = self.file.as_raw_fd();
        let mut pfd = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        let ret = unsafe { libc::poll(&mut pfd, 1, timeout_ms) };
        if ret <= 0 || (pfd.revents & libc::POLLIN) == 0 {
            return None;
        }
        let mut buf = [0u8; 64];
        match self.file.read(&mut buf) {
            Ok(n) if n > 0 => Some(buf),
            _ => None,
        }
    }

    pub fn sync(&mut self) -> Result<ParsedProfile, String> {
        self.clear_queue();
        self.write_raw(&[0x03, 0xFF], 60)?;
        let mut acc = Vec::with_capacity(261);
        loop {
            let Some(pkt) = self.read_packet(600) else {
                break;
            };
            let pkt_idx = pkt[2];
            let pkt_total = pkt[3];
            let pkt_len = (pkt[4] as usize).min(59);
            acc.extend_from_slice(&pkt[5..5 + pkt_len]);
            if pkt_idx >= pkt_total && pkt_total > 0 {
                break;
            }
        }
        if acc.len() < 132 {
            return Err(format!("Incomplete sync packet ({} bytes)", acc.len()));
        }
        if acc[1] != 0 {
            let _ = self.write_raw(&[0x05, 0x00, 0x00], 50);
        }
        parse_profile(&acc[3..132]).ok_or_else(|| "Failed to parse 129-byte profile".to_string())
    }
}
