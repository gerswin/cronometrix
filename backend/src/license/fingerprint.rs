//! Linux hardware fingerprint collection (LIC-02).
//!
//! Reads identifying values from Linux pseudo-filesystems (/proc, /sys),
//! concatenates them, and returns the SHA-256 hex digest. The fingerprint is
//! deterministic across consecutive calls on the same machine and changes when
//! the underlying hardware changes (anti-cloning baseline for LIC-05).
//!
//! Per D-05 the disk serial is best-effort: VPS instances often expose an
//! empty serial — that is acceptable, the resulting fingerprint stays stable
//! per-VPS even if uniqueness across VPSes degrades. Production targets are
//! Linux servers (Docker Compose on Linux per CLAUDE.md); macOS dev hosts
//! return Err because /proc/cpuinfo does not exist — handled by caller.

use sha2::{Digest, Sha256};
use std::fs;

/// Collect a deterministic hardware fingerprint from Linux pseudo-filesystems.
/// SHA256(cpu_model + mac + disk_serial), no salt (D-05).
/// Empty disk serial is acceptable on VPS (D-05 allows degraded uniqueness).
pub fn collect_fingerprint() -> Result<String, anyhow::Error> {
    let cpu = read_cpu_model()?;
    let mac = read_primary_mac()?;
    let disk = read_primary_disk_serial().unwrap_or_default();

    let mut hasher = Sha256::new();
    hasher.update(cpu.as_bytes());
    hasher.update(mac.as_bytes());
    hasher.update(disk.as_bytes());
    Ok(format!("{:x}", hasher.finalize()))
}

fn read_cpu_model() -> Result<String, anyhow::Error> {
    let cpuinfo = fs::read_to_string("/proc/cpuinfo")
        .map_err(|e| anyhow::anyhow!("read /proc/cpuinfo: {}", e))?;
    Ok(cpuinfo
        .lines()
        .find(|l| l.starts_with("model name"))
        .and_then(|l| l.split(':').nth(1))
        .map(|s| s.trim().to_string())
        .unwrap_or_default())
}

fn read_primary_mac() -> Result<String, anyhow::Error> {
    let net_dir = fs::read_dir("/sys/class/net")
        .map_err(|e| anyhow::anyhow!("read /sys/class/net: {}", e))?;
    for entry in net_dir.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name == "lo" {
            continue;
        }
        let mac_path = format!("/sys/class/net/{}/address", name);
        if let Ok(mac) = fs::read_to_string(&mac_path) {
            let mac = mac.trim().to_string();
            if mac != "00:00:00:00:00:00" && !mac.is_empty() {
                return Ok(mac);
            }
        }
    }
    Ok(String::new())
}

fn read_primary_disk_serial() -> Result<String, anyhow::Error> {
    let block_dir = fs::read_dir("/sys/block")
        .map_err(|e| anyhow::anyhow!("read /sys/block: {}", e))?;
    for entry in block_dir.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("loop") || name.starts_with("ram") || name.starts_with("dm-") {
            continue;
        }
        let serial_path = format!("/sys/block/{}/device/serial", name);
        if let Ok(serial) = fs::read_to_string(&serial_path) {
            let s = serial.trim().to_string();
            if !s.is_empty() {
                return Ok(s);
            }
        }
    }
    Ok(String::new())
}
