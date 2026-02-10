use std::io;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use tokio::io::copy_bidirectional;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio::time::sleep;

const DEFAULT_LISTEN_ADDR: &str = "0.0.0.0:25565";
const DEFAULT_BACKEND_ADDR: &str = "127.0.0.1:25566";
const DEFAULT_TARGET_BIN: &str = "/bin/pumpkin";
const DEFAULT_IDLE_SECS: u64 = 900;
const DEFAULT_RETRY_MS: u64 = 100;
const DEFAULT_RETRIES: usize = 80;
const DEFAULT_BASIC_CONFIG_PATH: &str = "config/basic.toml";

#[derive(Clone)]
struct WakeConfig {
    listen_addr: SocketAddr,
    backend_addr: SocketAddr,
    target_bin: String,
    target_args: Vec<String>,
    idle_timeout: Duration,
    retry_delay: Duration,
    retries: usize,
    write_backend_port: bool,
    basic_config_path: PathBuf,
}

struct ProcessState {
    child: Option<Child>,
    last_activity: Instant,
}

impl ProcessState {
    fn new() -> Self {
        Self {
            child: None,
            last_activity: Instant::now(),
        }
    }
}

impl Default for ProcessState {
    fn default() -> Self {
        Self::new()
    }
}

fn parse_socket_addr(var: &str, fallback: &str) -> io::Result<SocketAddr> {
    let value = std::env::var(var).unwrap_or_else(|_| fallback.to_string());
    value.parse().map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid {var} socket address '{value}': {error}"),
        )
    })
}

fn parse_u64(var: &str, fallback: u64) -> u64 {
    std::env::var(var)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(fallback)
}

fn parse_usize(var: &str, fallback: usize) -> usize {
    std::env::var(var)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(fallback)
}

fn parse_bool(var: &str, fallback: bool) -> bool {
    std::env::var(var).ok().map_or(fallback, |value| {
        value == "1" || value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("yes")
    })
}

fn parse_args(var: &str) -> Vec<String> {
    std::env::var(var).ok().map_or_else(Vec::new, |value| {
        value
            .split_ascii_whitespace()
            .map(ToOwned::to_owned)
            .collect()
    })
}

fn load_config() -> io::Result<WakeConfig> {
    Ok(WakeConfig {
        listen_addr: parse_socket_addr("PUMPKIN_WAKE_LISTEN_ADDR", DEFAULT_LISTEN_ADDR)?,
        backend_addr: parse_socket_addr("PUMPKIN_WAKE_BACKEND_ADDR", DEFAULT_BACKEND_ADDR)?,
        target_bin: std::env::var("PUMPKIN_WAKE_TARGET_BIN")
            .unwrap_or_else(|_| DEFAULT_TARGET_BIN.to_string()),
        target_args: parse_args("PUMPKIN_WAKE_TARGET_ARGS"),
        idle_timeout: Duration::from_secs(parse_u64("PUMPKIN_WAKE_IDLE_SECS", DEFAULT_IDLE_SECS)),
        retry_delay: Duration::from_millis(parse_u64("PUMPKIN_WAKE_RETRY_MS", DEFAULT_RETRY_MS)),
        retries: parse_usize("PUMPKIN_WAKE_RETRIES", DEFAULT_RETRIES),
        write_backend_port: parse_bool("PUMPKIN_WAKE_WRITE_BACKEND_PORT", true),
        basic_config_path: PathBuf::from(
            std::env::var("PUMPKIN_WAKE_BASIC_CONFIG_PATH")
                .unwrap_or_else(|_| DEFAULT_BASIC_CONFIG_PATH.to_string()),
        ),
    })
}

fn ensure_parent_dir(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn ensure_backend_java_port(config: &WakeConfig) -> io::Result<()> {
    if !config.write_backend_port {
        return Ok(());
    }

    ensure_parent_dir(&config.basic_config_path)?;

    let line = format!("java_edition_address = \"{}\"\n", config.backend_addr);
    if !config.basic_config_path.exists() {
        std::fs::write(&config.basic_config_path, line)?;
        return Ok(());
    }

    let original = std::fs::read_to_string(&config.basic_config_path)?;
    let mut replaced = false;
    let mut merged = String::with_capacity(original.len() + line.len());
    for row in original.lines() {
        if row.trim_start().starts_with("java_edition_address") {
            merged.push_str(&line);
            replaced = true;
        } else {
            merged.push_str(row);
            merged.push('\n');
        }
    }
    if !replaced {
        merged.push('\n');
        merged.push_str(&line);
    }
    std::fs::write(&config.basic_config_path, merged)?;
    Ok(())
}

fn child_is_alive(child: &mut Child) -> io::Result<bool> {
    child.try_wait().map(|status| status.is_none())
}

fn spawn_backend(config: &WakeConfig) -> io::Result<Child> {
    ensure_backend_java_port(config)?;

    let mut command = Command::new(&config.target_bin);
    if !config.target_args.is_empty() {
        command.args(&config.target_args);
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    command.spawn().map_err(|error| {
        io::Error::other(format!(
            "failed to spawn backend '{}': {error}",
            config.target_bin
        ))
    })
}

async fn backend_ready(config: &WakeConfig) -> bool {
    let mut attempts = 0usize;
    while attempts < config.retries {
        if TcpStream::connect(config.backend_addr).await.is_ok() {
            return true;
        }
        attempts += 1;
        sleep(config.retry_delay).await;
    }
    false
}

async fn ensure_backend_running(
    config: &WakeConfig,
    state: &Arc<Mutex<ProcessState>>,
) -> io::Result<()> {
    {
        let mut guard = state.lock().await;
        guard.last_activity = Instant::now();

        let should_spawn = if let Some(child) = guard.child.as_mut() {
            !child_is_alive(child)?
        } else {
            true
        };

        if should_spawn {
            guard.child = Some(spawn_backend(config)?);
            guard.last_activity = Instant::now();
        }
    }

    if backend_ready(config).await {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "backend did not become ready before timeout",
        ))
    }
}

async fn proxy_connection(
    mut inbound: TcpStream,
    config: Arc<WakeConfig>,
    state: Arc<Mutex<ProcessState>>,
    active: Arc<AtomicUsize>,
) {
    if ensure_backend_running(config.as_ref(), &state)
        .await
        .is_err()
    {
        return;
    }

    let Ok(mut outbound) = TcpStream::connect(config.backend_addr).await else {
        return;
    };

    active.fetch_add(1, Ordering::Relaxed);
    let mut guard = state.lock().await;
    guard.last_activity = Instant::now();
    drop(guard);

    let _ = copy_bidirectional(&mut inbound, &mut outbound).await;

    active.fetch_sub(1, Ordering::Relaxed);
    let mut guard = state.lock().await;
    guard.last_activity = Instant::now();
}

fn terminate_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

async fn idle_reaper(
    config: Arc<WakeConfig>,
    state: Arc<Mutex<ProcessState>>,
    active: Arc<AtomicUsize>,
) {
    loop {
        sleep(Duration::from_secs(1)).await;

        if active.load(Ordering::Relaxed) != 0 {
            continue;
        }

        let mut guard = state.lock().await;
        let elapsed = guard.last_activity.elapsed();
        if elapsed < config.idle_timeout {
            continue;
        }

        if let Some(child) = guard.child.as_mut() {
            match child_is_alive(child) {
                Ok(true) => {
                    terminate_child(child);
                    guard.child = None;
                    guard.last_activity = Instant::now();
                }
                Ok(false) | Err(_) => {
                    guard.child = None;
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let config = Arc::new(load_config()?);
    let state = Arc::new(Mutex::new(ProcessState::new()));
    let active = Arc::new(AtomicUsize::new(0));

    let listener = TcpListener::bind(config.listen_addr).await?;

    tokio::spawn(idle_reaper(config.clone(), state.clone(), active.clone()));

    loop {
        let (socket, _) = listener.accept().await?;
        tokio::spawn(proxy_connection(
            socket,
            config.clone(),
            state.clone(),
            active.clone(),
        ));
    }
}
