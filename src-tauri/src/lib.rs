use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::{distributions::Alphanumeric, Rng};
use rustfft::{num_complex::Complex, FftPlanner};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::VecDeque,
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, RwLock,
    },
    thread,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager, Position, Size, WebviewUrl, WebviewWindowBuilder,
};
use tauri_plugin_autostart::ManagerExt as AutostartManagerExt;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    time::{timeout, Duration},
};
use url::Url;

const REDIRECT_URI: &str = "http://127.0.0.1:8888/callback";
const SCOPES: &str = "user-read-playback-state user-modify-playback-state user-read-currently-playing user-top-read user-read-recently-played user-library-read playlist-read-private playlist-read-collaborative";
#[derive(Default, Serialize, Deserialize)]
struct Persisted {
    #[serde(default)]
    client_id: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    access_token: Option<String>,
    #[serde(default)]
    expires_at: u64,
}

struct AppState {
    config: Mutex<Persisted>,
    config_path: PathBuf,
    hit_rects: Arc<RwLock<Vec<HitRect>>>,
    ignore_requested: Arc<AtomicBool>,
    audio_stop: Mutex<Option<Arc<AtomicBool>>>,
}

#[derive(Clone, Copy, Deserialize)]
struct HitRect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MonitorInfo {
    index: usize,
    name: String,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    primary: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FrontendConfig {
    client_id: String,
    redirect: &'static str,
    logged_in: bool,
    autostart: bool,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: u64,
    refresh_token: Option<String>,
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn save(state: &AppState) -> Result<(), String> {
    let cfg = state.config.lock().map_err(|e| e.to_string())?;
    if let Some(parent) = state.config_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let data = serde_json::to_vec_pretty(&*cfg).map_err(|e| e.to_string())?;
    fs::write(&state.config_path, data).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_cfg(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<FrontendConfig, String> {
    let cfg = state.config.lock().map_err(|e| e.to_string())?;
    let autostart = app.autolaunch().is_enabled().unwrap_or(false);
    Ok(FrontendConfig {
        client_id: cfg.client_id.clone(),
        redirect: REDIRECT_URI,
        logged_in: cfg.refresh_token.is_some() || cfg.access_token.is_some(),
        autostart,
    })
}

#[tauri::command]
fn set_client_id(client_id: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let value = client_id.trim().to_owned();
    if value.is_empty() {
        return Err("Ingresa un Client ID de Spotify".into());
    }
    state.config.lock().map_err(|e| e.to_string())?.client_id = value;
    save(&state)
}

#[tauri::command]
async fn spotify_login(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let client_id = state
        .config
        .lock()
        .map_err(|e| e.to_string())?
        .client_id
        .clone();
    if client_id.is_empty() {
        return Err("Ingresa el Client ID de Spotify".into());
    }
    let verifier: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(64)
        .map(char::from)
        .collect();
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let nonce: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();
    let mut auth =
        Url::parse("https://accounts.spotify.com/authorize").map_err(|e| e.to_string())?;
    auth.query_pairs_mut()
        .append_pair("client_id", &client_id)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", REDIRECT_URI)
        .append_pair("scope", SCOPES)
        .append_pair("state", &nonce)
        .append_pair("code_challenge_method", "S256")
        .append_pair("code_challenge", &challenge);

    let listener = TcpListener::bind("127.0.0.1:8888")
        .await
        .map_err(|e| format!("No se pudo abrir {REDIRECT_URI}: {e}"))?;
    open::that(auth.as_str()).map_err(|e| format!("No se pudo abrir el navegador: {e}"))?;
    let callback = timeout(Duration::from_secs(180), async {
        let (mut stream, _) = listener.accept().await.map_err(|e| e.to_string())?;
        let mut buf = [0u8; 8192];
        let n = stream.read(&mut buf).await.map_err(|e| e.to_string())?;
        let request = String::from_utf8_lossy(&buf[..n]);
        let target = request.lines().next().and_then(|l| l.split_whitespace().nth(1)).ok_or("Respuesta OAuth inválida")?;
        let url = Url::parse(&format!("http://127.0.0.1{target}")).map_err(|e| e.to_string())?;
        let params: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        let state_value = params.get("state").ok_or("Falta state de OAuth")?;
        if state_value != &nonce { return Err("El state de OAuth no coincide".to_string()); }
        let code = params.get("code").ok_or_else(|| params.get("error").cloned().unwrap_or_else(|| "Spotify canceló la autorización".into()))?.clone();
        let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\n\r\n<!doctype html><meta charset=utf-8><title>Vinyl Desktop</title><p>Spotify conectado. Puedes cerrar esta pestaña.</p>";
        let _ = stream.write_all(response.as_bytes()).await;
        Ok::<String, String>(code)
    }).await.map_err(|_| "Tiempo de espera agotado al autorizar Spotify".to_string())??;

    let response = reqwest::Client::new()
        .post("https://accounts.spotify.com/api/token")
        .form(&[
            ("client_id", client_id.as_str()),
            ("grant_type", "authorization_code"),
            ("code", callback.as_str()),
            ("redirect_uri", REDIRECT_URI),
            ("code_verifier", verifier.as_str()),
        ])
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!(
            "Spotify rechazó la autorización ({})",
            response.status()
        ));
    }
    let token: TokenResponse = response.json().await.map_err(|e| e.to_string())?;
    {
        let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
        cfg.access_token = Some(token.access_token);
        if token.refresh_token.is_some() {
            cfg.refresh_token = token.refresh_token;
        }
        cfg.expires_at = unix_now() + token.expires_in;
    }
    save(&state)?;
    let _ = app.emit("vinyl://auth", true);
    Ok(())
}

#[tauri::command]
async fn spotify_token(state: tauri::State<'_, AppState>) -> Result<Option<String>, String> {
    let (access, refresh, client_id, expiry) = {
        let cfg = state.config.lock().map_err(|e| e.to_string())?;
        (
            cfg.access_token.clone(),
            cfg.refresh_token.clone(),
            cfg.client_id.clone(),
            cfg.expires_at,
        )
    };
    if let Some(token) = access.filter(|_| expiry > unix_now() + 60) {
        return Ok(Some(token));
    }
    let Some(refresh) = refresh else {
        return Ok(None);
    };
    let response = reqwest::Client::new()
        .post("https://accounts.spotify.com/api/token")
        .form(&[
            ("client_id", client_id.as_str()),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh.as_str()),
        ])
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("Error al renovar Spotify ({})", response.status()));
    }
    let token: TokenResponse = response.json().await.map_err(|e| e.to_string())?;
    let fresh = token.access_token.clone();
    {
        let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
        cfg.access_token = Some(fresh.clone());
        if token.refresh_token.is_some() {
            cfg.refresh_token = token.refresh_token;
        }
        cfg.expires_at = unix_now() + token.expires_in;
    }
    save(&state)?;
    Ok(Some(fresh))
}

#[tauri::command]
fn spotify_logout(state: tauri::State<'_, AppState>) -> Result<(), String> {
    {
        let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
        cfg.access_token = None;
        cfg.refresh_token = None;
        cfg.expires_at = 0;
    }
    save(&state)
}

#[tauri::command]
fn set_autostart(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let manager = app.autolaunch();
    if enabled {
        manager.enable()
    } else {
        manager.disable()
    }
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn ignore_mouse(app: tauri::AppHandle, ignore: bool) -> Result<(), String> {
    let state = app.state::<AppState>();
    state.ignore_requested.store(ignore, Ordering::Relaxed);
    if !ignore {
        app.get_webview_window("main")
            .ok_or("No se encontró la ventana principal")?
            .set_ignore_cursor_events(ignore)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn wall_active(app: tauri::AppHandle, on: bool) -> Result<(), String> {
    if on {
        let window = app
            .get_webview_window("main")
            .ok_or("No se encontró la ventana principal")?;
        place_behind_desktop(&window)?;
    }
    Ok(())
}

// Place the transparent window behind the Windows desktop icons. HWND_BOTTOM
// alone is not enough: Explorer's desktop windows otherwise paint over it.
#[cfg(target_os = "windows")]
fn place_behind_desktop(window: &tauri::WebviewWindow) -> Result<(), String> {
    use std::{ffi::OsStr, os::windows::ffi::OsStrExt};

    #[link(name = "user32")]
    extern "system" {
        fn SetWindowPos(
            hwnd: isize,
            after: isize,
            x: i32,
            y: i32,
            cx: i32,
            cy: i32,
            flags: u32,
        ) -> i32;
        fn FindWindowW(class_name: *const u16, window_name: *const u16) -> isize;
        fn FindWindowExW(
            parent: isize,
            after: isize,
            class_name: *const u16,
            window_name: *const u16,
        ) -> isize;
    }

    fn wide(value: &str) -> Vec<u16> {
        OsStr::new(value)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    const HWND_BOTTOM: isize = 1;
    const SWP_FLAGS: u32 = 0x0013; // SWP_NOSIZE | SWP_NOMOVE | SWP_NOACTIVATE
    let hwnd = window.hwnd().map_err(|error| error.to_string())?.0 as isize;
    unsafe {
        if SetWindowPos(hwnd, HWND_BOTTOM, 0, 0, 0, 0, SWP_FLAGS) == 0 {
            return Err("No se pudo enviar la ventana al fondo".into());
        }
        let progman = FindWindowW(wide("Progman").as_ptr(), std::ptr::null());
        if progman != 0 {
            SetWindowPos(progman, hwnd, 0, 0, 0, 0, SWP_FLAGS);
        }
        let mut worker = 0;
        loop {
            worker = FindWindowExW(0, worker, wide("WorkerW").as_ptr(), std::ptr::null());
            if worker == 0 {
                break;
            }
            SetWindowPos(worker, hwnd, 0, 0, 0, 0, SWP_FLAGS);
        }
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn place_behind_desktop(_window: &tauri::WebviewWindow) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
fn set_hit_rects(rects: Vec<HitRect>, state: tauri::State<'_, AppState>) {
    if let Ok(mut current) = state.hit_rects.write() {
        *current = rects;
    }
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn start_audio_loopback(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut worker = state.audio_stop.lock().map_err(|e| e.to_string())?;
    if worker
        .as_ref()
        .is_some_and(|stop| !stop.load(Ordering::Relaxed))
    {
        return Ok(());
    }
    let stop = Arc::new(AtomicBool::new(false));
    *worker = Some(stop.clone());
    drop(worker);
    thread::spawn(move || {
        while !stop.load(Ordering::Relaxed) {
            if let Err(error) = capture_system_audio(app.clone(), stop.clone()) {
                let _ = app.emit(
                    "vinyl://audio-status",
                    serde_json::json!({"ok": false, "error": error}),
                );
                if !stop.load(Ordering::Relaxed) {
                    thread::sleep(std::time::Duration::from_millis(700));
                }
            } else {
                break;
            }
        }
    });
    Ok(())
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
fn start_audio_loopback(
    _app: tauri::AppHandle,
    _state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    Err("La captura de audio solo está disponible en Windows".into())
}

#[tauri::command]
fn stop_audio_loopback(state: tauri::State<'_, AppState>) -> Result<(), String> {
    if let Some(stop) = state.audio_stop.lock().map_err(|e| e.to_string())?.take() {
        stop.store(true, Ordering::Relaxed);
    }
    Ok(())
}

#[tauri::command]
fn get_monitors(app: tauri::AppHandle) -> Result<Vec<MonitorInfo>, String> {
    let monitors = app.available_monitors().map_err(|e| e.to_string())?;
    let primary = app.primary_monitor().map_err(|e| e.to_string())?;
    Ok(monitors
        .into_iter()
        .enumerate()
        .map(|(index, monitor)| {
            let position = monitor.position();
            let size = monitor.size();
            let is_primary = primary.as_ref().is_some_and(|p| {
                p.position().x == position.x && p.position().y == position.y
            });
            MonitorInfo {
                index,
                name: monitor
                    .name()
                    .cloned()
                    .unwrap_or_else(|| format!("Pantalla {}", index + 1)),
                x: position.x,
                y: position.y,
                width: size.width,
                height: size.height,
                primary: is_primary,
            }
        })
        .collect())
}

#[tauri::command]
fn set_monitor(index: usize, app: tauri::AppHandle) -> Result<(), String> {
    let monitor = app
        .available_monitors()
        .map_err(|e| e.to_string())?
        .into_iter()
        .nth(index)
        .ok_or_else(|| "No se encontró la pantalla seleccionada".to_string())?;
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "No se encontró la ventana principal".to_string())?;
    window
        .set_position(Position::Physical(*monitor.position()))
        .map_err(|e| e.to_string())?;
    window
        .set_size(Size::Physical(*monitor.size()))
        .map_err(|e| e.to_string())
}

#[cfg(target_os = "windows")]
fn capture_system_audio(app: tauri::AppHandle, stop: Arc<AtomicBool>) -> Result<(), String> {
    use wasapi::{initialize_mta, DeviceEnumerator, Direction, SampleType, StreamMode, WaveFormat};

    initialize_mta()
        .ok()
        .map_err(|e| format!("No se pudo inicializar WASAPI: {e:?}"))?;
    let device = DeviceEnumerator::new()
        .and_then(|enumerator| enumerator.get_default_device(Direction::Render))
        .map_err(|e| format!("No se encontró la salida de audio predeterminada: {e:?}"))?;
    let device_id = device
        .get_id()
        .map_err(|e| format!("No se pudo identificar la salida de audio: {e:?}"))?;
    let mut client = device
        .get_iaudioclient()
        .map_err(|e| format!("WASAPI: {e:?}"))?;
    let format = WaveFormat::new(32, 32, SampleType::Float, 48_000, 2, None);
    client
        .initialize_client(
            &format,
            Direction::Capture,
            StreamMode::PollingShared {
                autoconvert: true,
                buffer_duration_hns: 0,
            },
            None,
        )
        .map_err(|e| format!("No se pudo iniciar la captura del audio del sistema: {e:?}"))?;
    let capture = client
        .get_audiocaptureclient()
        .map_err(|e| format!("WASAPI: {e:?}"))?;
    client
        .start_stream()
        .map_err(|e| format!("WASAPI: {e:?}"))?;
    let _ = app.emit("vinyl://audio-status", serde_json::json!({"ok": true}));

    const FFT_SIZE: usize = 2048;
    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(FFT_SIZE);
    let mut ring = VecDeque::<f32>::with_capacity(FFT_SIZE);
    let mut bytes = VecDeque::<u8>::new();
    let mut bins = vec![Complex::new(0.0f32, 0.0f32); FFT_SIZE];
    let mut last_emit = std::time::Instant::now();
    let mut last_device_check = std::time::Instant::now();

    while !stop.load(Ordering::Relaxed) {
        if last_device_check.elapsed() >= std::time::Duration::from_secs(1) {
            last_device_check = std::time::Instant::now();
            let current = DeviceEnumerator::new()
                .and_then(|enumerator| enumerator.get_default_device(Direction::Render))
                .and_then(|device| device.get_id());
            if current
                .as_deref()
                .map(|id| id != device_id.as_str())
                .unwrap_or(true)
            {
                let _ = client.stop_stream();
                return Err("La salida de audio cambió; reiniciando la captura".into());
            }
        }
        match capture.get_next_packet_size() {
            Ok(Some(frames)) if frames > 0 => {
                capture
                    .read_from_device_to_deque(&mut bytes)
                    .map_err(|e| format!("Error leyendo audio: {e:?}"))?;
                while bytes.len() >= 8 {
                    let left = f32::from_le_bytes([
                        bytes.pop_front().unwrap(),
                        bytes.pop_front().unwrap(),
                        bytes.pop_front().unwrap(),
                        bytes.pop_front().unwrap(),
                    ]);
                    let right = f32::from_le_bytes([
                        bytes.pop_front().unwrap(),
                        bytes.pop_front().unwrap(),
                        bytes.pop_front().unwrap(),
                        bytes.pop_front().unwrap(),
                    ]);
                    if ring.len() == FFT_SIZE {
                        ring.pop_front();
                    }
                    ring.push_back((left + right) * 0.5);
                }
                if ring.len() == FFT_SIZE
                    && last_emit.elapsed() >= std::time::Duration::from_millis(32)
                {
                    for (i, sample) in ring.iter().enumerate() {
                        let window = 0.5
                            - 0.5
                                * (std::f32::consts::TAU * i as f32 / (FFT_SIZE - 1) as f32).cos();
                        bins[i] = Complex::new(sample * window, 0.0);
                    }
                    fft.process(&mut bins);
                    let spectrum: Vec<u8> = bins[..FFT_SIZE / 2]
                        .iter()
                        .map(|bin| {
                            let magnitude = bin.norm() / (FFT_SIZE as f32 * 0.25);
                            let db = 20.0 * magnitude.max(1e-6).log10();
                            (((db + 90.0) / 70.0) * 255.0).clamp(0.0, 255.0) as u8
                        })
                        .collect();
                    let _ = app.emit("vinyl://audio-spectrum", spectrum);
                    last_emit = std::time::Instant::now();
                }
            }
            Ok(_) => thread::sleep(std::time::Duration::from_millis(8)),
            Err(e) => return Err(format!("Error leyendo audio del sistema: {e:?}")),
        }
    }
    let _ = client.stop_stream();
    Ok(())
}

#[cfg(target_os = "windows")]
fn cursor_position() -> Option<windows_sys::Win32::Foundation::POINT> {
    let mut point = windows_sys::Win32::Foundation::POINT { x: 0, y: 0 };
    let ok = unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut point) };
    (ok != 0).then_some(point)
}

fn build_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    let toggle = MenuItem::with_id(
        app,
        "toggle",
        "Reproducir / Pausar",
        true,
        None::<&str>,
    )?;
    let next = MenuItem::with_id(app, "next", "Siguiente canción", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Salir", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&toggle, &next, &separator, &quit])?;

    TrayIconBuilder::new()
        .icon(
            app.default_window_icon()
                .cloned()
                .ok_or_else(|| tauri::Error::AssetNotFound("icons/icon.ico".into()))?,
        )
        .menu(&menu)
        .tooltip("Vinyl Desktop")
        .on_menu_event(|app, event| match event.id().as_ref() {
            "toggle" | "next" => {
                let _ = app.emit("vinyl://command", event.id().as_ref().to_string());
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            let config_path = app.path().app_config_dir()?.join("config.json");
            let config = fs::read(&config_path)
                .ok()
                .and_then(|bytes| serde_json::from_slice(&bytes).ok())
                .unwrap_or_default();
            app.manage(AppState {
                config: Mutex::new(config),
                config_path,
                hit_rects: Arc::new(RwLock::new(Vec::new())),
                // Match Electron's click-through startup: desktop clicks pass
                // through except over the widget's reported interactive areas.
                ignore_requested: Arc::new(AtomicBool::new(true)),
                audio_stop: Mutex::new(None),
            });
            build_tray(app.handle())?;
            let (width, height, x, y) = if let Some(monitor) = app.primary_monitor()? {
                let scale = monitor.scale_factor();
                let size = monitor.size();
                let pos = monitor.position();
                (
                    size.width as f64 / scale,
                    size.height as f64 / scale,
                    pos.x as f64 / scale,
                    pos.y as f64 / scale,
                )
            } else {
                (1280.0, 720.0, 0.0, 0.0)
            };
            let window =
                WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                    .title("Vinyl Desktop")
                    .position(x, y)
                    .inner_size(width, height)
                    .transparent(true)
                    .decorations(false)
                    .always_on_bottom(true)
                    .skip_taskbar(true)
                    .shadow(false)
                    .build()?;
            window.set_ignore_cursor_events(true)?;
            // Tauri's always-on-bottom keeps the widget below normal windows;
            // also order it behind Explorer so the desktop wallpaper mode works.
            if let Err(error) = place_behind_desktop(&window) {
                eprintln!("No se pudo anclar Vinyl Desktop al fondo: {error}");
            }
            #[cfg(target_os = "windows")]
            {
                let app_handle = app.handle().clone();
                let anchor_handle = app_handle.clone();
                thread::spawn(move || loop {
                    thread::sleep(std::time::Duration::from_millis(1500));
                    if let Some(window) = anchor_handle.get_webview_window("main") {
                        let _ = place_behind_desktop(&window);
                    }
                });
                let state = app.state::<AppState>();
                let rects = state.hit_rects.clone();
                let ignore_requested = state.ignore_requested.clone();
                thread::spawn(move || {
                    let mut is_ignored = true;
                    loop {
                        thread::sleep(std::time::Duration::from_millis(35));
                        if !ignore_requested.load(Ordering::Relaxed) {
                            continue;
                        }
                        let Some(window) = app_handle.get_webview_window("main") else {
                            continue;
                        };
                        let Some(cursor) = cursor_position() else {
                            continue;
                        };
                        let inside =
                            match (window.outer_position(), window.scale_factor(), rects.read()) {
                                (Ok(origin), Ok(scale), Ok(rects)) => {
                                    let local_x = (cursor.x - origin.x) as f64 / scale;
                                    let local_y = (cursor.y - origin.y) as f64 / scale;
                                    rects.iter().any(|r| {
                                        local_x >= r.x
                                            && local_y >= r.y
                                            && local_x <= r.x + r.width
                                            && local_y <= r.y + r.height
                                    })
                                }
                                _ => false,
                            };
                        if inside != !is_ignored {
                            is_ignored = !inside;
                            let _ = window.set_ignore_cursor_events(is_ignored);
                        }
                    }
                });
            }
            let _ = window;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_cfg,
            set_client_id,
            spotify_login,
            spotify_token,
            spotify_logout,
            set_autostart,
            ignore_mouse,
            wall_active,
            set_hit_rects,
            start_audio_loopback,
            stop_audio_loopback,
            get_monitors,
            set_monitor
        ])
        .run(tauri::generate_context!())
        .expect("error al ejecutar Vinyl Desktop");
}
