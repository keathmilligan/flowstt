//! Linux-specific utility to list audio devices using pactl
//!
//! This binary is only functional on Linux systems with PulseAudio/PipeWire.
//! On other platforms, it will compile but display a "not supported" message.

#[cfg(target_os = "linux")]
mod linux {
    use std::process::Command;

    fn make_monitor_display_name(name: &str) -> String {
        if let Some(stripped) = name.strip_prefix("Monitor of ") {
            return stripped.to_string();
        }
        if let Some(stripped) = name.strip_suffix(".monitor") {
            if let Some(rest) = stripped.strip_prefix("alsa_output.") {
                let parts: Vec<&str> = rest.split('.').collect();
                
                if parts.len() >= 2 && parts[0].starts_with("usb-") {
                    let usb_part = parts[0].strip_prefix("usb-").unwrap_or(parts[0]);
                    let device_name = usb_part.rsplitn(2, '-').last().unwrap_or(usb_part);
                    let clean_name = device_name.replace('_', " ");
                    let output_type = parts.last().unwrap_or(&"output").replace('-', " ");
                    return format!("{} ({})", clean_name, output_type);
                }
                
                if parts.len() >= 2 && parts[0].starts_with("pci-") {
                    let output_type = parts.last().unwrap_or(&"output");
                    let friendly_type = match *output_type {
                        "analog-stereo" => "Speakers",
                        "hdmi-stereo" => "HDMI",
                        _ => output_type,
                    };
                    return friendly_type.to_string();
                }
            }
            
            let parts: Vec<&str> = stripped.split('.').collect();
            if let Some(last_part) = parts.last() {
                return last_part.replace(['-', '_'], " ");
            }
        }
        name.to_string()
    }

    pub fn run() {
        println!("=== System Audio (pactl) ===");
        if let Ok(output) = Command::new("pactl").args(["list", "sources", "short"]).output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() >= 5 {
                    let name = parts[1];
                    let state = parts[4];
                    if name.contains(".monitor") {
                        let display = make_monitor_display_name(name);
                        println!("  {} => \"{}\" [{}]", name, display, state);
                    }
                }
            }
        } else {
            println!("  Failed to run pactl - is PulseAudio/PipeWire installed?");
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod linux {
    pub fn run() {
        println!("This utility is only supported on Linux.");
        println!("It requires PulseAudio or PipeWire to enumerate audio devices.");
    }
}

fn main() {
    linux::run();
}
