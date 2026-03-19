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
    pub working_dir: String,
    pub project_name: String,
    pub command_line: String,
}

fn cmd(program: &str) -> Command {
    let mut c = Command::new(program);
    #[cfg(target_os = "windows")]
    c.creation_flags(CREATE_NO_WINDOW);
    c
}

/// Extract a friendly project name from a path.
/// If path contains `node_modules`, take the folder before it.
/// Otherwise take the last meaningful directory segment.
fn extract_project_name(path: &str) -> String {
    if path.is_empty() {
        return "-".to_string();
    }

    let normalized = path.replace('\\', "/");
    let parts: Vec<&str> = normalized.split('/').filter(|s| !s.is_empty()).collect();

    // Skip known non-project segments
    let skip = ["dist", "src", "build", "out", ".next", ".nuxt", ".angular",
                 "node_modules", ".bin", "bin", "lib", "usr", "local", "AppData",
                 "Roaming", "nvm", "Program Files", "Program Files (x86)"];

    // If path contains node_modules, take the segment right before it
    if let Some(idx) = parts.iter().position(|&s| s == "node_modules") {
        if idx > 0 {
            return parts[idx - 1].to_string();
        }
    }

    // If path contains .bin, go up past it and node_modules
    if let Some(idx) = parts.iter().position(|&s| s == ".bin") {
        if idx >= 2 {
            return parts[idx - 2].to_string();
        }
    }

    // Walk from the end, skip known non-project names, find the project root
    for &part in parts.iter().rev() {
        // Skip file extensions
        if part.contains('.') && (part.ends_with(".js") || part.ends_with(".ts") || part.ends_with(".exe") || part.ends_with(".mjs")) {
            continue;
        }
        // Skip drive letters
        if part.len() <= 2 && part.ends_with(':') {
            continue;
        }
        // Skip known system/build dirs
        if skip.iter().any(|&s| part.eq_ignore_ascii_case(s)) {
            continue;
        }
        // Skip version-like segments (v22.0.0)
        if part.starts_with('v') && part[1..].chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            continue;
        }
        // Skip user home dirs
        if part == "Users" || part == "home" || part == "root" {
            continue;
        }
        // This looks like a project name
        if part.len() > 1 {
            return part.to_string();
        }
    }

    "-".to_string()
}

#[tauri::command]
pub fn list_ports() -> Vec<PortInfo> {
    let mut ports = list_ports_platform();
    let process_names = get_process_names(&ports);

    for port in &mut ports {
        if let Some(name) = process_names.get(&port.pid) {
            port.process_name = name.clone();
        }
    }

    // Resolve working directories and command lines
    let mut cwd_cache: HashMap<u32, (String, String, String)> = HashMap::new();
    for port in &mut ports {
        if let Some((wdir, pname, cmdline)) = cwd_cache.get(&port.pid) {
            port.working_dir = wdir.clone();
            port.project_name = pname.clone();
            port.command_line = cmdline.clone();
        } else {
            let wdir = get_process_cwd(port.pid).unwrap_or_default();
            let pname = extract_project_name(&wdir);
            let cmdline = get_command_line(port.pid).unwrap_or_default();
            port.working_dir = wdir.clone();
            port.project_name = pname.clone();
            port.command_line = cmdline.clone();
            cwd_cache.insert(port.pid, (wdir, pname, cmdline));
        }
    }

    ports.sort_by_key(|p| p.port);
    ports.dedup_by(|a, b| a.port == b.port && a.pid == b.pid);
    ports
}

#[tauri::command]
pub fn focus_parent_terminal(pid: u32) -> Result<String, String> {
    focus_parent_platform(pid)
}

#[cfg(target_os = "windows")]
fn focus_parent_platform(pid: u32) -> Result<String, String> {
    // Walk up the parent chain: node.exe → cmd.exe → conhost.exe → WindowsTerminal.exe
    // We need to find any ancestor with a visible window (MainWindowHandle != 0)
    // PowerShell script: walks up parents up to 6 levels, tries to focus each
    let ps_script = format!(
        r#"
Add-Type -Name Win -Namespace Native -Member '
[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
[DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
'
$currentPid = {}
for ($i = 0; $i -lt 6; $i++) {{
    $proc = Get-CimInstance Win32_Process -Filter "ProcessId=$currentPid" -ErrorAction SilentlyContinue
    if (-not $proc) {{ break }}
    $parentPid = $proc.ParentProcessId
    if (-not $parentPid -or $parentPid -eq 0) {{ break }}
    $parent = Get-Process -Id $parentPid -ErrorAction SilentlyContinue
    if ($parent -and $parent.MainWindowHandle -ne 0) {{
        [Native.Win]::ShowWindow($parent.MainWindowHandle, 9)
        [Native.Win]::SetForegroundWindow($parent.MainWindowHandle)
        exit 0
    }}
    $currentPid = $parentPid
}}
exit 1
"#,
        pid
    );

    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps_script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("PowerShell failed: {}", e))?;

    if output.status.success() {
        Ok("Focused terminal window".to_string())
    } else {
        Err("No se encontro ventana de terminal para este proceso".to_string())
    }
}

#[cfg(target_os = "macos")]
fn focus_parent_platform(_pid: u32) -> Result<String, String> {
    Err("Not supported on macOS yet".to_string())
}

#[cfg(target_os = "linux")]
fn focus_parent_platform(_pid: u32) -> Result<String, String> {
    Err("Not supported on Linux yet".to_string())
}

#[tauri::command]
pub fn open_terminal(path: String) -> Result<String, String> {
    let resolved = if path.is_empty() || path == "-" || path == "." {
        std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .unwrap_or_else(|_| ".".to_string())
    } else {
        path
    };
    open_terminal_platform(&resolved)
}

#[cfg(target_os = "windows")]
fn open_terminal_platform(path: &str) -> Result<String, String> {
    // Quote the path to handle spaces
    let quoted = format!("\"{}\"", path);

    // Try Warp first (if installed)
    if Command::new("cmd")
        .args(["/c", "start", "", "warp", "--cwd", path])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .is_ok()
    {
        return Ok("Opened Warp".to_string());
    }

    // Try Windows Terminal
    if Command::new("cmd")
        .args(["/c", "start", "", "wt", "-d", &quoted])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .is_ok()
    {
        return Ok("Opened Windows Terminal".to_string());
    }

    // Fallback: PowerShell (open visible, don't close)
    Command::new("cmd")
        .args(["/c", "start", "powershell", "-NoExit", "-Command", &format!("cd '{}'", path)])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("Failed to open terminal: {}", e))?;
    Ok("Opened PowerShell".to_string())
}

#[cfg(target_os = "macos")]
fn open_terminal_platform(path: &str) -> Result<String, String> {
    Command::new("open")
        .args(["-a", "Terminal", path])
        .spawn()
        .map_err(|e| format!("Failed to open terminal: {}", e))?;
    Ok("Opened Terminal".to_string())
}

#[cfg(target_os = "linux")]
fn open_terminal_platform(path: &str) -> Result<String, String> {
    // Try common terminal emulators
    for term in &["x-terminal-emulator", "gnome-terminal", "konsole", "xterm"] {
        if Command::new(term)
            .args(["--working-directory", path])
            .spawn()
            .is_ok()
        {
            return Ok(format!("Opened {}", term));
        }
    }
    Err("No terminal emulator found".to_string())
}

#[tauri::command]
pub fn kill_port(pid: u32) -> Result<String, String> {
    if pid == 0 {
        return Err("Cannot kill system process".to_string());
    }
    #[cfg(target_os = "windows")]
    if pid == 4 {
        return Err("Cannot kill system process".to_string());
    }

    kill_process(pid)
}

// ═══ CWD detection ═══

#[cfg(target_os = "windows")]
fn get_process_cwd(pid: u32) -> Option<String> {
    // Try PowerShell to get the actual working directory (CommandLine contains the project path)
    // First attempt: get CommandLine which often contains the project path
    let output = cmd("wmic")
        .args(["process", "where", &format!("ProcessId={}", pid), "get", "CommandLine", "/VALUE"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    for line in text.lines() {
        let line = line.trim();
        if let Some(cmdline) = line.strip_prefix("CommandLine=") {
            let cmdline = cmdline.trim();
            if !cmdline.is_empty() {
                // Extract project path from command line
                // NestJS example: "node C:\Users\user\projects\test\dist\main.js"
                // Bun example: "bun run --cwd C:\Users\user\projects\myapp dev"
                // Look for paths that contain common project indicators
                for part in cmdline.split_whitespace() {
                    let clean = part.trim_matches('"');
                    // Skip executable paths (node.exe, bun.exe, etc.)
                    if clean.ends_with(".exe") || clean.starts_with('-') {
                        continue;
                    }
                    // If it looks like a file path with project structure
                    if (clean.contains('\\') || clean.contains('/')) && !clean.contains("node_modules\\.bin") {
                        // Go up from the file to find the project root
                        let path = std::path::Path::new(clean);
                        let mut dir = if path.is_file() || clean.contains('.') {
                            path.parent()
                        } else {
                            Some(path)
                        };
                        // Walk up past dist/, src/, build/ etc. to find project root
                        while let Some(d) = dir {
                            let name = d.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                            if ["dist", "src", "build", "out", ".next", ".nuxt"].contains(&name.as_str()) {
                                dir = d.parent();
                            } else {
                                return Some(d.to_string_lossy().to_string());
                            }
                        }
                    }
                }
                // Fallback: return the commandline itself for manual extraction
                return Some(cmdline.to_string());
            }
        }
    }

    // Fallback: try ExecutablePath
    let output2 = cmd("wmic")
        .args(["process", "where", &format!("ProcessId={}", pid), "get", "ExecutablePath", "/VALUE"])
        .output()
        .ok()?;
    let text2 = String::from_utf8_lossy(&output2.stdout).to_string();
    for line in text2.lines() {
        let line = line.trim();
        if let Some(path) = line.strip_prefix("ExecutablePath=") {
            let path = path.trim();
            if !path.is_empty() {
                return Some(path.to_string());
            }
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn get_process_cwd(pid: u32) -> Option<String> {
    let output = cmd("lsof")
        .args(["-p", &pid.to_string(), "-Fn"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    // lsof -Fn outputs lines starting with 'n' for name; find cwd (type 'cwd' appears before it)
    let mut found_cwd = false;
    for line in text.lines() {
        if line == "fcwd" {
            found_cwd = true;
        } else if found_cwd && line.starts_with('n') {
            return Some(line[1..].to_string());
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn get_process_cwd(pid: u32) -> Option<String> {
    std::fs::read_link(format!("/proc/{}/cwd", pid))
        .ok()
        .map(|p| p.to_string_lossy().to_string())
}

// ═══ Command Line ═══

#[cfg(target_os = "windows")]
fn get_command_line(pid: u32) -> Option<String> {
    let output = cmd("wmic")
        .args(["process", "where", &format!("ProcessId={}", pid), "get", "CommandLine", "/VALUE"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    for line in text.lines() {
        let line = line.trim();
        if let Some(cmdline) = line.strip_prefix("CommandLine=") {
            let cmdline = cmdline.trim();
            if !cmdline.is_empty() {
                return Some(cmdline.to_string());
            }
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn get_command_line(pid: u32) -> Option<String> {
    let output = cmd("ps")
        .args(["-p", &pid.to_string(), "-o", "command="])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() { None } else { Some(text) }
}

#[cfg(target_os = "linux")]
fn get_command_line(pid: u32) -> Option<String> {
    std::fs::read_to_string(format!("/proc/{}/cmdline", pid))
        .ok()
        .map(|s| s.replace('\0', " ").trim().to_string())
        .filter(|s| !s.is_empty())
}

// ═══ Windows ═══

#[cfg(target_os = "windows")]
fn list_ports_platform() -> Vec<PortInfo> {
    let output = match cmd("netstat").args(["-ano", "-p", "tcp"]).output() {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(_) => return vec![],
    };

    let mut ports = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        if !line.contains("LISTENING") {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 5 {
            continue;
        }
        let port: u16 = match parts[1].rsplit(':').next().and_then(|p| p.parse().ok()) {
            Some(p) => p,
            None => continue,
        };
        let pid: u32 = match parts[4].parse() {
            Ok(p) if p > 0 => p,
            _ => continue,
        };
        ports.push(PortInfo {
            port,
            pid,
            process_name: "Unknown".to_string(),
            working_dir: String::new(),
            project_name: "-".to_string(),
            command_line: String::new(),
        });
    }
    ports
}

#[cfg(target_os = "windows")]
fn get_process_names(_ports: &[PortInfo]) -> HashMap<u32, String> {
    let mut map = HashMap::new();
    let output = match cmd("tasklist").args(["/FO", "CSV", "/NH"]).output() {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
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

#[cfg(target_os = "windows")]
fn kill_process(pid: u32) -> Result<String, String> {
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

// ═══ macOS ═══

#[cfg(target_os = "macos")]
fn list_ports_platform() -> Vec<PortInfo> {
    let output = match cmd("lsof")
        .args(["-iTCP", "-sTCP:LISTEN", "-n", "-P", "-F", "pcn"])
        .output()
    {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(_) => return vec![],
    };

    let mut ports = Vec::new();
    let mut current_pid: u32 = 0;
    let mut current_name = String::new();

    for line in output.lines() {
        if let Some(rest) = line.strip_prefix('p') {
            current_pid = rest.parse().unwrap_or(0);
        } else if let Some(rest) = line.strip_prefix('c') {
            current_name = rest.to_string();
        } else if let Some(rest) = line.strip_prefix('n') {
            // Format: n*:PORT or n[::1]:PORT
            if let Some(port_str) = rest.rsplit(':').next() {
                if let Ok(port) = port_str.parse::<u16>() {
                    if current_pid > 0 {
                        ports.push(PortInfo {
                            port,
                            pid: current_pid,
                            process_name: current_name.clone(),
                            working_dir: String::new(),
                            project_name: "-".to_string(),
                            command_line: String::new(),
                        });
                    }
                }
            }
        }
    }
    ports
}

#[cfg(target_os = "macos")]
fn get_process_names(_ports: &[PortInfo]) -> HashMap<u32, String> {
    // lsof already provides process names via -F c
    HashMap::new()
}

#[cfg(target_os = "macos")]
fn kill_process(pid: u32) -> Result<String, String> {
    let output = cmd("kill")
        .args(["-9", &pid.to_string()])
        .output()
        .map_err(|e| format!("Failed to execute kill: {}", e))?;
    if output.status.success() {
        Ok(format!("Process {} killed", pid))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("Failed to kill process {}: {}", pid, stderr.trim()))
    }
}

// ═══ Linux ═══

#[cfg(target_os = "linux")]
fn list_ports_platform() -> Vec<PortInfo> {
    // Try ss first (modern), fallback to netstat
    let output = match cmd("ss").args(["-tlnp"]).output() {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => match cmd("netstat").args(["-tlnp"]).output() {
            Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
            Err(_) => return vec![],
        },
    };

    let mut ports = Vec::new();
    for line in output.lines().skip(1) {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 5 {
            continue;
        }

        // Local address is typically column 3 for ss, format: *:PORT or 0.0.0.0:PORT
        let addr = parts[3];
        let port: u16 = match addr.rsplit(':').next().and_then(|p| p.parse().ok()) {
            Some(p) => p,
            None => continue,
        };

        // Process info is in the last column, format: users:(("name",pid=123,fd=4))
        let last = parts.last().unwrap_or(&"");
        let pid = extract_pid_from_ss(last);
        let name = extract_name_from_ss(last);

        if pid > 0 {
            ports.push(PortInfo {
                port,
                pid,
                process_name: name,
                working_dir: String::new(),
                project_name: "-".to_string(),
                command_line: String::new(),
            });
        }
    }
    ports
}

#[cfg(target_os = "linux")]
fn extract_pid_from_ss(s: &str) -> u32 {
    // Format: users:(("name",pid=12345,fd=4))
    s.split("pid=")
        .nth(1)
        .and_then(|rest| rest.split(|c: char| !c.is_ascii_digit()).next())
        .and_then(|p| p.parse().ok())
        .unwrap_or(0)
}

#[cfg(target_os = "linux")]
fn extract_name_from_ss(s: &str) -> String {
    // Format: users:(("name",pid=12345,fd=4))
    s.split("((\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .unwrap_or("Unknown")
        .to_string()
}

#[cfg(target_os = "linux")]
fn get_process_names(_ports: &[PortInfo]) -> HashMap<u32, String> {
    // ss already provides process names
    HashMap::new()
}

#[cfg(target_os = "linux")]
fn kill_process(pid: u32) -> Result<String, String> {
    let output = cmd("kill")
        .args(["-9", &pid.to_string()])
        .output()
        .map_err(|e| format!("Failed to execute kill: {}", e))?;
    if output.status.success() {
        Ok(format!("Process {} killed", pid))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("Failed to kill process {}: {}", pid, stderr.trim()))
    }
}
