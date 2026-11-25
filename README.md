# Vinyl Desktop

Reproductor visual de escritorio para Spotify, hecho para Windows. Muestra un disco de vinilo giratorio y un carrusel de canciones sobre el escritorio, con un visualizador que reacciona al audio que suena en tu equipo.

Está construido con Rust y [Tauri v2](https://tauri.app). La interfaz es HTML y JavaScript, y la lógica de sistema (autenticación con Spotify, captura de audio, anclaje a la capa del escritorio) está en Rust.

## Características

- Inicio de sesión con Spotify (OAuth PKCE, sin guardar tu contraseña).
- Controles de reproducción desde el disco, el carrusel y el icono de la bandeja del sistema.
- Modo wallpaper: la ventana se ancla detrás de las ventanas normales y de los iconos del escritorio.
- Clic-through: el ratón pasa a través de la ventana, excepto sobre los controles visibles.
- Visualizador con el audio del sistema (WASAPI en loopback, sin compartir la pantalla).
- Inicio automático con Windows.
- Letras, distintos estilos de disco y tipografías.

## Uso con el instalador (.exe)

1. Descarga el instalador `Vinyl Desktop_0.1.0_x64-setup.exe` desde la sección **Releases** de este repositorio.
2. Ejecútalo y sigue el asistente.
3. Al abrir la app por primera vez necesitas un **Client ID de Spotify** (ver la sección siguiente).

Windows puede mostrar una advertencia de SmartScreen porque el instalador no está firmado digitalmente. Si confías en el origen del archivo, usa **Más información > Ejecutar de todos modos**.

## Configurar Spotify (Client ID)

La app necesita su propia credencial de Spotify. Solo tienes que crearla una vez:

1. Entra en el [Panel para desarrolladores de Spotify](https://developer.spotify.com/dashboard) e inicia sesión.
2. Crea una aplicación nueva. El nombre y la descripción no importan.
3. En **Redirect URI** agrega exactamente:
   ```
   http://127.0.0.1:8888/callback
   ```
4. Guarda la aplicación y copia el **Client ID** que aparece en su página de ajustes.
5. Abre Vinyl Desktop, pega el Client ID en el panel de configuración y pulsa **Iniciar sesión con Spotify**. Se abrirá tu navegador para autorizar la app.

Notas:
- Las funciones de control de reproducción de la API de Spotify normalmente requieren una cuenta **Premium**.
- Mientras la aplicación esté en modo desarrollo, Spotify puede limitar qué cuentas pueden iniciar sesión. Si tu cuenta no puede entrar, revisa la lista de usuarios de la app en el panel de Spotify.
- El puerto `8888` debe estar libre durante el inicio de sesión.

## Modo wallpaper

En el panel de configuración puedes activar el modo wallpaper. En ese modo:

- La ventana se coloca detrás de tus ventanas y de los iconos del escritorio, y se vuelve a anclar periódicamente.
- El visualizador usa el audio del sistema predeterminado. Si la captura falla, se muestra un espectro simulado.
- Los clics siguen llegando a los iconos del escritorio.

## Compilar desde el código fuente

### Requisitos

- Windows 10 u 11.
- [Rust](https://www.rust-lang.org/tools/install) estable, con el toolchain `x86_64-pc-windows-msvc`.
- Las herramientas de compilación de C++ de Visual Studio (la carga de trabajo "Desarrollo para el escritorio con C++").
- [Node.js](https://nodejs.org) LTS con npm.
- WebView2 Runtime (incluido en Windows 11 y en la mayoría de instalaciones de Windows 10 actualizadas).

### Pasos

```powershell
git clone https://github.com/TU_USUARIO/vinyl-desktop.git
cd vinyl-desktop
npm install
```

Para probarla en modo desarrollo:

```powershell
npm run tauri dev
```

Para generar el instalador:

```powershell
npm run tauri build
```

El instalador queda en:

```
src-tauri\target\release\bundle\nsis\
```

Para verificar solo el backend Rust sin compilar el instalador:

```powershell
cargo check --manifest-path src-tauri\Cargo.toml
```

## Estructura del proyecto

```
renderer/             Interfaz (HTML, CSS y JavaScript)
  index.html          Interfaz principal
  desk.js             Puente window.desk entre la interfaz y Rust
src-tauri/
  src/main.rs         Punto de entrada
  src/lib.rs          Lógica de Spotify, ventana, audio y bandeja
  Cargo.toml          Dependencias Rust
  tauri.conf.json     Configuración de Tauri y del instalador
  capabilities/       Permisos de la ventana
package.json          Scripts de npm
```

## Datos locales

La configuración (Client ID, tokens de sesión y preferencias) se guarda en el directorio de configuración de la aplicación de Windows, en `config.json`. Cerrar sesión borra los tokens de ese archivo.

## Licencia

Pendiente de definir.
