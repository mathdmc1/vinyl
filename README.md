# Vinyl Desktop

Aplicación de escritorio construida con Rust y Tauri v2. La interfaz original permanece en `renderer/index.html`; `renderer/desk.js` crea el puente `window.desk` antes de cargarla. Rust proporciona autenticación PKCE de Spotify, renovación de tokens, inicio automático, clic-through y captura WASAPI del audio del sistema.

En Windows, la ventana se ordena detrás de las ventanas normales y de Explorer al iniciar, y se vuelve a anclar periódicamente. El modo wallpaper captura el audio con WASAPI desde Rust, sin solicitar captura ni compartir la pantalla.

## Requisitos

- Rust estable y las herramientas de compilación de escritorio de Tauri para Windows.
- Node.js y npm.
- Un Client ID de Spotify con `http://127.0.0.1:8888/callback` agregado como Redirect URI.

## Desarrollo

```powershell
npm install
npm run tauri dev
```

## Compilar instalador

```powershell
npm run tauri build
```

