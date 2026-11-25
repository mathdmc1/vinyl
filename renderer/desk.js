// Bridge that gives the renderer the desktop API it used under Electron.
// It is loaded before the app script so `window.desk` is available at startup.
(() => {
  const tauri = window.__TAURI__;
  if (!tauri?.core?.invoke || !tauri?.event?.listen) {
    console.info('[desk.js] Tauri no está disponible; se inicia en modo demo.');
    return;
  }

  const { invoke } = tauri.core;
  const { listen } = tauri.event;
  const callbacks = new Set();

  window.desk = {
    token: () => invoke('spotify_token'),
    getCfg: () => invoke('get_cfg'),
    setClientId: (clientId) => invoke('set_client_id', { clientId: String(clientId || '').trim() }),
    login: () => invoke('spotify_login'),
    logout: () => invoke('spotify_logout'),
    setAutostart: (enabled) => invoke('set_autostart', { enabled: !!enabled }),
    ignoreMouse: (ignore) => invoke('ignore_mouse', { ignore: !!ignore }),
    wallActive: (on) => invoke('wall_active', { on: !!on }),
    startAudio: () => invoke('start_audio_loopback'),
    stopAudio: () => invoke('stop_audio_loopback'),
    getMonitors: () => invoke('get_monitors'),
    setMonitor: (index) => invoke('set_monitor', { index }),
    onCmd: (callback) => callbacks.add(callback),
  };

  listen('vinyl://command', ({ payload }) => callbacks.forEach((callback) => callback(payload)));
  listen('vinyl://audio-spectrum', ({ payload }) => {
    window.dispatchEvent(new CustomEvent('vinyl-audio-spectrum', { detail: payload }));
  });
  listen('vinyl://audio-status', ({ payload }) => {
    window.dispatchEvent(new CustomEvent('vinyl-audio-status', { detail: payload }));
  });

  // Keep the native click-through window interactive only over visible controls.
  let lastRects = '';
  const reportHitRects = () => {
    if (!document.documentElement) return;
    const rects = [...document.querySelectorAll('.hit')]
      .map((element) => {
        const rect = element.getBoundingClientRect();
        return { x: rect.x, y: rect.y, width: rect.width, height: rect.height };
      })
      .filter((rect) => rect.width > 0 && rect.height > 0);
    const json = JSON.stringify(rects);
    if (json !== lastRects) {
      lastRects = json;
      invoke('set_hit_rects', { rects });
    }
  };
  new MutationObserver(reportHitRects).observe(document, {
    subtree: true,
    childList: true,
    attributes: true,
  });
  window.addEventListener('resize', reportHitRects);
  window.setInterval(reportHitRects, 180);
})();
