# Vinyl Desktop

Reproductor visual interactivo para Windows conectado a Spotify. Ofrece una interfaz táctil centrada en un disco de vinilo manipulable con física de inercia, gestión completa de playlists, letras sincronizadas y un motor de fondos reactivos impulsados por las frecuencias bajas y altas del audio.

Está construido con **Rust** y **[Tauri v2](https://tauri.app)**. La interfaz está desarrollada en HTML, CSS y JavaScript, mientras que la lógica de sistema (autenticación con Spotify, captura de audio y anclaje a la capa del escritorio) corre de forma nativa en Rust.

## Características principales

- **Control táctil de vinilo:** Scrubbing gestual con física de inercia al girar el disco en pantalla para adelantar o retroceder la reproducción, además de controles en el carrusel y en la bandeja del sistema.
- **Fondos reactivos al audio:** Motor visual integrado que captura el audio del sistema mediante WASAPI loopback (sin compartir pantalla) y responde en tiempo real a los graves y agudos de la pista.
- **Letras sincronizadas:** Visualización en tiempo real de la letra de la canción en curso directamente en la interfaz.
- **Gestión completa de Spotify:** Inicio de sesión seguro mediante OAuth PKCE (sin almacenar tu contraseña), exploración de bibliotecas y playlists, búsqueda de pistas y edición de listas de reproducción.
- **Modo wallpaper:** Ancla la ventana detrás de las ventanas activas y de los iconos del escritorio.
- **Clic-through inteligente:** El ratón pasa a través de la ventana hacia el escritorio, excepto sobre los controles visibles.
- **Personalización e integración:** Distintos estilos de disco, tipografías configurables e inicio automático con Windows.

## Uso con el instalador (.exe)

1. Descarga el instalador `Vinyl Desktop_0.1.0_x64-setup.exe` desde la sección **Releases** de este repositorio.
2. Al abrir la aplicación por primera vez necesitas un **Client ID de Spotify** (ver la sección siguiente).

> **Nota sobre Windows SmartScreen:** Windows puede mostrar una advertencia de seguridad porque el instalador no está firmado digitalmente. Si confías en el origen del archivo, selecciona **Más información > Ejecutar de todos modos**.

## Configurar Spotify (Client ID)

La aplicación necesita su propia credencial de desarrollador de Spotify. Solo debes configurarla una vez:

1. Entra en el [Panel para desarrolladores de Spotify](https://developer.spotify.com/dashboard) e inicia sesión.
2. Crea una aplicación nueva (el nombre y la descripción no importan).
3. En **Redirect URI**, agrega exactamente:
   ```text
   [http://127.0.0.1:8888/callback](http://127.0.0.1:8888/callback)
   ```
4. Guarda la aplicación y copia el **Client ID** que aparece en su página de ajustes.
5. Abre **Vinyl Desktop**, pega el Client ID en el panel de configuración y pulsa **Iniciar sesión con Spotify**. Se abrirá tu navegador para autorizar la app.

**Notas:**
- Las funciones de control de reproducción de la API de Spotify normalmente requieren una cuenta **Spotify Premium**.
- Mientras la aplicación en Spotify esté en modo desarrollo (*Development Mode*), Spotify limita qué cuentas pueden iniciar sesión. Si tu cuenta no puede entrar, revisa la lista de usuarios permitidos de la app en el panel de Spotify.
- El puerto `8888` debe estar libre durante el inicio de sesión.

## Modo wallpaper

En el panel de configuración puedes activar el modo wallpaper. En ese modo:

- La ventana se coloca detrás de tus ventanas abiertas y de los iconos del escritorio, y se vuelve a anclar periódicamente.
- El visualizador utiliza la salida de audio predeterminada del sistema. Si la captura falla, se muestra un espectro simulado.
- Los clics siguen llegando con normalidad a los iconos del escritorio.



## Modo desarrollador y compilación

Si quieres ejecutar el proyecto en local, depurar la interfaz o compilar tu propia versión desde el código fuente, configura el entorno siguiendo estos pasos.

### Requisitos

- Windows 10 u 11.
- [Rust](https://www.rust-lang.org/tools/install) estable con el toolchain `x86_64-pc-windows-msvc`.
- Herramientas de compilación de C++ de Visual Studio (la carga de trabajo *Desarrollo para el escritorio con C++*).
- [Node.js](https://nodejs.org) LTS con `npm`.
- WebView2 Runtime (incluido en Windows 11 y en la mayoría de instalaciones de Windows 10 actualizadas).

### Ejecutar en modo desarrollador

1. Clona tu fork o el repositorio principal e instala las dependencias:
   ```powershell
   git clone [https://github.com/TU_USUARIO/vinyl-desktop.git](https://github.com/TU_USUARIO/vinyl-desktop.git)
   cd vinyl-desktop
   npm install
   ```

2. Inicia la aplicación en modo desarrollo:
   ```powershell
   npm run tauri dev
   ```
   En este modo dispones de recarga automática de los archivos de `renderer/` y acceso a las herramientas de desarrollo de WebView2 haciendo clic derecho sobre la interfaz > **Inspeccionar** (o con `Ctrl + Shift + I`).

3. Para verificar únicamente que el backend en Rust compila sin errores antes de levantar toda la ventana:
   ```powershell
   cargo check --manifest-path src-tauri\Cargo.toml
   ```

### Compilar el instalador (.exe)

Para generar el ejecutable y el instalador NSIS de producción:

```powershell
npm run tauri build
```

El instalador queda guardado en:

```text
src-tauri\target\release\bundle\nsis\
```



## Estructura del proyecto

```text
renderer/             Interfaz de usuario (HTML, CSS y JavaScript)
  index.html          Vista principal y elementos visuales
  desk.js             Puente window.desk entre la interfaz y el backend Rust
src-tauri/
  src/main.rs         Punto de entrada de la aplicación
  src/lib.rs          Lógica de Spotify, manejo de ventana, audio WASAPI y bandeja
  Cargo.toml          Dependencias y configuración de Rust
  tauri.conf.json     Configuración de Tauri y empaquetado NSIS
  capabilities/       Permisos y políticas de seguridad de la ventana
package.json          Scripts de desarrollo y compilación
```

## Datos locales

La configuración de la aplicación (como el Client ID, preferencias visuales, posición del disco y tokens de sesión de Spotify) se almacena exclusivamente de forma local en tu equipo dentro del directorio de datos de la aplicación en Windows (`%APPDATA%` / almacenamiento local de WebView2). No se envían datos ni telemetría a servidores externos.
