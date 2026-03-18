<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import type { PortInfo } from "./lib/types";

  let ports: PortInfo[] = $state([]);
  let filter = $state("");
  let killedPids = $state(new Set<number>());
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

  function hideWindow() {
    getCurrentWindow().hide();
  }

  $effect(() => {
    fetchPorts();
    intervalId = setInterval(fetchPorts, 3000) as unknown as number;
    return () => clearInterval(intervalId);
  });
</script>

<div class="app-container">
  <div class="titlebar">
    <div class="titlebar-title">
      <span class="icon">&#9889;</span>
      PortKill
    </div>
    <div class="titlebar-controls">
      <button class="titlebar-btn close" onclick={hideWindow} title="Cerrar">&#10005;</button>
    </div>
  </div>

  <div class="search-bar">
    <div class="search-wrapper">
      <span class="search-icon">&#128269;</span>
      <input
        class="search-input"
        type="text"
        placeholder="Filtrar puerto o proceso..."
        bind:value={filter}
      />
    </div>
  </div>

  <div class="port-header">
    <span>Puerto</span>
    <span>PID</span>
    <span>Proceso</span>
    <span></span>
  </div>

  <div class="port-list">
    {#if filteredPorts.length === 0}
      <div class="empty-state">
        <span class="icon">&#128268;</span>
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

  <div class="status-bar">
    <span>
      <span class="status-dot"></span>
      {ports.length} puerto{ports.length !== 1 ? "s" : ""} activo{ports.length !== 1 ? "s" : ""}
    </span>
    <span>Auto &#8635; 3s</span>
  </div>
</div>
