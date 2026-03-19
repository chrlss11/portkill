<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { check } from "@tauri-apps/plugin-updater";
  import { relaunch } from "@tauri-apps/plugin-process";
  import { open } from "@tauri-apps/plugin-shell";
  import type { PortInfo } from "./lib/types";

  let ports: PortInfo[] = $state([]);
  let previousPorts: PortInfo[] = $state([]);
  let filter = $state("");
  let killedPids = $state(new Set<number>());
  let updateAvailable = $state(false);
  let updating = $state(false);
  let showFavorites = $state(true);
  let expandedPort: string | null = $state(null);
  let copiedFeedback = $state(false);
  let toasts: { id: number; message: string; type: "new-port" | "closed-port" }[] = $state([]);
  let toastId = 0;
  let searchInput: HTMLInputElement | undefined = $state(undefined);
  let intervalId: number;

  // Favorites from localStorage
  let favorites: number[] = $state(
    JSON.parse(localStorage.getItem("portkill_favorites") || "[]")
  );

  $effect(() => {
    localStorage.setItem("portkill_favorites", JSON.stringify(favorites));
  });

  const filteredPorts = $derived(
    ports.filter((p) => {
      const q = filter.toLowerCase();
      return (
        p.port.toString().includes(q) ||
        p.process_name.toLowerCase().includes(q) ||
        p.project_name.toLowerCase().includes(q)
      );
    })
  );

  interface ProcessGroup {
    name: string;
    ports: PortInfo[];
  }

  // Priority keywords: NestJS and common dev tools show first
  const priorityKeywords = ["nest", "node", "bun", "vite", "next", "nuxt", "angular", "svelte"];

  function getProcessPriority(name: string): number {
    const lower = name.toLowerCase();
    for (let i = 0; i < priorityKeywords.length; i++) {
      if (lower.includes(priorityKeywords[i])) return i;
    }
    return 99;
  }

  const groupedPorts = $derived(() => {
    const groups = new Map<string, PortInfo[]>();
    for (const p of filteredPorts) {
      const key = p.process_name;
      if (!groups.has(key)) groups.set(key, []);
      groups.get(key)!.push(p);
    }
    const result: ProcessGroup[] = [];
    for (const [name, ports] of groups) {
      result.push({ name, ports });
    }
    // Sort: priority processes first (nest, node, bun...), then alphabetical
    result.sort((a, b) => {
      const pa = getProcessPriority(a.name);
      const pb = getProcessPriority(b.name);
      if (pa !== pb) return pa - pb;
      return a.name.localeCompare(b.name);
    });
    return result;
  });

  // Favorite ports: active ones + inactive placeholders
  const favoritePorts = $derived(
    favorites.map((favPort) => {
      const active = ports.find((p) => p.port === favPort);
      return {
        port: favPort,
        active: !!active,
        info: active || null,
      };
    })
  );

  function isFavorite(port: number): boolean {
    return favorites.includes(port);
  }

  function toggleFavorite(port: number) {
    if (favorites.includes(port)) {
      favorites = favorites.filter((f) => f !== port);
    } else {
      favorites = [...favorites, port];
    }
  }

  async function fetchPorts() {
    try {
      const newPorts = await invoke<PortInfo[]>("list_ports");

      // Detect changes for notifications
      if (previousPorts.length > 0) {
        const prevSet = new Set(previousPorts.map((p) => p.port));
        const newSet = new Set(newPorts.map((p) => p.port));

        for (const p of newPorts) {
          if (!prevSet.has(p.port)) {
            addToast(`${p.process_name} abrio el puerto :${p.port}`, "new-port");
            showNotification(`${p.process_name} abrio el puerto :${p.port}`);
          }
        }
        for (const p of previousPorts) {
          if (!newSet.has(p.port)) {
            addToast(`Puerto :${p.port} cerrado (${p.process_name})`, "closed-port");
          }
        }
      }

      previousPorts = [...newPorts];
      ports = newPorts;
    } catch (e) {
      console.error("Failed to fetch ports:", e);
    }
  }

  function addToast(message: string, type: "new-port" | "closed-port") {
    const id = ++toastId;
    toasts = [...toasts, { id, message, type }];
    setTimeout(() => {
      toasts = toasts.filter((t) => t.id !== id);
    }, 3000);
  }

  function showNotification(body: string) {
    if (Notification.permission === "granted") {
      new Notification("PortKill", { body });
    } else if (Notification.permission !== "denied") {
      Notification.requestPermission().then((perm) => {
        if (perm === "granted") {
          new Notification("PortKill", { body });
        }
      });
    }
  }

  async function killPort(pid: number) {
    try {
      await invoke("kill_port", { pid });
      killedPids.add(pid);
      killedPids = new Set(killedPids);
      setTimeout(() => {
        killedPids.delete(pid);
        killedPids = new Set(killedPids);
        fetchPorts();
      }, 800);
    } catch (e) {
      console.error("Failed to kill process:", e);
    }
  }

  async function killGroup(group: ProcessGroup) {
    const pids = group.ports.map((p) => p.pid);
    const uniquePids = [...new Set(pids)];
    for (const pid of uniquePids) {
      await killPort(pid);
    }
  }

  function hideWindow() {
    getCurrentWindow().hide();
  }

  async function openInBrowser(port: number) {
    try {
      await open(`http://localhost:${port}`);
    } catch (e) {
      console.error("Failed to open browser:", e);
    }
  }

  async function copyPort(port: number) {
    try {
      await navigator.clipboard.writeText(`localhost:${port}`);
      copiedFeedback = true;
      setTimeout(() => {
        copiedFeedback = false;
      }, 1200);
    } catch (e) {
      console.error("Failed to copy:", e);
    }
  }

  async function openFolder(workingDir: string) {
    if (!workingDir || workingDir === "-") return;
    try {
      await open(workingDir);
    } catch (e) {
      console.error("Failed to open folder:", e);
    }
  }

  function toggleExpand(key: string) {
    expandedPort = expandedPort === key ? null : key;
  }

  async function checkForUpdates() {
    try {
      const update = await check();
      if (update) {
        updateAvailable = true;
      }
    } catch (e) {
      console.error("Update check failed:", e);
    }
  }

  async function installUpdate() {
    try {
      updating = true;
      const update = await check();
      if (update) {
        await update.downloadAndInstall();
        await relaunch();
      }
    } catch (e) {
      console.error("Update failed:", e);
      updating = false;
    }
  }

  // Detect system theme
  let isDark = $state(true);

  function detectTheme() {
    isDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
    document.documentElement.setAttribute(
      "data-theme",
      isDark ? "dark" : "light"
    );
  }

  // Keyboard shortcuts
  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      hideWindow();
    } else if (e.key === "/" && !(e.target instanceof HTMLInputElement)) {
      e.preventDefault();
      searchInput?.focus();
    } else if ((e.ctrlKey || e.metaKey) && e.key === "k") {
      e.preventDefault();
      searchInput?.focus();
    }
  }

  // Request notification permission on startup
  function requestNotificationPermission() {
    if (Notification.permission === "default") {
      Notification.requestPermission();
    }
  }

  $effect(() => {
    detectTheme();
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const handler = () => detectTheme();
    mq.addEventListener("change", handler);

    document.addEventListener("keydown", handleKeydown);

    requestNotificationPermission();
    fetchPorts();
    checkForUpdates();
    intervalId = setInterval(fetchPorts, 3000) as unknown as number;

    return () => {
      clearInterval(intervalId);
      mq.removeEventListener("change", handler);
      document.removeEventListener("keydown", handleKeydown);
    };
  });
</script>

<div class="app-container">
  <!-- Titlebar -->
  <div class="titlebar">
    <div class="titlebar-title">
      <div class="titlebar-logo">
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round"><path d="M13 2L3 14h9l-1 8 10-12h-9l1-8z"/></svg>
      </div>
      PortKill
    </div>
    <div class="titlebar-controls">
      <button class="titlebar-btn close" onclick={hideWindow} title="Cerrar">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M18 6L6 18M6 6l12 12"/></svg>
      </button>
    </div>
  </div>

  <!-- Update bar -->
  {#if updateAvailable}
    <div class="update-bar">
      <span>Nueva version disponible</span>
      <button class="update-btn" onclick={installUpdate} disabled={updating}>
        {updating ? "Actualizando..." : "Actualizar"}
      </button>
    </div>
  {/if}

  <!-- Search + toggles -->
  <div class="search-bar">
    <div class="search-wrapper">
      <svg class="search-icon-svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <circle cx="11" cy="11" r="8"/>
        <path d="m21 21-4.35-4.35"/>
      </svg>
      <input
        class="search-input"
        type="text"
        placeholder="Filtrar...  (/)"
        bind:value={filter}
        bind:this={searchInput}
      />
    </div>
    <div class="search-actions">
      <button
        class="search-action-btn"
        class:active={showFavorites}
        onclick={() => (showFavorites = !showFavorites)}
        title="Mostrar favoritos"
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill={showFavorites ? "currentColor" : "none"} stroke="currentColor" stroke-width="2"><polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/></svg>
      </button>
    </div>
  </div>

  <!-- Favorites section -->
  {#if showFavorites && favorites.length > 0}
    <div class="section-header">
      <span>Favoritos</span>
      <span class="count">{favorites.length}</span>
    </div>
    <div class="favorites-section">
      {#each favoritePorts as fav (fav.port)}
        <div class="fav-row">
          <span class="fav-status-dot" class:active={fav.active} class:inactive={!fav.active}></span>
          <span class="fav-port" class:active={fav.active} class:inactive={!fav.active}>:{fav.port}</span>
          {#if fav.active && fav.info}
            <span class="fav-info">{fav.info.process_name} {fav.info.project_name !== "-" ? `- ${fav.info.project_name}` : ""}</span>
          {:else}
            <span class="fav-info inactive">Inactivo</span>
          {/if}
          <div class="fav-actions">
            {#if fav.active}
              <button class="action-btn" onclick={() => openInBrowser(fav.port)} title="Abrir en navegador">
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><circle cx="12" cy="12" r="10"/><path d="M2 12h20M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/></svg>
              </button>
              <button class="action-btn" onclick={() => copyPort(fav.port)} title="Copiar">
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>
              </button>
            {/if}
            <button class="action-btn star starred" onclick={() => toggleFavorite(fav.port)} title="Quitar de favoritos">
              <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor" stroke="currentColor" stroke-width="2"><polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/></svg>
            </button>
          </div>
        </div>
      {/each}
    </div>
    <div class="separator"></div>
  {/if}

  <!-- Port list (grouped by process) -->
  <div class="section-header">
    <span>Por proceso</span>
    <span class="count">{groupedPorts().length}</span>
  </div>
  <div class="port-list">
    {#if groupedPorts().length === 0}
      <div class="empty-state">
        <div class="empty-icon">
          <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <path d="M12 22c5.523 0 10-4.477 10-10S17.523 2 12 2 2 6.477 2 12s4.477 10 10 10z"/>
            <path d="M12 8v4m0 4h.01"/>
          </svg>
        </div>
        <p>{filter ? "Sin resultados" : "No hay puertos activos"}</p>
      </div>
    {:else}
      {#each groupedPorts() as group (group.name)}
        <div class="group-card">
          <div class="group-header">
            <div class="group-info">
              <span class="group-name">{group.name}</span>
              <span class="group-count">{group.ports.length} puerto{group.ports.length !== 1 ? "s" : ""}</span>
            </div>
            <button class="kill-all-btn" onclick={() => killGroup(group)}>
              Kill All
            </button>
          </div>
          <div class="group-ports">
            {#each group.ports as port (port.pid + "-" + port.port)}
              {@const rowKey = `${port.pid}-${port.port}`}
              <!-- svelte-ignore a11y_click_events_have_key_events -->
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div class="group-port-row" class:killing={killedPids.has(port.pid)} onclick={() => toggleExpand(rowKey)}>
                <div class="group-port-main">
                  <span class="port-num">:{port.port}</span>
                  {#if port.project_name && port.project_name !== "-"}
                    <span class="project-badge has-project">{port.project_name}</span>
                  {/if}
                  <span class="pid">PID {port.pid}</span>
                </div>
                <div class="group-port-actions">
                  <button class="action-btn star" class:starred={isFavorite(port.port)} onclick={(e) => { e.stopPropagation(); toggleFavorite(port.port); }} title={isFavorite(port.port) ? "Quitar favorito" : "Agregar favorito"}>
                    <svg width="11" height="11" viewBox="0 0 24 24" fill={isFavorite(port.port) ? "currentColor" : "none"} stroke="currentColor" stroke-width="2"><polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/></svg>
                  </button>
                  <button class="action-btn" onclick={(e) => { e.stopPropagation(); openInBrowser(port.port); }} title="Abrir en navegador">
                    <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><circle cx="12" cy="12" r="10"/><path d="M2 12h20"/></svg>
                  </button>
                  <button class="action-btn" onclick={(e) => { e.stopPropagation(); copyPort(port.port); }} title="Copiar">
                    <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>
                  </button>
                  <button class="action-btn kill" onclick={(e) => { e.stopPropagation(); killPort(port.pid); }} title="Matar">
                    <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M18 6L6 18M6 6l12 12"/></svg>
                  </button>
                </div>

                <!-- Expanded detail -->
                {#if expandedPort === rowKey}
                  <div class="port-detail" onclick={(e) => e.stopPropagation()}>
                    {#if port.working_dir && port.working_dir !== "-"}
                      <div class="detail-row">
                        <span class="detail-label">Dir</span>
                        <span class="detail-value">{port.working_dir}</span>
                      </div>
                    {/if}
                    <div class="detail-actions">
                      <button class="detail-action-btn" onclick={() => openInBrowser(port.port)}>Abrir en navegador</button>
                      <button class="detail-action-btn" onclick={() => copyPort(port.port)}>Copiar puerto</button>
                      {#if port.working_dir && port.working_dir !== "-"}
                        <button class="detail-action-btn" onclick={() => openFolder(port.working_dir)}>Abrir carpeta</button>
                      {/if}
                    </div>
                  </div>
                {/if}
              </div>
            {/each}
          </div>
        </div>
      {/each}
    {/if}
  </div>

  <!-- Status bar -->
  <div class="status-bar">
    <div class="status-left">
      <span class="status-dot"></span>
      {ports.length} activo{ports.length !== 1 ? "s" : ""}
      &middot; &#8635; 3s
    </div>
    <div class="status-right">
      <span class="status-version">v0.6.0</span>
    </div>
  </div>
</div>

<!-- Toasts -->
{#if toasts.length > 0}
  <div class="toast-container">
    {#each toasts as toast (toast.id)}
      <div class="toast {toast.type}">{toast.message}</div>
    {/each}
  </div>
{/if}

<!-- Copied feedback -->
{#if copiedFeedback}
  <div class="copied-feedback">Copiado</div>
{/if}
