<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { getCurrentWebview } from '@tauri-apps/api/webview';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { ask, open as openDialog } from '@tauri-apps/plugin-dialog';
  import { api } from '$lib/api';
  import { CardDrag } from '$lib/dnd.svelte';
  import { collectTags, filterTasks, matchHint, parseTerms } from '$lib/search';
  import { SEPARATOR, wantsNativeMenu, type MenuItem, type OpenMenu } from '$lib/menu';
  import type {
    AppConfig,
    CalendarViewState,
    FolderEntry,
    FolderListing,
    ListViewState,
    MetaPatch,
    Status,
    Task,
    TaskLink,
    ViewName,
  } from '$lib/types';
  import BoardView from '$lib/components/BoardView.svelte';
  import DetailPanel from '$lib/components/DetailPanel.svelte';
  import ListView from '$lib/components/ListView.svelte';
  import CalendarView from '$lib/components/CalendarView.svelte';
  import Dashboard from '$lib/components/Dashboard.svelte';
  import ContextMenu from '$lib/components/ContextMenu.svelte';
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
  let menu = $state<OpenMenu | null>(null);

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
  // カードに出ていない場所で当たったものだけ、一致箇所を持つ
  const hints = $derived.by(() => {
    const map = new Map<string, string>();
    for (const t of filtered) {
      const hint = matchHint(t, terms);
      if (hint) map.set(t.path, hint);
    }
    return map;
  });

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
      // 起動時に伝えられなかった不具合は、画面が出たこの時点で知らせる
      const warning = await api.takeStartupWarning().catch(() => null);
      if (warning) toast(warning, 'error');
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
  /** 入力欄と選択テキストの上では OS 標準を残し、それ以外の既定メニューは出さない */
  function onContextMenu(e: MouseEvent) {
    if (wantsNativeMenu(e.target)) return;
    e.preventDefault();
  }

  function openMenu(e: MouseEvent, items: MenuItem[]) {
    e.preventDefault();
    e.stopPropagation();
    menu = { x: e.clientX, y: e.clientY, items };
  }

  function taskMenu(e: MouseEvent, task: Task) {
    selectedPath = task.path;
    const items: MenuItem[] = [
      { label: 'エクスプローラーで開く', action: () => handleOpen(task) },
      { label: 'パスをコピー', action: () => handleCopyPath(task) },
      SEPARATOR,
    ];
    if (task.archived) {
      items.push({ label: '作業に戻す', action: () => handleReopen(task) });
      if (config?.deep_archive_root) {
        items.push({
          label: 'ディープアーカイブへ',
          action: () => handleDeepArchiveOne(task),
          danger: true,
        });
      }
    } else {
      items.push({ label: '完了してアーカイブ', action: () => handleComplete(task) });
    }
    openMenu(e, items);
  }

  function entryMenu(e: MouseEvent, task: Task, entry: FolderEntry) {
    const full = `${task.path}\\${entry.rel}`;
    // サブフォルダの中のものは、その親フォルダを開く
    const parent = entry.rel.includes('\\')
      ? `${task.path}\\${entry.rel.slice(0, entry.rel.lastIndexOf('\\'))}`
      : task.path;
    openMenu(e, [
      {
        label: entry.is_dir ? 'エクスプローラーで開く' : '開く',
        action: () => api.openEntry(full).catch((err) => toast(String(err), 'error')),
      },
      {
        label: '保存場所を開く',
        action: () => api.openInExplorer(parent).catch((err) => toast(String(err), 'error')),
      },
      SEPARATOR,
      {
        label: 'パスをコピー',
        action: async () => {
          try {
            await navigator.clipboard.writeText(full);
            toast('パスをコピーしました');
          } catch {
            toast('コピーに失敗しました', 'error');
          }
        },
      },
    ]);
  }

  // 選ぶだけ。同じカードで選択を外すと詳細パネルが開閉して幅が変わり、
  // ボードがガタつく。閉じるのは右上の × と Esc に任せる
  function selectCard(t: Task) {
    if (drag.suppressClick) return;
    selectedPath = t.path;
  }

  // エクスプローラーと同じ作法。押したカードを選んだうえで開く
  function openCard(t: Task) {
    selectedPath = t.path;
    handleOpen(t);
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
        await api.setTaskMeta(newPath, patchOf(task, status));
        if (selectedPath === task.path) selectedPath = newPath;
        toast(`「${task.name}」を作業ディレクトリへ戻しました`);
      } else {
        await api.setTaskMeta(task.path, patchOf(task, status));
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

  /** 今のタスクの内容をそのまま送るための下敷き。状態だけ差し替えて使う */
  function patchOf(task: Task, status: Status): MetaPatch {
    return {
      status,
      tags: task.tags,
      due: task.due,
      memo: task.memo,
      checklist: task.checklist,
      manualProgress: task.manual_progress,
      progressMode: task.progress_mode,
      onHold: task.on_hold_since !== null,
      links: task.links,
    };
  }

  async function handleSaveMeta(task: Task, patch: MetaPatch) {
    try {
      await api.setTaskMeta(task.path, patch);
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

  // 開ける形かは Rust 側で絞る。弾かれた理由はそのまま伝える
  // 参照は `.todo.json` に入るため、フォルダごと他人から届くことがある。
  // プログラムが黙って起動するのだけは避け、意図を一度だけ確かめる。
  // 設定から来る「よく使う場所」は自分で書いたものなので確認しない
  async function handleOpenLink(url: string, trusted = false) {
    try {
      await api.openLink(url, trusted);
    } catch (e) {
      if (String(e) !== 'EXECUTABLE') {
        toast(String(e), 'error');
        return;
      }
      const ok = await ask(`${url}

これはプログラムです。実行しますか。`, {
        title: '参照を開く',
        kind: 'warning',
        okLabel: '実行する',
        cancelLabel: 'キャンセル',
      });
      if (!ok) return;
      api.openLink(url, true).catch((err) => toast(String(err), 'error'));
    }
  }

  /** タスクに属さない場所の一覧。ルートは設定不要で常に出す */
  function placesMenu(e: MouseEvent) {
    const items: MenuItem[] = [];
    const add = (label: string, path: string | null | undefined) => {
      if (path) items.push({ label, action: () => handleOpenLink(path, true) });
    };
    add('作業ディレクトリ', config?.work_root);
    add('アーカイブ', config?.archive_root);
    if (config?.places?.length) {
      items.push(SEPARATOR);
      // 末尾の区切りを落とし、フォルダ名だけを見出しにする
      for (const place of config.places) {
        add(place.replace(/[\\/]+$/, '').split(/[\\/]/).pop() || place, place);
      }
    }
    openMenu(e, items);
  }

  // 場所を指すものなので、ファイル項目と同じ右クリックの作法に揃える
  function linkMenu(e: MouseEvent, link: TaskLink) {
    openMenu(e, [
      { label: '開く', action: () => handleOpenLink(link.url) },
      {
        label: 'コピー',
        action: async () => {
          try {
            await navigator.clipboard.writeText(link.url);
            toast('リンクをコピーしました');
          } catch {
            toast('コピーに失敗しました', 'error');
          }
        },
      },
    ]);
  }

  function handleOpenEntry(task: Task, entry: FolderEntry) {
    api.openEntry(`${task.path}\\${entry.rel}`).catch((e) => toast(String(e), 'error'));
  }

  async function handleSaveSettings(next: AppConfig, auto: boolean) {
    try {
      const warning = await api.setConfig(next);
      config = next;
      await api.setAutostart(auto).catch(() => {});
      autostart = auto;
      showSettings = false;
      await refresh();
      // 保存自体は済んでいるので、伝えることがあっても閉じるところまでは進める
      toast(warning ?? '設定を保存しました', warning ? 'error' : 'info');
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

  // ホットキーで呼び出したあと、手をマウスへ移さずにしまえるようにする。
  // 閉じるボタンと同じくトレイへ格納するだけで、終了はしない
  async function hideToTray() {
    try {
      await getCurrentWindow().hide();
    } catch (e) {
      toast(String(e), 'error');
    }
  }

  // 呼び出し → 絞り込み → 選択 → 開く、をキーボードだけで通せるようにする。
  // 並び順はビューごとに違うため、描画された順序をそのまま辿る

  /** 描画中のカードを列ごとにまとめる。列を持たないビューは1列として扱う */
  function cardColumns(): HTMLElement[][] {
    const groups = new Map<string, HTMLElement[]>();
    for (const el of document.querySelectorAll<HTMLElement>('[data-task]')) {
      const key = el.closest<HTMLElement>('[data-status]')?.dataset.status ?? '';
      const group = groups.get(key);
      if (group) group.push(el);
      else groups.set(key, [el]);
    }
    // Map は挿入順を保つので、値の並びはそのまま描画順になる
    return [...groups.values()];
  }

  function selectCardElement(el: HTMLElement | undefined) {
    if (!el) return;
    selectedPath = el.dataset.task ?? null;
    el.scrollIntoView({ block: 'nearest', inline: 'nearest' });
  }

  function moveSelection(dx: number, dy: number) {
    const columns = cardColumns();
    if (!columns.length) return;

    let col = -1;
    let row = -1;
    columns.forEach((cards, i) => {
      const at = cards.findIndex((el) => el.dataset.task === selectedPath);
      if (at >= 0) {
        col = i;
        row = at;
      }
    });

    // 未選択のときは、進む向きに応じて端から入る
    if (col < 0) {
      const cards = columns[0];
      selectCardElement(dy < 0 ? cards[cards.length - 1] : cards[0]);
      return;
    }

    if (dx !== 0) {
      // 空の列はカードを持たないためそもそも並びに現れない。
      // 列を移るときは縦位置をできるだけ保つ
      const target = columns[col + dx];
      if (!target) return;
      selectCardElement(target[Math.min(row, target.length - 1)]);
      return;
    }

    const cards = columns[col];
    selectCardElement(cards[Math.max(0, Math.min(cards.length - 1, row + dy))]);
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      if (showCreate) showCreate = false;
      else if (showSettings) showSettings = false;
      // 絞り込み中はまず解除する（フィルタバーの案内と一致させる）
      else if (query) topBar?.clear();
      else if (selectedPath) selectedPath = null;
      // 解除するものが無くなったらウィンドウ自体をしまう。
      // 初期設定中だけは、行き先が分からなくなるので残す
      else if (configured) hideToTray();
      return;
    }
    const inField =
      e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement;
    // 検索欄からは続けて一覧へ入れる。候補が出ている間は TopBar 側が先に処理する
    const inSearch = e.target instanceof HTMLElement && e.target.dataset.search !== undefined;
    if (!e.isComposing && (!inField || inSearch)) {
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        e.preventDefault();
        moveSelection(0, e.key === 'ArrowDown' ? 1 : -1);
        return;
      }
      if (e.key === 'Enter' && selected) {
        e.preventDefault();
        handleOpen(selected);
        return;
      }
    }
    // 左右は検索欄では文字カーソルの移動なので、入力欄の外でだけ列移動に使う
    if (!inField && (e.key === 'ArrowLeft' || e.key === 'ArrowRight')) {
      e.preventDefault();
      moveSelection(e.key === 'ArrowRight' ? 1 : -1, 0);
      return;
    }
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
      `「${task.name}」をディープアーカイブへ移動します。\n移動後はアプリの表示・検索対象から外れます（フォルダをこのウィンドウにドロップすれば、作業として戻せます）。`,
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
      `${candidates.length} 件の完了タスクをディープアーカイブへ移動します。\n移動後はアプリの表示・検索対象から外れます（フォルダをこのウィンドウにドロップすれば、作業として戻せます）。`,
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

<svelte:window onkeydown={onKeydown} oncontextmenu={onContextMenu} />

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
    onplaces={placesMenu}
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
        <ListView
          tasks={filtered}
          {selectedPath}
          {hints}
          state={listState}
          onselect={selectCard}
          onopen={openCard}
          oncontext={taskMenu}
        />
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
          {hints}
          dragOver={drag.over}
          dragging={drag.dragging}
          filtering={!!query}
          onselect={selectCard}
          onopen={openCard}
          onpointerdown={drag.start}
          oncontext={taskMenu}
        />
      {/if}

      {#if selected}
        <DetailPanel
          task={selected}
          {listing}
          alltags={allTags}
          onclose={() => (selectedPath = null)}
          onopen={handleOpen}
          oncopy={handleCopyPath}
          oncomplete={handleComplete}
          onreopen={handleReopen}
          onrename={handleRename}
          onsave={handleSaveMeta}
          onopenentry={handleOpenEntry}
          onopenlink={handleOpenLink}
          onlinkcontext={linkMenu}
          onentrycontext={entryMenu}
        />
      {/if}
    </div>
  {/if}

  {#if drag.dragging && drag.task}
    <div class="ghost" style="left: {drag.ghost.x + 12}px; top: {drag.ghost.y + 10}px">
      {drag.task.name}
    </div>
  {/if}

  {#if menu}
    <ContextMenu x={menu.x} y={menu.y} items={menu.items} onclose={() => (menu = null)} />
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
