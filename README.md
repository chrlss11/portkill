# PortKill

App de system tray para Windows que muestra los puertos abiertos en localhost y te permite matar los procesos al instante.

![Windows](https://img.shields.io/badge/Windows-10%2F11-0078D6?logo=windows)
![License](https://img.shields.io/badge/license-MIT-green)
![Tauri](https://img.shields.io/badge/Tauri-v2-FFC131?logo=tauri)

## Por qué

Durante desarrollo es común tener puertos ocupados por procesos zombie o servidores que olvidaste cerrar. En vez de abrir PowerShell, correr `netstat`, buscar el PID y hacer `taskkill`, PortKill lo resuelve en un click desde la bandeja del sistema.

## Features

- **System tray**: vive al lado del reloj/wifi, sin ocupar espacio en la taskbar
- **Lista de puertos**: muestra todos los puertos TCP en estado LISTENING
- **Kill instantáneo**: mata cualquier proceso con un click
- **Auto-refresh**: actualiza la lista cada 3 segundos
- **Filtro**: busca por número de puerto o nombre de proceso
- **Dark theme**: Catppuccin Mocha, consistente con Windows 11 dark mode
- **Liviano**: ~5MB de instalador

## Preview

```
┌─────────────────────────────────────┐
│  ⚡ PortKill                     ✕  │
├─────────────────────────────────────┤
│  🔍 Filtrar puerto o proceso...    │
├─────────────────────────────────────┤
│  PUERTO  PID     PROCESO    ACCION │
│  3000    12345   node.exe   [Kill] │
│  3800    23456   bun.exe    [Kill] │
│  5432    34567   postgres   [Kill] │
│  8080    45678   java.exe   [Kill] │
├─────────────────────────────────────┤
│  4 puertos activos · Auto ↻ 3s    │
└─────────────────────────────────────┘
```

## Instalacion

Descarga el `.msi` o `.exe` de la ultima [Release](https://github.com/chrlss11/portkill/releases).

### Desde el instalador

1. Descarga `PortKill_0.1.0_x64-setup.exe` o `PortKill_0.1.0_x64_en-US.msi`
2. Ejecuta el instalador
3. PortKill aparece en la bandeja del sistema

## Uso

| Accion | Como |
|--------|------|
| Abrir lista de puertos | Click izquierdo en el icono del tray |
| Cerrar popup | Click izquierdo de nuevo o boton X |
| Matar un proceso | Click en el boton **Kill** rojo |
| Filtrar | Escribe en el campo de busqueda |
| Salir de la app | Click derecho en el tray → "Salir" |

## Build desde source

### Requisitos

- [Bun](https://bun.sh) (runtime + package manager)
- [Rust](https://rustup.rs) (stable)
- Windows 10/11
- [WebView2](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) (viene incluido en Windows 11)

### Pasos

```bash
# Clonar el repo
git clone https://github.com/chrlss11/portkill.git
cd portkill

# Instalar dependencias del frontend
bun install

# Desarrollo (hot reload)
bun run tauri dev

# Build para produccion
bun run tauri build
```

El instalador queda en `src-tauri/target/release/bundle/`.

## CI/CD

El proyecto usa GitHub Actions para compilar y crear releases automaticamente.

### Como funciona

1. Pusheas un tag con formato `v*` (ej: `v0.1.0`)
2. GitHub Actions ejecuta el workflow en un runner `windows-latest`
3. Compila el frontend (Svelte) y el backend (Rust/Tauri)
4. Crea un GitHub Release con los instaladores `.exe` y `.msi` adjuntos

### Crear un release

```bash
git tag v0.1.0
git push origin v0.1.0
```

El workflow se activa automaticamente. Puedes ver el progreso en la tab **Actions** del repo.

### Workflow

El archivo `.github/workflows/release.yml` configura:

- **Trigger**: push de tags `v*`
- **Runner**: `windows-latest`
- **Steps**: checkout → Node 22 → Bun → Rust stable → cache de Rust → `bun install` → `tauri-action` (build + release)
- **Permisos**: `contents: write` para crear el release

> No necesitas configurar secrets adicionales. El `GITHUB_TOKEN` que provee GitHub Actions ya tiene los permisos necesarios.

## Stack

| Capa | Tecnologia |
|------|-----------|
| Frontend | Svelte 5 + TypeScript |
| Backend | Rust |
| Framework | Tauri v2 |
| Build tool | Vite |
| Package manager | Bun |
| CI/CD | GitHub Actions |
| Estilo | Catppuccin Mocha |

## Estructura del proyecto

```
portkill/
├── src/                        # Frontend (Svelte)
│   ├── App.svelte              # UI principal: lista + kill buttons
│   ├── main.ts                 # Entry point de Svelte
│   ├── app.css                 # Dark theme
│   └── lib/types.ts            # Interfaces TypeScript
├── src-tauri/                  # Backend (Rust)
│   ├── src/lib.rs              # Comandos: list_ports, kill_port
│   ├── src/main.rs             # Entry point + tray setup
│   ├── tauri.conf.json         # Config de Tauri
│   ├── capabilities/           # Permisos Tauri v2
│   └── icons/                  # Iconos de la app
├── .github/workflows/          # CI/CD
├── index.html                  # Entry HTML para Vite
├── package.json
├── vite.config.ts
└── svelte.config.js
```

## Como funciona internamente

1. **`list_ports`** (Rust): ejecuta `netstat -ano -p tcp`, parsea las lineas con estado `LISTENING`, luego resuelve nombres de proceso con `tasklist /FO CSV /NH`. Retorna la lista al frontend via IPC de Tauri.

2. **`kill_port`** (Rust): ejecuta `taskkill /PID {pid} /F` para matar el proceso. Protege contra PIDs del sistema (0, 4).

3. **System tray** (Rust/Tauri): click izquierdo toggle show/hide del popup. Click derecho muestra menu contextual.

4. **Frontend** (Svelte): llama `list_ports` cada 3 segundos, filtra reactivamente, y muestra animacion de fade-out al matar un proceso.

## License

[MIT](LICENSE)
