<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { getCurrentWebview } from '@tauri-apps/api/webview';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { ask, open as openDialog } from '@tauri-apps/plugin-dialog';
  import { api } from '$lib/api';
  import { CardDrag } from '$lib/dnd.svelte';
  import { collectTags, filterTasks, parseTerms } from '$lib/search';
  import type {
    AppConfig,
    CalendarViewState,
    ChecklistItem,
    FolderEntry,
    FolderListing,
    ListViewState,
    ProgressMode,
    Status,
    Task,
    ViewName,
  } from '$lib/types';
  import BoardView from '$lib/components/BoardView.svelte';
  import DetailPanel from '$lib/components/DetailPanel.svelte';
  import ListView from '$lib/components/ListView.svelte';
  import CalendarView from '$lib/components/CalendarView.svelte';
  import Dashboard from '$lib/components/Dashboard.svelte';
  import CreateModal from '$lib/components/CreateModal.svelte';
  import Onboarding from '$lib/components/Onboarding.svelte';
  import SettingsModal from '$lib/components/SettingsModal.svelte';
  import TopBar from '$lib/components/TopBar.svelte';

  let config = $state<AppConfig | null>(null);
  let tasks = $state<Task[]>([]);
  let view = $state<ViewName>('board');

  // ビューは切り替えのたびに作り直されるため、表示条件はここで保持する
  const listState = $state<ListViewState>({
    statusFilter: 'all',
    sortKey: 'activity',
    sortAsc: false,
  });
  const calState = $state<CalendarViewState>({
    year: new Date().getFullYear(),
    month: new Date().getMonth(),
    showStart: true,
    showDue: true,
    showDone: true,
  });
  let loaded = $state(false);
  let query = $state('');
  let selectedPath = $state<string | null>(null);
  const emptyListing: FolderListing = { entries: [], deeper_omitted: false, count_capped: false };
  let listing = $state<FolderListing>(emptyListing);
  let showCreate = $state(false);
  let showSettings = $state(false);
  let autostart = $state(false);
  let toasts = $state<{ id: number; msg: string; kind: 'info' | 'error' }[]>([]);
  let topBar: ReturnType<typeof TopBar> | undefined = $state();

  const drag = new CardDrag(dropOn);

  // オンボーディング用
  let obWork = $state('');
  let obArchive = $state('');

  let toastSeq = 0;
  function toast(msg: string, kind: 'info' | 'error' = 'info') {
    const id = ++toastSeq;
    toasts.push({ id, msg, kind });
    setTimeout(() => {
      toasts = toasts.filter((t) => t.id !== id);
    }, 4000);
  }

  const configured = $derived(!!config?.work_root && !!config?.archive_root);
  const brandName = $derived(config?.display_name ?? 'Fubako');
  const selected = $derived(tasks.find((t) => t.path === selectedPath) ?? null);

  const allTags = $derived(collectTags(tasks));
  const terms = $derived(parseTerms(query));
  const filtered = $derived(filterTasks(tasks, terms));

  async function refresh() {
    try {
      tasks = await api.listTasks();
      if (selectedPath && !tasks.some((t) => t.path === selectedPath)) {
        selectedPath = null;
      }
    } catch (e) {
      toast(String(e), 'error');
    }
  }

  async function loadAll() {
    try {
      config = await api.getConfig();
      autostart = await api.getAutostart().catch(() => false);
      await refresh();
    } catch (e) {
      toast(String(e), 'error');
    } finally {
      loaded = true;
    }
  }

  // 選択タスクの中身プレビューを読み込む
  $effect(() => {
    const path = selected?.path;
    if (!path) {
      listing = emptyListing;
      return;
    }
    api
      .listFolder(path)
      .then((result) => {
        if (selected?.path === path) listing = result;
      })
      .catch(() => (listing = emptyListing));
  });
  function selectCard(t: Task) {
    if (drag.suppressClick) return;
    selectedPath = selectedPath === t.path ? null : t.path;
  }

  async function dropOn(task: Task, status: Status) {
    if (task.status === status) return;
    try {
      if (status === 'done') {
        const newPath = await api.completeTask(task.path);
        if (selectedPath === task.path) selectedPath = newPath;
        toast(`「${task.name}」をアーカイブへ移動しました`);
      } else if (task.archived) {
        const newPath = await api.reopenTask(task.path);
        await api.setTaskMeta(
          newPath,
          status,
          task.tags,
          task.due,
          task.memo,
          task.checklist,
          task.manual_progress,
          task.progress_mode
        );
        if (selectedPath === task.path) selectedPath = newPath;
        toast(`「${task.name}」を作業ディレクトリへ戻しました`);
      } else {
        await api.setTaskMeta(
          task.path,
          status,
          task.tags,
          task.due,
          task.memo,
          task.checklist,
          task.manual_progress,
          task.progress_mode
        );
      }
      await refresh();
    } catch (e) {
      toast(String(e), 'error');
    }
  }

  async function handleCreate(name: string, useTemplate: boolean) {
    try {
      const task = await api.createTask(name, useTemplate);
      showCreate = false;
      await refresh();
      selectedPath = task.path;
      toast(`${task.folder_name} を作成しました`);
    } catch (e) {
      toast(String(e), 'error');
    }
  }

  async function handleComplete(task: Task) {
    try {
      const newPath = await api.completeTask(task.path);
      selectedPath = newPath;
      await refresh();
      toast(`「${task.name}」をアーカイブへ移動しました`);
    } catch (e) {
      toast(String(e), 'error');
    }
  }

  async function handleReopen(task: Task) {
    try {
      const newPath = await api.reopenTask(task.path);
      selectedPath = newPath;
      await refresh();
      toast(`「${task.name}」を作業ディレクトリへ戻しました`);
    } catch (e) {
      toast(String(e), 'error');
    }
  }

  async function handleSaveMeta(
    task: Task,
    status: Status,
    tags: string[],
    due: string | null,
    memo: string,
    checklist: ChecklistItem[],
    manualProgress: number | null,
    progressMode: ProgressMode
  ) {
    try {
      await api.setTaskMeta(
        task.path,
        status,
        tags,
        due,
        memo,
        checklist,
        manualProgress,
        progressMode
      );
      await refresh();
    } catch (e) {
      toast(String(e), 'error');
    }
  }

  async function handleCopyPath(task: Task) {
    try {
      await navigator.clipboard.writeText(task.path);
      toast('パスをコピーしました');
    } catch {
      toast('コピーに失敗しました', 'error');
    }
  }

  function handleOpen(task: Task) {
    api.openInExplorer(task.path).catch((e) => toast(String(e), 'error'));
  }

  function handleOpenEntry(task: Task, entry: FolderEntry) {
    api.openEntry(`${task.path}\\${entry.rel}`).catch((e) => toast(String(e), 'error'));
  }

  async function handleSaveSettings(next: AppConfig, auto: boolean) {
    try {
      await api.setConfig(next);
      config = next;
      await api.setAutostart(auto).catch(() => {});
      autostart = auto;
      showSettings = false;
      await refresh();
      toast('設定を保存しました');
    } catch (e) {
      toast(String(e), 'error');
    }
  }

  async function pickOnboarding(target: 'work' | 'archive') {
    const dir = await openDialog({ directory: true, title: 'フォルダを選択' });
    if (typeof dir === 'string') {
      if (target === 'work') obWork = dir;
      else obArchive = dir;
    }
  }

  async function finishOnboarding() {
    if (!config || !obWork || !obArchive) return;
    await handleSaveSettings({ ...config, work_root: obWork, archive_root: obArchive }, autostart);
  }

  // 付箋のように脇へ置いておくためのモード。ウィンドウは細く縮められる
  // 付箋のように脇へ置いておくためのモード。ウィンドウは細く縮められる
  let pinned = $state(false);
  async function togglePinned() {
    const next = !pinned;
    try {
      await getCurrentWindow().setAlwaysOnTop(next);
      pinned = next;
    } catch (e) {
      toast(String(e), 'error');
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      if (showCreate) showCreate = false;
      else if (showSettings) showSettings = false;
      // 絞り込み中はまず解除する（フィルタバーの案内と一致させる）
      else if (query) topBar?.clear();
      else if (selectedPath) selectedPath = null;
      return;
    }
    const inField =
      e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement;
    if (e.ctrlKey && e.key.toLowerCase() === 'n') {
      e.preventDefault();
      showCreate = true;
    } else if (e.ctrlKey && e.key.toLowerCase() === 'f') {
      e.preventDefault();
      topBar?.focusSearch(true);
    } else if (e.key === '/' && !inField) {
      e.preventDefault();
      topBar?.focusSearch();
    }
  }

  async function handleRename(task: Task, name: string) {
    try {
      const newPath = await api.renameTask(task.path, name);
      selectedPath = newPath;
      await refresh();
    } catch (e) {
      toast(String(e), 'error');
    }
  }

  async function handleDeepArchiveOne(task: Task) {
    const ok = await ask(
      `「${task.name}」をディープアーカイブへ移動します。\n移動後はアプリの表示・検索対象から外れます（フォルダを戻せば再登録されます）。`,
      { title: 'ディープアーカイブ', kind: 'warning', okLabel: '移動する', cancelLabel: 'キャンセル' }
    );
    if (!ok) return;
    try {
      await api.deepArchiveTask(task.path);
      await refresh();
      toast(`「${task.name}」をディープアーカイブへ移動しました`);
    } catch (e) {
      toast(String(e), 'error');
    }
  }

  async function handleDeepSweep(candidates: Task[]) {
    if (candidates.length === 0) return;
    const ok = await ask(
      `${candidates.length} 件の完了タスクをディープアーカイブへ移動します。\n移動後はアプリの表示・検索対象から外れます（フォルダを戻せば再登録されます）。`,
      { title: 'アーカイブ整理', kind: 'warning', okLabel: '移動する', cancelLabel: 'キャンセル' }
    );
    if (!ok) return;
    let moved = 0;
    for (const t of candidates) {
      try {
        await api.deepArchiveTask(t.path);
        moved++;
      } catch (e) {
        toast(String(e), 'error');
      }
    }
    await refresh();
    if (moved > 0) toast(`${moved} 件をディープアーカイブへ移動しました`);
  }

  async function importDropped(paths: string[]) {
    if (!configured || paths.length === 0) return;
    const shown = paths.slice(0, 4).join('\n');
    const ok = await ask(
      `${paths.length} 件のフォルダを作業ディレクトリへ移動して管理対象にします。\n\n${shown}${paths.length > 4 ? '\n…' : ''}`,
      { title: 'フォルダの取り込み', kind: 'info', okLabel: '移動して取り込む', cancelLabel: 'キャンセル' }
    );
    if (!ok) return;
    let imported = 0;
    for (const p of paths) {
      try {
        await api.importTask(p);
        imported++;
      } catch (e) {
        toast(String(e), 'error');
      }
    }
    await refresh();
    if (imported > 0) toast(`${imported} 件を取り込みました`);
  }

  onMount(() => {
    loadAll();
    const unlisteners = [
      listen('tasks-changed', refresh),
      listen('focus-search', () => topBar?.focusSearch(true)),
      listen('quick-create', () => {
        if (configured) showCreate = true;
      }),
      getCurrentWebview().onDragDropEvent((event) => {
        if (event.payload.type === 'drop') {
          importDropped(event.payload.paths);
        }
      }),
    ];
    return () => {
      unlisteners.forEach((p) => p.then((u) => u()));
    };
  });
</script>

<svelte:window onkeydown={onKeydown} />

<div class="app">
  <TopBar
    bind:this={topBar}
    bind:query
    bind:view
    {brandName}
    {configured}
    alltags={allTags}
    {pinned}
    total={tasks.length}
    shown={filtered.length}
    showFilterBar={loaded && configured}
    oncreate={() => (showCreate = true)}
    onsettings={() => (showSettings = true)}
    ontogglepin={togglePinned}
  />

  {#if !loaded}
    <div class="center-note">読み込み中…</div>
  {:else if !configured}
    <Onboarding
      work={obWork}
      archive={obArchive}
      onpick={pickOnboarding}
      onstart={finishOnboarding}
    />
  {:else}
    <div class="main">
      {#if view === 'list'}
        <ListView tasks={filtered} {selectedPath} state={listState} onselect={selectCard} />
      {:else if view === 'cal'}
        <CalendarView tasks={filtered} {selectedPath} state={calState} onselect={selectCard} />
      {:else if view === 'dash'}
        <Dashboard
          {tasks}
          deepMonths={config?.deep_archive_months ?? 6}
          hasDeepRoot={!!config?.deep_archive_root}
          onselect={(t) => (selectedPath = t.path)}
          ondeepsweep={handleDeepSweep}
        />
      {:else}
        <BoardView
          tasks={filtered}
          {selectedPath}
          dragOver={drag.over}
          dragging={drag.dragging}
          filtering={!!query}
          onselect={selectCard}
          onpointerdown={drag.start}
        />
      {/if}

      {#if selected}
        <DetailPanel
          task={selected}
          {listing}
          alltags={allTags}
          candeep={!!config?.deep_archive_root}
          onclose={() => (selectedPath = null)}
          onopen={handleOpen}
          oncopy={handleCopyPath}
          oncomplete={handleComplete}
          onreopen={handleReopen}
          ondeeparchive={handleDeepArchiveOne}
          onrename={handleRename}
          onsave={handleSaveMeta}
          onopenentry={handleOpenEntry}
        />
      {/if}
    </div>
  {/if}

  {#if drag.dragging && drag.task}
    <div class="ghost" style="left: {drag.ghost.x + 12}px; top: {drag.ghost.y + 10}px">
      {drag.task.name}
    </div>
  {/if}

  <div class="toasts">
    {#each toasts as t (t.id)}
      <div class="toast" class:error={t.kind === 'error'}>{t.msg}</div>
    {/each}
  </div>
</div>

{#if showCreate && config}
  <CreateModal
    templates={config.template_files}
    oncreate={handleCreate}
    onclose={() => (showCreate = false)}
  />
{/if}

{#if showSettings && config}
  <SettingsModal
    {config}
    {autostart}
    onsave={handleSaveSettings}
    onclose={() => (showSettings = false)}
  />
{/if}

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }

  .center-note {
    flex: 1;
    display: grid;
    place-items: center;
    color: var(--ink-3);
  }
  .main {
    flex: 1;
    display: flex;
    min-height: 0;
  }
  .ghost {
    position: fixed;
    z-index: 70;
    pointer-events: none;
    background: var(--surface);
    border: 1px solid var(--manila);
    border-radius: var(--radius);
    box-shadow: var(--shadow-lift);
    padding: 6px 12px;
    font-size: 12.5px;
    font-weight: 600;
    max-width: 240px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .toasts {
    position: fixed;
    bottom: 16px;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    flex-direction: column;
    gap: 8px;
    z-index: 60;
  }
  .toast {
    background: var(--ink);
    color: #fff;
    padding: 8px 16px;
    border-radius: 99px;
    font-size: 12.5px;
    box-shadow: var(--shadow-lift);
    animation: rise 0.18s ease;
  }
  .toast.error {
    background: var(--red);
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
</style>
