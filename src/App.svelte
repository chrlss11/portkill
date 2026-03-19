<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { check } from "@tauri-apps/plugin-updater";
  import { relaunch } from "@tauri-apps/plugin-process";
  import type { PortInfo } from "./lib/types";

  type ViewMode = "list" | "grouped";

  let ports: PortInfo[] = $state([]);
  let filter = $state("");
  let killedPids = $state(new Set<number>());
  let updateAvailable = $state(false);
  let updating = $state(false);
  let viewMode: ViewMode = $state("list");
  let intervalId: number;

  const filteredPorts = $derived(
    ports.filter((p) => {
      const q = filter.toLowerCase();
      return (
        p.port.toString().includes(q) ||
        p.process_name.toLowerCase().includes(q)
      );
    })
  );

  interface ProcessGroup {
    name: string;
    ports: PortInfo[];
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
    result.sort((a, b) => a.name.localeCompare(b.name));
    return result;
  });

  async function fetchPorts() {
    try {
      ports = await invoke<PortInfo[]>("list_ports");
    } catch (e) {
      console.error("Failed to fetch ports:", e);
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
    document.documentElement.setAttribute("data-theme", isDark ? "dark" : "light");
  }

  $effect(() => {
    detectTheme();
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const handler = () => detectTheme();
    mq.addEventListener("change", handler);

    fetchPorts();
    checkForUpdates();
    intervalId = setInterval(fetchPorts, 3000) as unknown as number;

    return () => {
      clearInterval(intervalId);
      mq.removeEventListener("change", handler);
    };
  });
</script>

<div class="app-container">
  <div class="titlebar">
    <div class="titlebar-title">
      <div class="titlebar-logo">K</div>
      PortKill
    </div>
    <div class="titlebar-controls">
      <button
        class="titlebar-btn view-toggle"
        onclick={() => (viewMode = viewMode === "list" ? "grouped" : "list")}
        title={viewMode === "list" ? "Agrupar por proceso" : "Vista lista"}
      >
        {viewMode === "list" ? "\u2630" : "\u2261"}
      </button>
      <button class="titlebar-btn close" onclick={hideWindow} title="Cerrar">&#10005;</button>
    </div>
  </div>

  {#if updateAvailable}
    <div class="update-bar">
      <span>Nueva version disponible</span>
      <button class="update-btn" onclick={installUpdate} disabled={updating}>
        {updating ? "Actualizando..." : "Actualizar"}
      </button>
    </div>
  {/if}

  <div class="search-bar">
    <div class="search-wrapper">
      <svg class="search-icon-svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <circle cx="11" cy="11" r="8"/>
        <path d="m21 21-4.35-4.35"/>
      </svg>
      <input
        class="search-input"
        type="text"
        placeholder="Filtrar puerto o proceso..."
        bind:value={filter}
      />
    </div>
  </div>

  {#if viewMode === "list"}
    <div class="port-header">
      <span>Puerto</span>
      <span>PID</span>
      <span>Proceso</span>
      <span></span>
    </div>

    <div class="port-list">
      {#if filteredPorts.length === 0}
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
        {#each filteredPorts as port (port.pid + "-" + port.port)}
          <div class="port-row" class:killing={killedPids.has(port.pid)}>
            <span class="port-num">{port.port}</span>
            <span class="pid">{port.pid}</span>
            <span class="process">{port.process_name}</span>
            <button
              class="kill-btn"
              class:killed={killedPids.has(port.pid)}
              onclick={() => killPort(port.pid)}
            >
              {killedPids.has(port.pid) ? "Done" : "Kill"}
            </button>
          </div>
        {/each}
      {/if}
    </div>
  {:else}
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
                <div class="group-port-row" class:killing={killedPids.has(port.pid)}>
                  <span class="port-num">:{port.port}</span>
                  <span class="pid">PID {port.pid}</span>
                  <button
                    class="kill-btn small"
                    class:killed={killedPids.has(port.pid)}
                    onclick={() => killPort(port.pid)}
                  >
                    {killedPids.has(port.pid) ? "Done" : "Kill"}
                  </button>
                </div>
              {/each}
            </div>
          </div>
        {/each}
      {/if}
    </div>
  {/if}

  <div class="status-bar">
    <div class="status-left">
      <span class="status-dot"></span>
      {ports.length} puerto{ports.length !== 1 ? "s" : ""} activo{ports.length !== 1 ? "s" : ""}
    </div>
    <span class="status-refresh">Auto &#8635; 3s</span>
  </div>
</div>
