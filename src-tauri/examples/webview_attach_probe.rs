//! Isolated WebView2 startup probe for P4-60.
//!
//! This example creates only a Tao window and a Wry WebView. It does not load
//! Arcana configuration, initialize the app, open a database, or register IPC.
//! The supplied profile directory must not exist; this process creates and owns it.

use std::{
    env, fs,
    path::{Component, Path, PathBuf},
    process,
    time::{Duration, Instant},
};

#[cfg(windows)]
use tauri_runtime_wry::{
    tao::{
        event::{Event, WindowEvent},
        event_loop::{ControlFlow, EventLoop},
        window::WindowBuilder,
    },
    wry::{WebContext, WebViewBuilder, WebViewBuilderExtWindows},
};

struct ProbeArgs {
    profile: PathBuf,
    runtime_dir: PathBuf,
    browser_args: String,
    lifetime: Duration,
}

fn parse_args() -> Result<ProbeArgs, &'static str> {
    let mut profile = None;
    let mut runtime_dir = None;
    let mut browser_args = None;
    let mut lifetime_seconds = None;
    let mut args = env::args().skip(1);

    while let Some(flag) = args.next() {
        let value = args.next().ok_or("missing-option-value")?;
        match flag.as_str() {
            "--profile" if profile.is_none() => profile = Some(PathBuf::from(value)),
            "--runtime-dir" if runtime_dir.is_none() => {
                runtime_dir = Some(PathBuf::from(value));
            }
            "--browser-args" if browser_args.is_none() => browser_args = Some(value),
            "--lifetime-seconds" if lifetime_seconds.is_none() => {
                let seconds = value
                    .parse::<u64>()
                    .map_err(|_| "invalid-lifetime-seconds")?;
                if !(1..=300).contains(&seconds) {
                    return Err("lifetime-out-of-range");
                }
                lifetime_seconds = Some(seconds);
            }
            "--profile" | "--runtime-dir" | "--browser-args" | "--lifetime-seconds" => {
                return Err("duplicate-option");
            }
            _ => return Err("unsupported-option"),
        }
    }

    let browser_args = validate_browser_args(browser_args.ok_or("missing-browser-args")?)?;
    Ok(ProbeArgs {
        profile: profile.ok_or("missing-profile")?,
        runtime_dir: runtime_dir.ok_or("missing-runtime-dir")?,
        browser_args,
        lifetime: Duration::from_secs(lifetime_seconds.ok_or("missing-lifetime-seconds")?),
    })
}

fn validate_browser_args(value: String) -> Result<String, &'static str> {
    if value.is_empty() || value.len() > 256 || !value.is_ascii() {
        return Err("invalid-browser-args");
    }

    let mut normalized = Vec::new();
    let mut has_remote_debugging_port = false;
    for argument in value.split_ascii_whitespace() {
        if argument == "--enable-gpu" || argument == "--use-angle=d3d11" {
            if normalized.contains(&argument) {
                return Err("duplicate-browser-arg");
            }
            normalized.push(argument);
            continue;
        }

        if let Some(port) = argument.strip_prefix("--remote-debugging-port=") {
            if has_remote_debugging_port
                || port.is_empty()
                || port.len() > 5
                || !port.bytes().all(|byte| byte.is_ascii_digit())
                || port.parse::<u16>().is_err()
            {
                return Err("invalid-remote-debugging-port");
            }
            has_remote_debugging_port = true;
            normalized.push(argument);
            continue;
        }

        return Err("unsupported-browser-arg");
    }

    if !has_remote_debugging_port {
        return Err("remote-debugging-port-required");
    }
    Ok(normalized.join(" "))
}

fn validate_runtime_dir(path: &Path) -> Result<PathBuf, &'static str> {
    if !path.is_absolute() || path.components().any(|part| part == Component::ParentDir) {
        return Err("runtime-dir-must-be-absolute");
    }
    let resolved = fs::canonicalize(path).map_err(|_| "runtime-dir-unavailable")?;
    if !resolved.is_dir() || !resolved.join("msedgewebview2.exe").is_file() {
        return Err("runtime-dir-missing-webview2-runtime");
    }
    Ok(resolved)
}

fn create_owned_profile(path: &Path) -> Result<PathBuf, &'static str> {
    if !path.is_absolute() || path.components().any(|part| part == Component::ParentDir) {
        return Err("profile-must-be-absolute");
    }
    let name = path
        .file_name()
        .ok_or("profile-path-needs-final-component")?;
    let parent = path.parent().ok_or("profile-parent-required")?;
    let parent = fs::canonicalize(parent).map_err(|_| "profile-parent-unavailable")?;
    if !parent.is_dir() {
        return Err("profile-parent-is-not-a-directory");
    }

    let owned_path = parent.join(name);
    fs::create_dir(&owned_path).map_err(|_| "profile-must-be-new-and-creatable")?;
    fs::canonicalize(&owned_path).map_err(|_| "profile-directory-unavailable")
}

#[cfg(windows)]
fn run_probe(args: ProbeArgs) -> Result<(), &'static str> {
    let runtime_dir = validate_runtime_dir(&args.runtime_dir)?;
    let profile = create_owned_profile(&args.profile)?;

    // Ensure the explicit arguments and profile are the only WebView2 overrides
    // inherited by the runtime child processes.
    env::remove_var("WEBVIEW2_USER_DATA_FOLDER");
    env::remove_var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS");
    env::set_var("WEBVIEW2_BROWSER_EXECUTABLE_FOLDER", runtime_dir);

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Arcana WebView2 Attach Probe")
        .build(&event_loop)
        .map_err(|_| "native-window-create-failed")?;
    let mut web_context = WebContext::new(Some(profile));
    let _webview = WebViewBuilder::new_with_web_context(&mut web_context)
        .with_url("about:blank")
        .with_additional_browser_args(args.browser_args)
        .build(&window)
        .map_err(|_| "webview-create-failed")?;

    let deadline = Instant::now() + args.lifetime;
    println!(
        "probe-window-ready=yes lifetime-seconds={}",
        args.lifetime.as_secs()
    );
    event_loop.run(move |event, _, control_flow| {
        if matches!(
            event,
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            }
        ) {
            println!("probe-stopped=window-close");
            *control_flow = ControlFlow::Exit;
        } else if Instant::now() >= deadline {
            println!("probe-stopped=deadline");
            *control_flow = ControlFlow::Exit;
        } else {
            *control_flow = ControlFlow::WaitUntil(deadline);
        }
    });
}

#[cfg(not(windows))]
fn run_probe(_args: ProbeArgs) -> Result<(), &'static str> {
    Err("windows-only-probe")
}

fn main() {
    let result = parse_args().and_then(run_probe);
    if let Err(error) = result {
        eprintln!("probe-error={error}");
        process::exit(1);
    }
}
