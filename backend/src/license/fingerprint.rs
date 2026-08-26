//! Linux hardware fingerprint collection (LIC-02).
//!
//! Reads identifying values from Linux pseudo-filesystems exposed by the host,
//! concatenates them, and returns the SHA-256 hex digest. Production Compose
//! mounts host sources below `/run/cronometrix-host` read-only so container
//! network identities never participate in the fingerprint.
//!
//! Per D-05 the disk serial is best-effort: VPS instances often expose an
//! empty serial — that is acceptable, the resulting fingerprint stays stable
//! per-VPS even if uniqueness across VPSes degrades. Production targets are
//! Linux servers (Docker Compose on Linux per CLAUDE.md); macOS dev hosts
//! return Err because /proc/cpuinfo does not exist — handled by caller.

use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

const HOST_ROOT: &str = "/run/cronometrix-host";

/// Collect a deterministic hardware fingerprint from Linux pseudo-filesystems.
/// V2 hashes CPU model, board/DMI/machine identity, physical MAC, and disk
/// serial. Empty disk serial is acceptable on VPS (D-05).
pub fn collect_fingerprint() -> Result<String, anyhow::Error> {
    let host_root = Path::new(HOST_ROOT);
    if host_root.join("sys").is_dir() {
        return collect_fingerprint_from(
            &host_root.join("cpuinfo"),
            &host_root.join("sys"),
            &host_root.join("machine-id"),
        );
    }
    collect_fingerprint_from(
        Path::new("/proc/cpuinfo"),
        Path::new("/sys"),
        Path::new("/etc/machine-id"),
    )
}

fn collect_fingerprint_from(
    cpuinfo_path: &Path,
    sys_root: &Path,
    machine_id_path: &Path,
) -> Result<String, anyhow::Error> {
    let cpu = read_cpu_model_from(cpuinfo_path)?;
    let stable_id = read_stable_host_id(sys_root, machine_id_path);
    let mac = read_primary_mac_from(&sys_root.join("class/net")).unwrap_or_default();
    let disk = read_primary_disk_serial_from(&sys_root.join("block")).unwrap_or_default();
    if stable_id.is_empty() && mac.is_empty() && disk.is_empty() {
        anyhow::bail!("no stable host identity is available");
    }

    let mut hasher = Sha256::new();
    hasher.update(b"cronometrix-hardware-v2\0");
    hasher.update(cpu.as_bytes());
    hasher.update(stable_id.as_bytes());
    hasher.update(mac.as_bytes());
    hasher.update(disk.as_bytes());
    Ok(format!("{:x}", hasher.finalize()))
}

fn read_cpu_model_from(path: &Path) -> Result<String, anyhow::Error> {
    let cpuinfo =
        fs::read_to_string(path).map_err(|e| anyhow::anyhow!("read {}: {}", path.display(), e))?;
    Ok(cpuinfo
        .lines()
        .find(|line| {
            let key = line.split(':').next().unwrap_or_default().trim();
            matches!(key, "model name" | "Model" | "Hardware")
        })
        .and_then(|l| l.split(':').nth(1))
        .map(|s| s.trim().to_string())
        .unwrap_or_default())
}

fn read_primary_mac_from(root: &Path) -> Result<String, anyhow::Error> {
    let mut entries = fs::read_dir(root)
        .map_err(|e| anyhow::anyhow!("read {}: {}", root.display(), e))?
        .flatten()
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let name = entry.file_name().to_string_lossy().to_string();
        if name == "lo" || !entry.path().join("device").exists() {
            continue;
        }
        let mac_path = entry.path().join("address");
        if let Ok(mac) = fs::read_to_string(&mac_path) {
            let mac = mac.trim().to_string();
            if mac != "00:00:00:00:00:00" && !mac.is_empty() {
                return Ok(mac);
            }
        }
    }
    Ok(String::new())
}

fn read_primary_disk_serial_from(root: &Path) -> Result<String, anyhow::Error> {
    let mut entries = fs::read_dir(root)
        .map_err(|e| anyhow::anyhow!("read {}: {}", root.display(), e))?
        .flatten()
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("loop") || name.starts_with("ram") || name.starts_with("dm-") {
            continue;
        }
        let serial_path = entry.path().join("device/serial");
        if let Ok(serial) = fs::read_to_string(serial_path) {
            let s = serial.trim().to_string();
            if !s.is_empty() {
                return Ok(s);
            }
        }
    }
    Ok(String::new())
}

fn read_stable_host_id(sys_root: &Path, machine_id_path: &Path) -> String {
    [
        sys_root.join("firmware/devicetree/base/serial-number"),
        sys_root.join("class/dmi/id/product_uuid"),
        machine_id_path.to_path_buf(),
    ]
    .into_iter()
    .find_map(|path| {
        fs::read(path).ok().and_then(|bytes| {
            let value = String::from_utf8_lossy(&bytes)
                .trim_matches(|character: char| character == '\0' || character.is_whitespace())
                .to_string();
            (!value.is_empty()).then_some(value)
        })
    })
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{collect_fingerprint_from, read_primary_mac_from};

    fn interface_root(name: &str, address: &str) -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        let interface = root.path().join(name);
        std::fs::create_dir(&interface).unwrap();
        std::fs::write(interface.join("address"), address).unwrap();
        root
    }

    #[test]
    fn primary_mac_skips_loopback_zero_and_empty_addresses() {
        let loopback = interface_root("lo", "11:22:33:44:55:66\n");
        assert_eq!(read_primary_mac_from(loopback.path()).unwrap(), "");

        let zero = interface_root("eth0", "00:00:00:00:00:00\n");
        std::fs::create_dir(zero.path().join("eth0/device")).unwrap();
        assert_eq!(read_primary_mac_from(zero.path()).unwrap(), "");

        let empty = interface_root("eth0", "\n");
        std::fs::create_dir(empty.path().join("eth0/device")).unwrap();
        assert_eq!(read_primary_mac_from(empty.path()).unwrap(), "");

        let valid = interface_root("eth0", "AA:BB:CC:DD:EE:FF\n");
        std::fs::create_dir(valid.path().join("eth0/device")).unwrap();
        assert_eq!(
            read_primary_mac_from(valid.path()).unwrap(),
            "AA:BB:CC:DD:EE:FF"
        );
    }

    #[test]
    fn host_board_serial_keeps_fingerprint_stable_when_virtual_mac_changes() {
        let root = tempfile::tempdir().unwrap();
        let cpuinfo = root.path().join("cpuinfo");
        let sys = root.path().join("sys");
        let machine_id = root.path().join("machine-id");
        std::fs::write(&cpuinfo, "Model\t: NanoPi R5C\n").unwrap();
        std::fs::write(&machine_id, "host-machine-id\n").unwrap();
        let serial = sys.join("firmware/devicetree/base/serial-number");
        std::fs::create_dir_all(serial.parent().unwrap()).unwrap();
        std::fs::write(&serial, b"stable-board-serial\0").unwrap();
        let virtual_net = sys.join("class/net/eth0");
        std::fs::create_dir_all(&virtual_net).unwrap();
        std::fs::write(virtual_net.join("address"), "02:42:ac:11:00:02\n").unwrap();

        let before = collect_fingerprint_from(&cpuinfo, &sys, &machine_id).unwrap();
        std::fs::write(virtual_net.join("address"), "02:42:ac:11:00:99\n").unwrap();
        let after = collect_fingerprint_from(&cpuinfo, &sys, &machine_id).unwrap();

        assert_eq!(before, after);
    }

    #[test]
    fn fingerprint_fails_closed_without_stable_host_identity() {
        let root = tempfile::tempdir().unwrap();
        let cpuinfo = root.path().join("cpuinfo");
        let sys = root.path().join("sys");
        let machine_id = root.path().join("missing-machine-id");
        std::fs::write(&cpuinfo, "Model\t: test\n").unwrap();
        std::fs::create_dir_all(sys.join("class/net")).unwrap();
        std::fs::create_dir_all(sys.join("block")).unwrap();

        let error = collect_fingerprint_from(&cpuinfo, &sys, &machine_id).unwrap_err();

        assert!(error.to_string().contains("stable host identity"));
    }
}
