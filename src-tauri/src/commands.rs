use serde::Serialize;
use std::collections::HashMap;
use std::process::Command;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Serialize, Clone)]
pub struct PortInfo {
    pub port: u16,
    pub pid: u32,
    pub process_name: String,
}

fn cmd(program: &str) -> Command {
    let mut c = Command::new(program);
    #[cfg(target_os = "windows")]
    c.creation_flags(CREATE_NO_WINDOW);
    c
}

#[tauri::command]
pub fn list_ports() -> Vec<PortInfo> {
    let netstat_output = match cmd("netstat")
        .args(["-ano", "-p", "tcp"])
        .output()
    {
        Ok(output) => String::from_utf8_lossy(&output.stdout).to_string(),
        Err(_) => return vec![],
    };

    let mut pid_port_pairs: Vec<(u16, u32)> = Vec::new();

    for line in netstat_output.lines() {
        let line = line.trim();
        if !line.contains("LISTENING") {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 5 {
            continue;
        }

        let addr = parts[1];
        let port: u16 = match addr.rsplit(':').next().and_then(|p| p.parse().ok()) {
            Some(p) => p,
            None => continue,
        };

        let pid: u32 = match parts[4].parse() {
            Ok(p) if p > 0 => p,
            _ => continue,
        };

        pid_port_pairs.push((port, pid));
    }

    let process_names = get_process_names();

    let mut ports: Vec<PortInfo> = pid_port_pairs
        .into_iter()
        .map(|(port, pid)| PortInfo {
            port,
            pid,
            process_name: process_names
                .get(&pid)
                .cloned()
                .unwrap_or_else(|| "Unknown".to_string()),
        })
        .collect();

    ports.sort_by_key(|p| p.port);
    ports.dedup_by(|a, b| a.port == b.port && a.pid == b.pid);

    ports
}

fn get_process_names() -> HashMap<u32, String> {
    let mut map = HashMap::new();

    let output = match cmd("tasklist")
        .args(["/FO", "CSV", "/NH"])
        .output()
    {
        Ok(output) => String::from_utf8_lossy(&output.stdout).to_string(),
        Err(_) => return map,
    };

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let fields: Vec<&str> = line.split(',').collect();
        if fields.len() < 2 {
            continue;
        }

        let name = fields[0].trim_matches('"').to_string();
        if let Ok(pid) = fields[1].trim_matches('"').parse::<u32>() {
            map.insert(pid, name);
        }
    }

    map
}

#[tauri::command]
pub fn kill_port(pid: u32) -> Result<String, String> {
    if pid == 0 || pid == 4 {
        return Err("Cannot kill system process".to_string());
    }

    let output = cmd("taskkill")
        .args(["/PID", &pid.to_string(), "/F"])
        .output()
        .map_err(|e| format!("Failed to execute taskkill: {}", e))?;

    if output.status.success() {
        Ok(format!("Process {} killed", pid))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("Failed to kill process {}: {}", pid, stderr.trim()))
    }
}
