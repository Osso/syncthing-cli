mod api;
mod config;

use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "syncthing")]
#[command(about = "Syncthing CLI for monitoring and control")]
struct Cli {
    /// Host URL (e.g., 192.168.2.32:8384 or http://host:8384)
    #[arg(short = 'H', long, global = true)]
    host: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show system status
    Status,
    /// List folders with sync status
    Folders {
        /// Show detailed info for a specific folder
        #[arg(short, long)]
        id: Option<String>,
    },
    /// List connected devices
    Devices,
    /// Trigger folder rescan
    Scan {
        /// Folder ID (rescan all if not specified)
        folder: Option<String>,
    },
    /// Show sync errors
    Errors {
        /// Show errors for specific folder
        #[arg(short, long)]
        folder: Option<String>,
        /// Clear all errors
        #[arg(short, long)]
        clear: bool,
    },
    /// Show pending devices and folders
    Pending,
    /// Restart syncthing
    Restart,
    /// Shutdown syncthing
    Shutdown,
    /// Show recent events
    Events {
        /// Number of events to show
        #[arg(short, long, default_value = "20")]
        limit: u32,
    },
    /// Configure API key and host
    Config {
        /// API key
        #[arg(long)]
        api_key: Option<String>,
        /// Host URL (e.g., http://localhost:8384)
        #[arg(long)]
        host: Option<String>,
    },
}

fn get_client(host_override: Option<&str>) -> Result<api::Client> {
    let api_key = config::get_api_key()?;
    let cfg = config::load_config()?;

    let host = match host_override {
        Some(h) => {
            // Add http:// if no scheme provided
            if h.starts_with("http://") || h.starts_with("https://") {
                h.to_string()
            } else {
                format!("http://{}", h)
            }
        }
        None => cfg.host().to_string(),
    };

    api::Client::new(&api_key, &host)
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format!("{:.1} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

fn format_duration_since(timestamp: &str) -> String {
    if let Ok(dt) = DateTime::parse_from_rfc3339(timestamp) {
        let now = Utc::now();
        let duration = now.signed_duration_since(dt.with_timezone(&Utc));

        if duration.num_days() > 0 {
            format!("{}d ago", duration.num_days())
        } else if duration.num_hours() > 0 {
            format!("{}h ago", duration.num_hours())
        } else if duration.num_minutes() > 0 {
            format!("{}m ago", duration.num_minutes())
        } else {
            "just now".to_string()
        }
    } else {
        timestamp.to_string()
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let host_override = cli.host.as_deref();

    match cli.command {
        Commands::Config { api_key, host } => {
            if api_key.is_none() && host.is_none() {
                // Show current config
                let cfg = config::load_config()?;
                println!(
                    "API Key: {}",
                    cfg.api_key.as_deref().unwrap_or("(from syncthing config)")
                );
                println!("Host: {}", cfg.host());
            } else {
                let mut cfg = config::load_config()?;
                if let Some(key) = api_key {
                    cfg.api_key = Some(key);
                }
                if let Some(h) = host {
                    cfg.host = Some(h);
                }
                config::save_config(&cfg)?;
                eprintln!("Configuration saved");
            }
        }

        Commands::Status => {
            let client = get_client(host_override)?;
            let status = client.status().await?;
            let version = client.version().await?;
            let completion = client.db_completion().await?;

            println!(
                "Syncthing {}",
                version
                    .get("version")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
            );
            println!();

            let uptime = status.get("uptime").and_then(|u| u.as_u64()).unwrap_or(0);
            let hours = uptime / 3600;
            let mins = (uptime % 3600) / 60;
            println!("Uptime: {}h {}m", hours, mins);

            let alloc = status.get("alloc").and_then(|a| a.as_u64()).unwrap_or(0);
            let sys = status.get("sys").and_then(|s| s.as_u64()).unwrap_or(0);
            println!("Memory: {} / {}", format_bytes(alloc), format_bytes(sys));

            let global_bytes = completion
                .get("globalBytes")
                .and_then(|b| b.as_u64())
                .unwrap_or(0);
            let need_bytes = completion
                .get("needBytes")
                .and_then(|b| b.as_u64())
                .unwrap_or(0);
            let pct = completion
                .get("completion")
                .and_then(|c| c.as_f64())
                .unwrap_or(100.0);

            println!();
            println!("Sync: {:.1}% complete", pct);
            println!("Total: {}", format_bytes(global_bytes));
            if need_bytes > 0 {
                println!("Need: {}", format_bytes(need_bytes));
            }
        }

        Commands::Folders { id } => {
            let client = get_client(host_override)?;

            if let Some(folder_id) = id {
                let status = client.db_status(&folder_id).await?;
                println!("{}", serde_json::to_string_pretty(&status)?);
            } else {
                let folders = client.config_folders().await?;

                if let Some(folders) = folders.as_array() {
                    for folder in folders {
                        let id = folder.get("id").and_then(|i| i.as_str()).unwrap_or("?");
                        let label = folder
                            .get("label")
                            .and_then(|l| l.as_str())
                            .filter(|s| !s.is_empty())
                            .unwrap_or(id);
                        let paused = folder
                            .get("paused")
                            .and_then(|p| p.as_bool())
                            .unwrap_or(false);

                        if paused {
                            println!("{:<20} paused", label);
                            continue;
                        }

                        // Get sync status for this folder
                        match client.db_status(id).await {
                            Ok(status) => {
                                let state = status
                                    .get("state")
                                    .and_then(|s| s.as_str())
                                    .unwrap_or("unknown");
                                let need_files = status
                                    .get("needFiles")
                                    .and_then(|n| n.as_u64())
                                    .unwrap_or(0);
                                let need_bytes = status
                                    .get("needBytes")
                                    .and_then(|n| n.as_u64())
                                    .unwrap_or(0);
                                let errors =
                                    status.get("errors").and_then(|e| e.as_u64()).unwrap_or(0);

                                let mut status_parts = vec![state.to_string()];
                                if need_files > 0 {
                                    status_parts.push(format!(
                                        "{} files ({})",
                                        need_files,
                                        format_bytes(need_bytes)
                                    ));
                                }
                                if errors > 0 {
                                    status_parts.push(format!("{} errors", errors));
                                }

                                println!("{:<20} {}", label, status_parts.join(", "));
                            }
                            Err(_) => {
                                println!("{:<20} (status unavailable)", label);
                            }
                        }
                    }
                }
            }
        }

        Commands::Devices => {
            let client = get_client(host_override)?;
            let devices = client.config_devices().await?;
            let connections = client.connections().await?;
            let stats = client.stats_device().await?;

            if let Some(devices) = devices.as_array() {
                for device in devices {
                    let id = device
                        .get("deviceID")
                        .and_then(|i| i.as_str())
                        .unwrap_or("?");
                    let name = device.get("name").and_then(|n| n.as_str()).unwrap_or(id);
                    let short_id = &id[..7.min(id.len())];

                    let connected = connections
                        .get("connections")
                        .and_then(|c| c.get(id))
                        .and_then(|d| d.get("connected"))
                        .and_then(|c| c.as_bool())
                        .unwrap_or(false);

                    let last_seen = stats
                        .get(id)
                        .and_then(|s| s.get("lastSeen"))
                        .and_then(|t| t.as_str())
                        .map(format_duration_since)
                        .unwrap_or_else(|| "never".to_string());

                    let status = if connected { "connected" } else { "offline" };
                    println!(
                        "{:<20} ({}) {:<12} last: {}",
                        name, short_id, status, last_seen
                    );
                }
            }
        }

        Commands::Scan { folder } => {
            let client = get_client(host_override)?;
            if let Some(f) = folder {
                client.db_scan(&f).await?;
                println!("Scan triggered for folder: {}", f);
            } else {
                client.db_scan_all().await?;
                println!("Scan triggered for all folders");
            }
        }

        Commands::Errors { folder, clear } => {
            let client = get_client(host_override)?;
            if clear {
                client.clear_errors().await?;
                println!("Errors cleared");
            } else if let Some(folder_id) = folder {
                // Show folder-specific errors
                let errors = client.folder_errors(&folder_id).await?;
                if let Some(errs) = errors.get("errors").and_then(|e| e.as_array()) {
                    if errs.is_empty() {
                        println!("No errors for folder '{}'", folder_id);
                    } else {
                        for err in errs {
                            let path = err.get("path").and_then(|p| p.as_str()).unwrap_or("?");
                            let error = err.get("error").and_then(|e| e.as_str()).unwrap_or("?");
                            println!("{}: {}", path, error);
                        }
                    }
                } else {
                    println!("No errors for folder '{}'", folder_id);
                }
            } else {
                // Show system errors
                let errors = client.errors().await?;
                if let Some(errs) = errors.get("errors").and_then(|e| e.as_array()) {
                    if errs.is_empty() {
                        println!("No errors");
                    } else {
                        for err in errs {
                            let when = err.get("when").and_then(|w| w.as_str()).unwrap_or("?");
                            let msg = err.get("message").and_then(|m| m.as_str()).unwrap_or("?");
                            println!("[{}] {}", format_duration_since(when), msg);
                        }
                    }
                } else {
                    println!("No errors");
                }
            }
        }

        Commands::Pending => {
            let client = get_client(host_override)?;
            let devices = client.pending_devices().await?;
            let folders = client.pending_folders().await?;

            println!("Pending Devices:");
            if let Some(devs) = devices.as_object() {
                if devs.is_empty() {
                    println!("  (none)");
                } else {
                    for (id, info) in devs {
                        let name = info
                            .get("name")
                            .and_then(|n| n.as_str())
                            .unwrap_or("unknown");
                        println!("  {} ({})", name, &id[..7.min(id.len())]);
                    }
                }
            }

            println!("\nPending Folders:");
            if let Some(flds) = folders.as_object() {
                if flds.is_empty() {
                    println!("  (none)");
                } else {
                    for (device_id, device_folders) in flds {
                        if let Some(folders) = device_folders.as_object() {
                            for (folder_id, info) in folders {
                                let label = info
                                    .get("label")
                                    .and_then(|l| l.as_str())
                                    .unwrap_or(folder_id);
                                println!(
                                    "  {} from {}",
                                    label,
                                    &device_id[..7.min(device_id.len())]
                                );
                            }
                        }
                    }
                }
            }
        }

        Commands::Restart => {
            let client = get_client(host_override)?;
            client.restart().await?;
            println!("Syncthing restart initiated");
        }

        Commands::Shutdown => {
            let client = get_client(host_override)?;
            client.shutdown().await?;
            println!("Syncthing shutdown initiated");
        }

        Commands::Events { limit } => {
            let client = get_client(host_override)?;
            let events = client.events(None, Some(limit)).await?;

            if let Some(events) = events.as_array() {
                for event in events.iter().rev().take(limit as usize) {
                    let id = event.get("id").and_then(|i| i.as_u64()).unwrap_or(0);
                    let event_type = event.get("type").and_then(|t| t.as_str()).unwrap_or("?");
                    let time = event.get("time").and_then(|t| t.as_str()).unwrap_or("?");

                    println!("[{}] {} - {}", id, format_duration_since(time), event_type);
                }
            }
        }
    }

    Ok(())
}
