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
    /// Manage folder configuration
    Folder {
        #[command(subcommand)]
        command: FolderCommands,
    },
    /// Pause a device (or all devices)
    Pause {
        /// Device name or ID (full or short prefix)
        device: Option<String>,
        /// Pause all devices
        #[arg(long)]
        all: bool,
    },
    /// Unpause a device (or all devices)
    Unpause {
        /// Device name or ID (full or short prefix)
        device: Option<String>,
        /// Unpause all paused devices
        #[arg(long)]
        all: bool,
    },
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

#[derive(Subcommand)]
enum FolderCommands {
    /// Add a new folder and share it with devices
    Add {
        /// Folder ID
        id: String,
        /// Folder path on this Syncthing host
        #[arg(long)]
        path: String,
        /// Display label (defaults to folder ID)
        #[arg(long)]
        label: Option<String>,
        /// Device name or ID to share with; repeat for multiple devices
        #[arg(short, long = "device")]
        devices: Vec<String>,
    },
    /// Share an existing folder with devices
    Share {
        /// Folder ID
        id: String,
        /// Device name or ID to share with; repeat for multiple devices
        #[arg(short, long = "device")]
        devices: Vec<String>,
    },
    /// Remove a folder from this Syncthing host configuration
    Remove {
        /// Folder ID
        id: String,
    },
}

fn get_client(host_override: Option<&str>) -> Result<api::Client> {
    let cfg = config::load_config()?;
    let host = host_override
        .map(normalize_host)
        .unwrap_or_else(|| cfg.host().to_string());
    let api_key = api_key_for_host(&host)?;

    api::Client::new(&api_key, &host)
}

fn normalize_host(host: &str) -> String {
    if host.starts_with("http://") || host.starts_with("https://") {
        host.to_string()
    } else {
        format!("http://{}", host)
    }
}

fn api_key_for_host(normalized_host: &str) -> Result<String> {
    if should_use_local_api_key(normalized_host) {
        return config::get_local_api_key();
    }
    config::get_api_key()
}

fn should_use_local_api_key(normalized_host: &str) -> bool {
    is_local_host(normalized_host)
}

fn is_local_host(host: &str) -> bool {
    let without_scheme = host
        .strip_prefix("http://")
        .or_else(|| host.strip_prefix("https://"))
        .unwrap_or(host);
    let authority = without_scheme
        .split_once('/')
        .map(|(authority, _)| authority)
        .unwrap_or(without_scheme)
        .rsplit_once('@')
        .map(|(_, authority)| authority)
        .unwrap_or(without_scheme);
    let hostname = host_name(authority);

    matches!(hostname, "localhost" | "127.0.0.1" | "::1" | "[::1]")
}

fn host_name(authority: &str) -> &str {
    if let Some(rest) = authority.strip_prefix('[') {
        let Some((ipv6_host, _)) = rest.split_once(']') else {
            return authority;
        };
        return ipv6_host;
    }
    authority
        .split_once(':')
        .map(|(hostname, _)| hostname)
        .unwrap_or(authority)
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

async fn cmd_config(api_key: Option<String>, host: Option<String>) -> Result<()> {
    if api_key.is_none() && host.is_none() {
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
    Ok(())
}

async fn cmd_status(client: api::Client) -> Result<()> {
    let status = client.status().await?;
    let version = client.version().await?;
    let completion = client.db_completion().await?;

    print_version(&version);
    print_status_stats(&status);
    print_completion_stats(&completion);
    Ok(())
}

fn print_version(version: &serde_json::Value) {
    println!(
        "Syncthing {}",
        version
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
    );
    println!();
}

fn print_status_stats(status: &serde_json::Value) {
    let uptime = status.get("uptime").and_then(|u| u.as_u64()).unwrap_or(0);
    let hours = uptime / 3600;
    let mins = (uptime % 3600) / 60;
    println!("Uptime: {}h {}m", hours, mins);

    let alloc = status.get("alloc").and_then(|a| a.as_u64()).unwrap_or(0);
    let sys = status.get("sys").and_then(|s| s.as_u64()).unwrap_or(0);
    println!("Memory: {} / {}", format_bytes(alloc), format_bytes(sys));
}

fn print_completion_stats(completion: &serde_json::Value) {
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

async fn cmd_folders(client: api::Client, id: Option<String>) -> Result<()> {
    if let Some(folder_id) = id {
        return print_folder_detail(&client, &folder_id).await;
    }
    print_folder_overview(&client).await
}

async fn print_folder_detail(client: &api::Client, folder_id: &str) -> Result<()> {
    let status = client.db_status(folder_id).await?;
    println!("{}", serde_json::to_string_pretty(&status)?);
    Ok(())
}

async fn print_folder_overview(client: &api::Client) -> Result<()> {
    let folders = client.config_folders().await?;
    let Some(folders) = folders.as_array() else {
        return Ok(());
    };
    for folder in folders {
        print_folder_line(client, folder).await;
    }
    Ok(())
}

async fn print_folder_line(client: &api::Client, folder: &serde_json::Value) {
    let id = folder.get("id").and_then(|i| i.as_str()).unwrap_or("?");
    let label = folder
        .get("label")
        .and_then(|l| l.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(id);

    if folder
        .get("paused")
        .and_then(|p| p.as_bool())
        .unwrap_or(false)
    {
        println!("{:<20} paused", label);
        return;
    }

    let Ok(status) = client.db_status(id).await else {
        println!("{:<20} (status unavailable)", label);
        return;
    };

    let status_line = build_folder_status_line(&status);
    println!("{:<20} {}", label, status_line);
}

fn build_folder_status_line(status: &serde_json::Value) -> String {
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
    let errors = status.get("errors").and_then(|e| e.as_u64()).unwrap_or(0);

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
    status_parts.join(", ")
}

async fn cmd_devices(client: api::Client) -> Result<()> {
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
    Ok(())
}

async fn cmd_set_paused(
    client: api::Client,
    device: Option<String>,
    all: bool,
    paused: bool,
) -> Result<()> {
    let devices = client.config_devices().await?;
    let devices = devices
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("unexpected /rest/config/devices response"))?;

    let targets = select_devices(devices, device.as_deref(), all, paused)?;

    if targets.is_empty() {
        let action = if paused { "pause" } else { "unpause" };
        println!("No devices to {}", action);
        return Ok(());
    }

    for (id, name) in &targets {
        client.set_device_paused(id, paused).await?;
        let verb = if paused { "paused" } else { "unpaused" };
        println!("{} {} ({})", verb, name, &id[..7.min(id.len())]);
    }
    Ok(())
}

async fn cmd_folder(client: api::Client, command: FolderCommands) -> Result<()> {
    match command {
        FolderCommands::Add {
            id,
            path,
            label,
            devices,
        } => cmd_folder_add(client, id, path, label, devices).await,
        FolderCommands::Share { id, devices } => cmd_folder_share(client, id, devices).await,
        FolderCommands::Remove { id } => cmd_folder_remove(client, id).await,
    }
}

async fn cmd_folder_add(
    client: api::Client,
    id: String,
    path: String,
    label: Option<String>,
    devices: Vec<String>,
) -> Result<()> {
    let mut selected_devices = resolve_devices(&client, &devices).await?;
    include_current_device(&client, &mut selected_devices).await?;
    let label = label.unwrap_or_else(|| id.clone());
    let folder = build_folder_config(&id, &label, &path, &selected_devices);

    client.put_config_folder(&id, &folder).await?;
    println!(
        "added folder {} at {} shared with {}",
        id,
        path,
        device_names(&selected_devices)
    );
    Ok(())
}

async fn include_current_device(
    client: &api::Client,
    devices: &mut Vec<(String, String)>,
) -> Result<()> {
    let status = client.status().await?;
    let Some(device_id) = status.get("myID").and_then(|id| id.as_str()) else {
        return Ok(());
    };

    let current_device = find_current_device(client, device_id).await?;
    add_device_if_missing(devices, current_device);
    Ok(())
}

async fn find_current_device(client: &api::Client, device_id: &str) -> Result<(String, String)> {
    let devices = client.config_devices().await?;
    let configured_devices = devices
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("unexpected /rest/config/devices response"))?;

    let device = configured_devices
        .iter()
        .find(|device| device.get("deviceID").and_then(|id| id.as_str()) == Some(device_id));
    let name = device
        .and_then(|device| device.get("name"))
        .and_then(|name| name.as_str())
        .unwrap_or("this-device");
    Ok((device_id.to_string(), name.to_string()))
}

fn add_device_if_missing(devices: &mut Vec<(String, String)>, device: (String, String)) {
    let already_selected = devices.iter().any(|(device_id, _)| device_id == &device.0);
    if !already_selected {
        devices.push(device);
    }
}

async fn cmd_folder_share(client: api::Client, id: String, devices: Vec<String>) -> Result<()> {
    let selected_devices = resolve_devices(&client, &devices).await?;
    let mut folder = client.config_folder(&id).await?;

    add_devices_to_folder(&mut folder, &selected_devices)?;
    client.put_config_folder(&id, &folder).await?;
    println!(
        "shared folder {} with {}",
        id,
        device_names(&selected_devices)
    );
    Ok(())
}

async fn cmd_folder_remove(client: api::Client, id: String) -> Result<()> {
    client.delete_config_folder(&id).await?;
    println!("removed folder {}", id);
    Ok(())
}

async fn resolve_devices(
    client: &api::Client,
    queries: &[String],
) -> Result<Vec<(String, String)>> {
    if queries.is_empty() {
        anyhow::bail!("specify at least one --device");
    }

    let devices = client.config_devices().await?;
    let devices = devices
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("unexpected /rest/config/devices response"))?;

    let mut selected = Vec::new();
    for query in queries {
        selected.extend(select_devices(devices, Some(query), false, false)?);
    }
    Ok(selected)
}

fn build_folder_config(
    id: &str,
    label: &str,
    path: &str,
    devices: &[(String, String)],
) -> serde_json::Value {
    let min_disk_free = serde_json::json!({"value": 1, "unit": "%"});
    let versioning = serde_json::json!({
        "type": "",
        "params": {},
        "cleanupIntervalS": 3600,
        "fsPath": "",
        "fsType": "basic"
    });
    let xattr_filter = serde_json::json!({
        "entries": [],
        "maxSingleEntrySize": 1024,
        "maxTotalSize": 4096
    });

    serde_json::json!({
        "id": id,
        "label": label,
        "filesystemType": "basic",
        "path": path,
        "type": "sendreceive",
        "devices": folder_device_configs(devices),
        "rescanIntervalS": 3600,
        "fsWatcherEnabled": true,
        "fsWatcherDelayS": 10,
        "fsWatcherTimeoutS": 0,
        "ignorePerms": false,
        "autoNormalize": true,
        "minDiskFree": min_disk_free,
        "versioning": versioning,
        "copiers": 0,
        "pullerMaxPendingKiB": 0,
        "hashers": 0,
        "order": "random",
        "ignoreDelete": false,
        "scanProgressIntervalS": 0,
        "pullerPauseS": 0,
        "maxConflicts": 10,
        "disableSparseFiles": false,
        "disableTempIndexes": false,
        "paused": false,
        "weakHashThresholdPct": 25,
        "markerName": ".stfolder",
        "copyOwnershipFromParent": false,
        "modTimeWindowS": 0,
        "maxConcurrentWrites": 2,
        "disableFsync": false,
        "blockPullOrder": "standard",
        "copyRangeMethod": "standard",
        "caseSensitiveFS": false,
        "junctionsAsDirs": false,
        "syncOwnership": false,
        "sendOwnership": false,
        "syncXattrs": false,
        "sendXattrs": false,
        "xattrFilter": xattr_filter
    })
}

fn folder_device_configs(devices: &[(String, String)]) -> Vec<serde_json::Value> {
    devices
        .iter()
        .map(|(device_id, _)| {
            serde_json::json!({
                "deviceID": device_id,
                "introducedBy": "",
                "encryptionPassword": ""
            })
        })
        .collect()
}

fn add_devices_to_folder(
    folder: &mut serde_json::Value,
    devices: &[(String, String)],
) -> Result<()> {
    let existing_devices = folder
        .get_mut("devices")
        .and_then(|devices| devices.as_array_mut())
        .ok_or_else(|| anyhow::anyhow!("folder config has no devices array"))?;

    for (device_id, _) in devices {
        let already_shared = existing_devices
            .iter()
            .any(|device| device.get("deviceID").and_then(|id| id.as_str()) == Some(device_id));
        if already_shared {
            continue;
        }
        existing_devices.push(serde_json::json!({
            "deviceID": device_id,
            "introducedBy": "",
            "encryptionPassword": ""
        }));
    }
    Ok(())
}

fn device_names(devices: &[(String, String)]) -> String {
    devices
        .iter()
        .map(|(_, name)| name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn select_devices(
    devices: &[serde_json::Value],
    query: Option<&str>,
    all: bool,
    target_paused: bool,
) -> Result<Vec<(String, String)>> {
    if all && query.is_some() {
        anyhow::bail!("specify either a device or --all, not both");
    }

    if all {
        let current_paused = !target_paused;
        return Ok(devices
            .iter()
            .filter(|d| {
                d.get("paused").and_then(|p| p.as_bool()).unwrap_or(false) == current_paused
            })
            .filter_map(device_id_and_name)
            .collect());
    }

    let Some(query) = query else {
        anyhow::bail!("specify a device name/ID or --all");
    };

    let matches: Vec<(String, String)> = devices
        .iter()
        .filter(|d| device_matches(d, query))
        .filter_map(device_id_and_name)
        .collect();

    if matches.is_empty() {
        anyhow::bail!("no device matched '{}'", query);
    }
    if matches.len() > 1 {
        let names: Vec<String> = matches.into_iter().map(|(_, n)| n).collect();
        anyhow::bail!("'{}' matched multiple devices: {}", query, names.join(", "));
    }
    Ok(matches)
}

fn device_id_and_name(device: &serde_json::Value) -> Option<(String, String)> {
    let id = device.get("deviceID").and_then(|i| i.as_str())?;
    let name = device
        .get("name")
        .and_then(|n| n.as_str())
        .unwrap_or(id)
        .to_string();
    Some((id.to_string(), name))
}

fn device_matches(device: &serde_json::Value, query: &str) -> bool {
    let q = query.to_lowercase();
    let id = device
        .get("deviceID")
        .and_then(|i| i.as_str())
        .unwrap_or("");
    let name = device.get("name").and_then(|n| n.as_str()).unwrap_or("");
    name.eq_ignore_ascii_case(query)
        || id.eq_ignore_ascii_case(query)
        || id.to_lowercase().starts_with(&q)
}

async fn cmd_scan(client: api::Client, folder: Option<String>) -> Result<()> {
    if let Some(f) = folder {
        client.db_scan(&f).await?;
        println!("Scan triggered for folder: {}", f);
    } else {
        client.db_scan_all().await?;
        println!("Scan triggered for all folders");
    }
    Ok(())
}

async fn cmd_errors(client: api::Client, folder: Option<String>, clear: bool) -> Result<()> {
    if clear {
        client.clear_errors().await?;
        println!("Errors cleared");
        return Ok(());
    }
    if let Some(folder_id) = folder {
        return print_folder_errors(&client, &folder_id).await;
    }
    print_global_errors(&client).await?;
    Ok(())
}

async fn print_folder_errors(client: &api::Client, folder_id: &str) -> Result<()> {
    let errors = client.folder_errors(folder_id).await?;
    let Some(errs) = errors.get("errors").and_then(|e| e.as_array()) else {
        println!("No errors for folder '{}'", folder_id);
        return Ok(());
    };
    if errs.is_empty() {
        println!("No errors for folder '{}'", folder_id);
        return Ok(());
    }
    for err in errs {
        let path = err.get("path").and_then(|p| p.as_str()).unwrap_or("?");
        let error = err.get("error").and_then(|e| e.as_str()).unwrap_or("?");
        println!("{}: {}", path, error);
    }
    Ok(())
}

async fn print_global_errors(client: &api::Client) -> Result<()> {
    let errors = client.errors().await?;
    let Some(errs) = errors.get("errors").and_then(|e| e.as_array()) else {
        println!("No errors");
        return Ok(());
    };
    if errs.is_empty() {
        println!("No errors");
        return Ok(());
    }
    for err in errs {
        let when = err.get("when").and_then(|w| w.as_str()).unwrap_or("?");
        let msg = err.get("message").and_then(|m| m.as_str()).unwrap_or("?");
        println!("[{}] {}", format_duration_since(when), msg);
    }
    Ok(())
}

async fn cmd_pending(client: api::Client) -> Result<()> {
    let devices = client.pending_devices().await?;
    let folders = client.pending_folders().await?;

    println!("Pending Devices:");
    print_pending_devices(&devices);

    println!("\nPending Folders:");
    print_pending_folders(&folders);
    Ok(())
}

fn print_pending_devices(devices: &serde_json::Value) {
    let Some(devs) = devices.as_object() else {
        println!("  (none)");
        return;
    };
    if devs.is_empty() {
        println!("  (none)");
        return;
    }
    for (id, info) in devs {
        let name = info
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("unknown");
        println!("  {} ({})", name, &id[..7.min(id.len())]);
    }
}

fn print_pending_folders(folders: &serde_json::Value) {
    let lines = pending_folder_lines(folders);
    if lines.is_empty() {
        println!("  (none)");
        return;
    }
    for line in lines {
        println!("  {}", line);
    }
}

fn pending_folder_lines(folders: &serde_json::Value) -> Vec<String> {
    let Some(flds) = folders.as_object() else {
        return Vec::new();
    };

    let mut lines = Vec::new();
    for (folder_id, info) in flds {
        let Some(offered_by) = info.get("offeredBy").and_then(|o| o.as_object()) else {
            continue;
        };
        for (device_id, offer) in offered_by {
            let label = offer
                .get("label")
                .and_then(|l| l.as_str())
                .unwrap_or(folder_id);
            lines.push(format!(
                "{} from {}",
                label,
                &device_id[..7.min(device_id.len())]
            ));
        }
    }
    lines
}

async fn cmd_restart(client: api::Client) -> Result<()> {
    client.restart().await?;
    println!("Syncthing restart initiated");
    Ok(())
}

async fn cmd_shutdown(client: api::Client) -> Result<()> {
    client.shutdown().await?;
    println!("Syncthing shutdown initiated");
    Ok(())
}

async fn cmd_events(client: api::Client, limit: u32) -> Result<()> {
    let events = client.events(None, Some(limit)).await?;

    if let Some(events) = events.as_array() {
        for event in events.iter().rev().take(limit as usize) {
            let id = event.get("id").and_then(|i| i.as_u64()).unwrap_or(0);
            let event_type = event.get("type").and_then(|t| t.as_str()).unwrap_or("?");
            let time = event.get("time").and_then(|t| t.as_str()).unwrap_or("?");

            println!("[{}] {} - {}", id, format_duration_since(time), event_type);
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    run(cli).await
}

async fn run(cli: Cli) -> Result<()> {
    let host_override = cli.host.as_deref();

    match cli.command {
        Commands::Config { api_key, host } => cmd_config(api_key, host).await,
        command => {
            let client = get_client(host_override)?;
            dispatch_with_client(client, command).await
        }
    }
}

async fn dispatch_with_client(client: api::Client, command: Commands) -> Result<()> {
    match command {
        Commands::Status => cmd_status(client).await,
        Commands::Folders { id } => cmd_folders(client, id).await,
        Commands::Devices => cmd_devices(client).await,
        Commands::Folder { command } => cmd_folder(client, command).await,
        Commands::Pause { device, all } => cmd_set_paused(client, device, all, true).await,
        Commands::Unpause { device, all } => cmd_set_paused(client, device, all, false).await,
        Commands::Scan { folder } => cmd_scan(client, folder).await,
        Commands::Errors { folder, clear } => cmd_errors(client, folder, clear).await,
        Commands::Pending => cmd_pending(client).await,
        Commands::Restart => cmd_restart(client).await,
        Commands::Shutdown => cmd_shutdown(client).await,
        Commands::Events { limit } => cmd_events(client, limit).await,
        Commands::Config { .. } => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn pending_folder_lines_reads_syncthing_offered_by_shape() {
        let folders = json!({
            "wow-install": {
                "offeredBy": {
                    "3NF6RRU-U3MGBPO-ITEFILB-YYWGIYA-X3M3CQD-T6XIAGH-4OZEQ5I-MJC5RQ7": {
                        "label": "World of Warcraft Install",
                        "receiveEncrypted": false,
                        "remoteEncrypted": false
                    }
                }
            }
        });

        assert_eq!(
            pending_folder_lines(&folders),
            vec!["World of Warcraft Install from 3NF6RRU"]
        );
    }

    #[test]
    fn pending_folder_lines_returns_empty_for_no_pending_folders() {
        assert!(pending_folder_lines(&json!({})).is_empty());
    }

    #[test]
    fn is_local_host_accepts_common_loopback_forms() {
        assert!(is_local_host("http://localhost:8384"));
        assert!(is_local_host("127.0.0.1:8384"));
        assert!(is_local_host("http://[::1]:8384"));
    }

    #[test]
    fn is_local_host_rejects_remote_hosts() {
        assert!(!is_local_host("http://192.168.2.32:8384"));
        assert!(!is_local_host("nas:8384"));
    }

    #[test]
    fn configured_local_host_uses_local_api_key() {
        assert!(should_use_local_api_key("http://localhost:8384"));
    }

    fn sample_devices() -> Vec<serde_json::Value> {
        vec![
            json!({"deviceID": "AAAAAAA-BBBB", "name": "nas", "paused": true}),
            json!({"deviceID": "CCCCCCC-DDDD", "name": "alessio-desktop", "paused": false}),
            json!({"deviceID": "EEEEEEE-FFFF", "name": "aso", "paused": true}),
        ]
    }

    #[test]
    fn select_devices_all_unpause_returns_only_paused() {
        let devs = sample_devices();
        let selected = select_devices(&devs, None, true, false).unwrap();
        let names: Vec<&str> = selected.iter().map(|(_, n)| n.as_str()).collect();
        assert_eq!(names, vec!["nas", "aso"]);
    }

    #[test]
    fn select_devices_all_pause_returns_only_unpaused() {
        let devs = sample_devices();
        let selected = select_devices(&devs, None, true, true).unwrap();
        let names: Vec<&str> = selected.iter().map(|(_, n)| n.as_str()).collect();
        assert_eq!(names, vec!["alessio-desktop"]);
    }

    #[test]
    fn select_devices_matches_by_name_case_insensitive() {
        let devs = sample_devices();
        let selected = select_devices(&devs, Some("Nas"), false, false).unwrap();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].1, "nas");
    }

    #[test]
    fn select_devices_matches_by_id_prefix() {
        let devs = sample_devices();
        let selected = select_devices(&devs, Some("ccccccc"), false, false).unwrap();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].1, "alessio-desktop");
    }

    #[test]
    fn select_devices_errors_when_no_match() {
        let devs = sample_devices();
        let err = select_devices(&devs, Some("nope"), false, false).unwrap_err();
        assert!(err.to_string().contains("no device matched"));
    }

    #[test]
    fn select_devices_errors_when_all_and_query_combined() {
        let devs = sample_devices();
        let err = select_devices(&devs, Some("nas"), true, false).unwrap_err();
        assert!(err.to_string().contains("either a device or --all"));
    }

    #[test]
    fn select_devices_errors_when_neither_all_nor_query() {
        let devs = sample_devices();
        let err = select_devices(&devs, None, false, false).unwrap_err();
        assert!(err.to_string().contains("specify a device"));
    }

    #[test]
    fn build_folder_config_includes_devices_and_defaults() {
        let devices = vec![("CCCCCCC-DDDD".to_string(), "alessio-desktop".to_string())];
        let folder =
            build_folder_config("agent-config", "Agent Config", "~/agent-config", &devices);

        assert_eq!(folder["id"], "agent-config");
        assert_eq!(folder["label"], "Agent Config");
        assert_eq!(folder["path"], "~/agent-config");
        assert_eq!(folder["type"], "sendreceive");
        assert_eq!(folder["devices"][0]["deviceID"], "CCCCCCC-DDDD");
        assert_eq!(folder["rescanIntervalS"], 3600);
        assert_eq!(folder["fsWatcherEnabled"], true);
    }

    #[test]
    fn add_devices_to_folder_preserves_existing_and_skips_duplicates() {
        let mut folder = json!({
            "id": "agent-config",
            "devices": [{"deviceID": "AAAAAAA-BBBB"}]
        });
        let devices = vec![
            ("AAAAAAA-BBBB".to_string(), "nas".to_string()),
            ("CCCCCCC-DDDD".to_string(), "alessio-desktop".to_string()),
        ];

        add_devices_to_folder(&mut folder, &devices).unwrap();

        let device_ids: Vec<&str> = folder["devices"]
            .as_array()
            .unwrap()
            .iter()
            .map(|device| device["deviceID"].as_str().unwrap())
            .collect();
        assert_eq!(device_ids, vec!["AAAAAAA-BBBB", "CCCCCCC-DDDD"]);
    }

    #[test]
    fn add_device_if_missing_appends_only_new_devices() {
        let mut devices = vec![("AAAAAAA-BBBB".to_string(), "nas".to_string())];

        add_device_if_missing(
            &mut devices,
            ("AAAAAAA-BBBB".to_string(), "nas-duplicate".to_string()),
        );
        add_device_if_missing(
            &mut devices,
            ("CCCCCCC-DDDD".to_string(), "alessio-desktop".to_string()),
        );

        let names: Vec<&str> = devices.iter().map(|(_, name)| name.as_str()).collect();
        assert_eq!(names, vec!["nas", "alessio-desktop"]);
    }
}
