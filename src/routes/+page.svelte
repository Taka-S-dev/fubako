<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { getCurrentWebview } from '@tauri-apps/api/webview';
  import { ask, open as openDialog } from '@tauri-apps/plugin-dialog';
  import { api } from '$lib/api';
  import type {
    AppConfig,
    ChecklistItem,
    FolderEntry,
    ProgressMode,
    Status,
    Task,
  } from '$lib/types';
  import TaskCard from '$lib/components/TaskCard.svelte';
  import DetailPanel from '$lib/components/DetailPanel.svelte';
  import ListView from '$lib/components/ListView.svelte';
  import CalendarView from '$lib/components/CalendarView.svelte';
  import Dashboard from '$lib/components/Dashboard.svelte';
  import CreateModal from '$lib/components/CreateModal.svelte';
  import SettingsModal from '$lib/components/SettingsModal.svelte';

  let config = $state<AppConfig | null>(null);
  let tasks = $state<Task[]>([]);
  let view = $state<'board' | 'list' | 'cal' | 'dash'>('board');
  let loaded = $state(false);
  let query = $state('');
  let selectedPath = $state<string | null>(null);
  let entries = $state<FolderEntry[]>([]);
  let showCreate = $state(false);
  let showSettings = $state(false);
  let autostart = $state(false);
  let toasts = $state<{ id: number; msg: string; kind: 'info' | 'error' }[]>([]);
  let searchEl: HTMLInputElement | undefined = $state();

  // カードのドラッグ&ドロップ（WindowsのTauriではHTML5 DnDが使えないためポインターで自前実装）
  let dragTask = $state<Task | null>(null);
  let dragging = $state(false);
  let ghost = $state({ x: 0, y: 0 });
  let dragOver = $state<Status | null>(null);
  let dragMoved = false;
  let dragStartX = 0;
  let dragStartY = 0;

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

  const filtered = $derived.by(() => {
    const q = query.trim().toLowerCase();
    if (!q) return tasks;
    const words = q.split(/\s+/);
    return tasks.filter((t) => {
      const hay = [
        t.folder_name,
        t.name,
        t.memo,
        t.tags.join(' '),
        t.date_prefix ?? '',
        t.checklist.map((i) => i.text).join(' '),
        t.file_names.join(' '),
      ]
        .join(' ')
        .toLowerCase();
      return words.every((w) => hay.includes(w));
    });
  });

  function byActivity(a: Task, b: Task) {
    return (b.last_activity ?? '').localeCompare(a.last_activity ?? '');
  }
  const columns = $derived.by(() => ({
    backlog: filtered.filter((t) => t.status === 'backlog').sort(byActivity),
    doing: filtered.filter((t) => t.status === 'doing').sort(byActivity),
    done: filtered
      .filter((t) => t.status === 'done')
      .sort((a, b) =>
        (b.completed_at ?? b.folder_name).localeCompare(a.completed_at ?? a.folder_name)
      ),
  }));

  const columnDefs: { status: Status; title: string; empty: string }[] = [
    { status: 'backlog', title: '未着手', empty: '予定の作業はここに並びます' },
    { status: 'doing', title: '進行中', empty: '「＋ 新しい作業」で今日のフォルダを作成' },
    { status: 'done', title: '完了・アーカイブ', empty: '完了した作業はここに並びます' },
  ];

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
      entries = [];
      return;
    }
    api
      .listFolder(path)
      .then((list) => {
        if (selected?.path === path) entries = list;
      })
      .catch(() => (entries = []));
  });

  function startDrag(e: PointerEvent, task: Task) {
    if (e.button !== 0) return;
    dragTask = task;
    dragging = false;
    dragStartX = e.clientX;
    dragStartY = e.clientY;
    window.addEventListener('pointermove', onDragMove);
    window.addEventListener('pointerup', onDragEnd);
  }

  function onDragMove(e: PointerEvent) {
    if (!dragTask) return;
    if (!dragging) {
      if (Math.hypot(e.clientX - dragStartX, e.clientY - dragStartY) < 6) return;
      dragging = true;
      document.body.style.userSelect = 'none';
    }
    ghost = { x: e.clientX, y: e.clientY };
    const el = document.elementFromPoint(e.clientX, e.clientY);
    dragOver = (el?.closest('[data-status]')?.getAttribute('data-status') as Status) ?? null;
  }

  function onDragEnd() {
    window.removeEventListener('pointermove', onDragMove);
    window.removeEventListener('pointerup', onDragEnd);
    document.body.style.userSelect = '';
    const task = dragTask;
    const target = dragOver;
    const moved = dragging;
    dragTask = null;
    dragging = false;
    dragOver = null;
    if (moved) {
      // ドラッグ直後のclickで選択がトグルされるのを抑止
      dragMoved = true;
      setTimeout(() => (dragMoved = false), 0);
      if (task && target) dropOn(task, target);
    }
  }

  function selectCard(t: Task) {
    if (dragMoved) return;
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

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      if (showCreate) showCreate = false;
      else if (showSettings) showSettings = false;
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
      searchEl?.focus();
      searchEl?.select();
    } else if (e.key === '/' && !inField) {
      e.preventDefault();
      searchEl?.focus();
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
      listen('focus-search', () => {
        searchEl?.focus();
        searchEl?.select();
      }),
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
  <header class="topbar">
    <div class="brand">
      <svg width="18" height="15" viewBox="0 0 18 15" aria-hidden="true">
        <path
          d="M1 3.5C1 2.7 1.7 2 2.5 2h4l1.5 2h7.5c.8 0 1.5.7 1.5 1.5v7c0 .8-.7 1.5-1.5 1.5h-13C1.7 14 1 13.3 1 12.5v-9z"
          fill="var(--manila)"
          opacity="0.85"
        />
      </svg>
      {#if brandName}{brandName}{/if}
    </div>
    <nav class="view-seg" aria-label="表示切り替え">
      <button class="view-btn" class:active={view === 'board'} onclick={() => (view = 'board')}>
        ボード
      </button>
      <button class="view-btn" class:active={view === 'list'} onclick={() => (view = 'list')}>
        リスト
      </button>
      <button class="view-btn" class:active={view === 'cal'} onclick={() => (view = 'cal')}>
        カレンダー
      </button>
      <button class="view-btn" class:active={view === 'dash'} onclick={() => (view = 'dash')}>
        ダッシュボード
      </button>
    </nav>
    <div class="search-wrap">
      <input
        class="search"
        bind:this={searchEl}
        bind:value={query}
        placeholder="検索: 名前・タグ・メモ・日付   (Ctrl+F)"
        disabled={!configured}
      />
      {#if query}
        <button class="clear" onclick={() => (query = '')} aria-label="検索をクリア">✕</button>
      {/if}
    </div>
    <div class="top-actions">
      <button class="btn primary" onclick={() => (showCreate = true)} disabled={!configured}>
        ＋ 新しい作業
      </button>
      <button class="btn gear" onclick={() => (showSettings = true)} aria-label="設定" title="設定">
        <svg
          width="16"
          height="16"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="1.8"
          stroke-linecap="round"
          stroke-linejoin="round"
          aria-hidden="true"
        >
          <circle cx="12" cy="12" r="3" />
          <path
            d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"
          />
        </svg>
      </button>
    </div>
  </header>

  {#if !loaded}
    <div class="center-note">読み込み中…</div>
  {:else if !configured}
    <div class="onboarding">
      <div class="ob-card">
        <h1>作業フォルダを、そのままタスクに</h1>
        <p>
          管理したい作業ディレクトリと、完了後の移動先（アーカイブ）を選ぶと、
          中のフォルダがカードとして並びます。
        </p>
        <div class="ob-row">
          <span class="ob-label">作業ディレクトリ</span>
          <div class="ob-pick">
            <span class="mono ob-path">{obWork || '未選択'}</span>
            <button class="btn" onclick={() => pickOnboarding('work')}>選択…</button>
          </div>
        </div>
        <div class="ob-row">
          <span class="ob-label">アーカイブ先</span>
          <div class="ob-pick">
            <span class="mono ob-path">{obArchive || '未選択'}</span>
            <button class="btn" onclick={() => pickOnboarding('archive')}>選択…</button>
          </div>
        </div>
        <button
          class="btn primary start"
          onclick={finishOnboarding}
          disabled={!obWork || !obArchive}
        >
          はじめる
        </button>
      </div>
    </div>
  {:else}
    <div class="main">
      {#if view === 'list'}
        <ListView tasks={filtered} {selectedPath} onselect={selectCard} />
      {:else if view === 'cal'}
        <CalendarView tasks={filtered} {selectedPath} onselect={selectCard} />
      {:else if view === 'dash'}
        <Dashboard
          {tasks}
          deepMonths={config?.deep_archive_months ?? 6}
          hasDeepRoot={!!config?.deep_archive_root}
          onselect={(t) => (selectedPath = t.path)}
          ondeepsweep={handleDeepSweep}
        />
      {:else}
        <div class="board">
        {#each columnDefs as col (col.status)}
          {@const list = columns[col.status]}
          <section
            class="column"
            aria-label={col.title}
            data-status={col.status}
            class:drag-over={dragOver === col.status && dragging}
          >
            <h2 class="col-head status-{col.status}">
              <span class="col-dot"></span>
              {col.title}
              <span class="col-count">{list.length}</span>
            </h2>
            <div class="col-body">
              {#each list as task (task.path)}
                <TaskCard
                  {task}
                  selected={task.path === selectedPath}
                  onselect={selectCard}
                  onpointerdown={startDrag}
                />
              {:else}
                <p class="col-empty">
                  {query ? '検索に一致する作業はありません' : col.empty}
                </p>
              {/each}
            </div>
          </section>
        {/each}
        </div>
      {/if}

      {#if selected}
        <DetailPanel
          task={selected}
          {entries}
          candeep={!!config?.deep_archive_root}
          onclose={() => (selectedPath = null)}
          onopen={handleOpen}
          oncopy={handleCopyPath}
          oncomplete={handleComplete}
          onreopen={handleReopen}
          ondeeparchive={handleDeepArchiveOne}
          onrename={handleRename}
          onsave={handleSaveMeta}
        />
      {/if}
    </div>
  {/if}

  {#if dragging && dragTask}
    <div class="ghost" style="left: {ghost.x + 12}px; top: {ghost.y + 10}px">
      {dragTask.name}
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

  .topbar {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 10px 16px;
    background: var(--surface);
    border-bottom: 1px solid var(--line);
    flex: none;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 7px;
    font-weight: 700;
    font-size: 13.5px;
    letter-spacing: 0.01em;
    flex: none;
  }
  .search-wrap {
    flex: 1;
    max-width: 560px;
    margin: 0 auto;
    position: relative;
  }
  .search {
    width: 100%;
    padding: 8px 30px 8px 12px;
    font-size: 12.5px;
    border: 1px solid var(--line-strong);
    border-radius: 99px;
    background: var(--surface-2);
  }
  .search:focus {
    background: var(--surface);
    outline: 2px solid var(--focus);
    outline-offset: -1px;
  }
  .clear {
    position: absolute;
    right: 6px;
    top: 50%;
    transform: translateY(-50%);
    border: none;
    background: none;
    color: var(--ink-3);
    padding: 2px 6px;
    border-radius: 99px;
  }
  .clear:hover {
    color: var(--ink);
  }
  .top-actions {
    display: flex;
    gap: 8px;
    flex: none;
  }
  .view-seg {
    display: inline-flex;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius);
    overflow: hidden;
    flex: none;
  }
  .view-btn {
    border: none;
    background: var(--surface);
    padding: 6px 13px;
    font-size: 12px;
    color: var(--ink-2);
  }
  .view-btn + .view-btn {
    border-left: 1px solid var(--line-strong);
  }
  .view-btn:hover {
    background: var(--surface-2);
  }
  .view-btn.active {
    background: var(--ink);
    color: #fff;
  }
  .gear {
    padding: 0;
    width: 34px;
    height: 34px;
    justify-content: center;
    color: var(--ink-2);
  }
  .gear:hover {
    color: var(--ink);
  }
  .gear svg {
    display: block;
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

  .board {
    flex: 1;
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 14px;
    padding: 16px;
    min-width: 0;
    overflow-x: auto;
  }

  .column {
    display: flex;
    flex-direction: column;
    min-width: 220px;
    background: var(--surface-2);
    border: 1px solid var(--line);
    border-radius: 12px;
    min-height: 0;
    transition: border-color 0.12s ease, background 0.12s ease;
  }
  .column.drag-over {
    border-color: var(--manila);
    background: var(--manila-soft);
  }

  .col-head {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0;
    padding: 12px 14px 8px;
    font-size: 12.5px;
    font-weight: 700;
    color: var(--ink-2);
  }
  .col-dot {
    width: 8px;
    height: 8px;
    border-radius: 2px;
    background: var(--slate);
  }
  .col-head.status-doing .col-dot {
    background: var(--manila);
  }
  .col-head.status-done .col-dot {
    background: var(--green);
  }
  .col-count {
    margin-left: auto;
    font-weight: 500;
    font-size: 11.5px;
    color: var(--ink-3);
    background: var(--surface);
    border: 1px solid var(--line);
    padding: 0 8px;
    border-radius: 99px;
  }

  .col-body {
    flex: 1;
    overflow-y: auto;
    padding: 8px 12px 14px;
    display: flex;
    flex-direction: column;
    gap: 13px;
  }
  .col-empty {
    color: var(--ink-3);
    font-size: 12px;
    text-align: center;
    margin-top: 28px;
  }

  /* オンボーディング */
  .onboarding {
    flex: 1;
    display: grid;
    place-items: center;
    padding: 24px;
  }
  .ob-card {
    width: 480px;
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: 14px;
    box-shadow: var(--shadow);
    padding: 28px 30px;
  }
  .ob-card h1 {
    margin: 0 0 8px;
    font-size: 18px;
  }
  .ob-card p {
    margin: 0 0 18px;
    color: var(--ink-2);
    line-height: 1.7;
    font-size: 12.5px;
  }
  .ob-row {
    margin-bottom: 12px;
  }
  .ob-label {
    display: block;
    font-size: 11.5px;
    color: var(--ink-2);
    margin-bottom: 4px;
  }
  .ob-pick {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .ob-path {
    flex: 1;
    font-size: 11.5px;
    color: var(--ink-2);
    background: var(--surface-2);
    border: 1px dashed var(--line-strong);
    border-radius: var(--radius);
    padding: 7px 10px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .start {
    width: 100%;
    justify-content: center;
    margin-top: 10px;
    padding: 10px;
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
