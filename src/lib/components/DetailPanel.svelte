<script lang="ts">
  import type { ChecklistItem, FolderEntry, ProgressMode, Status, Task } from '$lib/types';
  import { fmtDateTime, fmtDurationCompact, fmtMinutes, fmtSize, parseDuration } from '$lib/format';

  let {
    task,
    entries,
    alltags = [],
    candeep = false,
    onclose,
    onopen,
    oncopy,
    oncomplete,
    onreopen,
    ondeeparchive,
    onrename,
    onsave,
  }: {
    task: Task;
    entries: FolderEntry[];
    /** 他のタスクで使われているタグ。よく使う順 */
    alltags?: string[];
    candeep?: boolean;
    onclose: () => void;
    onopen: (t: Task) => void;
    oncopy: (t: Task) => void;
    oncomplete: (t: Task) => void;
    onreopen: (t: Task) => void;
    ondeeparchive: (t: Task) => void;
    onrename: (t: Task, name: string) => void;
    onsave: (
      t: Task,
      status: Status,
      tags: string[],
      due: string | null,
      memo: string,
      checklist: ChecklistItem[],
      manualProgress: number | null,
      progressMode: ProgressMode
    ) => void;
  } = $props();

  let currentPath = $state('');
  let status = $state<Status>('doing');
  let tags = $state<string[]>([]);
  let due = $state('');
  let memo = $state('');
  let checklist = $state<ChecklistItem[]>([]);
  let manualProgress = $state<number | null>(null);
  let progressMode = $state<ProgressMode>('auto');
  let newItemText = $state('');

  $effect(() => {
    if (task.path !== currentPath) {
      currentPath = task.path;
      status = task.status;
      tags = [...task.tags];
      tagInput = '';
      due = task.due ?? '';
      memo = task.memo;
      checklist = task.checklist.map((i) => ({ ...i }));
      manualProgress = task.manual_progress;
      progressMode = task.progress_mode;
      newItemText = '';
      renaming = false;
    }
  });

  // 名前の編集はフォルダ名そのものを変更する（表示名は持たない）
  let renaming = $state(false);
  let renameText = $state('');
  let renameInput: HTMLInputElement | undefined = $state();

  function startRename() {
    renameText = task.name;
    renaming = true;
    queueMicrotask(() => renameInput?.select());
  }

  // Enter で確定すると入力欄が外れて blur も発火するため、二重実行を防ぐ
  function commitRename() {
    if (!renaming) return;
    renaming = false;
    const next = renameText.trim();
    if (next && next !== task.name) onrename(task, next);
  }

  function onRenameKeydown(e: KeyboardEvent) {
    e.stopPropagation();
    if (e.key === 'Enter') commitRename();
    if (e.key === 'Escape') renaming = false;
  }

  // タグはチップ操作のたびに保存するため、保存ボタンの対象には含めない
  const dirty = $derived(
    status !== task.status || (due || null) !== task.due || memo !== task.memo
  );

  let tagInput = $state('');
  let tagFocused = $state(false);
  let tagHighlight = $state(-1);

  const SUGGEST_LIMIT = 6;

  // タグが増えても一覧が長くならないよう、頻度順の上位だけを出す。
  // 絞り込み時は前方一致を優先し、打った文字で始まるタグが埋もれないようにする
  const suggestions = $derived.by(() => {
    const q = tagInput.trim().toLowerCase();
    const pool = alltags.filter((t) => !tags.includes(t));
    if (!q) return pool.slice(0, SUGGEST_LIMIT);
    return pool
      .filter((t) => t.toLowerCase().includes(q))
      .sort(
        (a, b) =>
          Number(b.toLowerCase().startsWith(q)) - Number(a.toLowerCase().startsWith(q))
      )
      .slice(0, SUGGEST_LIMIT);
  });

  function addTag(raw: string) {
    const value = raw.replace(/[,、]/g, '').trim();
    tagInput = '';
    tagHighlight = -1;
    if (!value || tags.includes(value)) return;
    tags.push(value);
    save();
  }

  function removeTag(index: number) {
    tags.splice(index, 1);
    save();
  }

  function onTagKeydown(e: KeyboardEvent) {
    // 日本語変換中の Enter は確定操作なのでタグ追加に使わない
    if (e.isComposing) return;
    if (e.key === 'Enter' || e.key === ',' || e.key === '、') {
      e.preventDefault();
      e.stopPropagation();
      addTag(tagHighlight >= 0 ? suggestions[tagHighlight] : tagInput);
    } else if (e.key === 'ArrowDown' && suggestions.length) {
      e.preventDefault();
      e.stopPropagation();
      tagHighlight = Math.min(tagHighlight + 1, suggestions.length - 1);
    } else if (e.key === 'ArrowUp' && suggestions.length) {
      e.preventDefault();
      e.stopPropagation();
      tagHighlight = Math.max(tagHighlight - 1, -1);
    } else if (e.key === 'Backspace' && !tagInput && tags.length) {
      e.stopPropagation();
      removeTag(tags.length - 1);
    } else if (e.key === 'Escape' && (tagInput || tagHighlight >= 0)) {
      // 候補を閉じるだけ。空のときは通常どおり上位へ渡す
      e.stopPropagation();
      tagInput = '';
      tagHighlight = -1;
    }
  }

  function onTagBlur() {
    tagFocused = false;
    if (tagInput.trim()) addTag(tagInput);
  }

  // 手動なのに値が無い状態は「未設定」とみなし、やることがあれば自動計算に戻す
  const manualMode = $derived(
    checklist.length === 0 || (progressMode === 'manual' && manualProgress !== null)
  );

  // 表示用の進捗（ローカル編集を即時反映）
  const localProgress = $derived.by(() => {
    if (manualMode) return manualProgress;
    const ests = checklist.map((i) => i.estimate_min).filter((e): e is number => e != null);
    const avg = ests.length ? Math.max(1, Math.round(ests.reduce((a, b) => a + b, 0) / ests.length)) : 1;
    const w = (i: ChecklistItem) => Math.max(1, i.estimate_min ?? avg);
    const total = checklist.reduce((a, i) => a + w(i), 0);
    const done = checklist.filter((i) => i.done).reduce((a, i) => a + w(i), 0);
    return Math.round((done * 100) / Math.max(1, total));
  });
  const localRemaining = $derived.by(() => {
    const rest = checklist.filter((i) => !i.done && i.estimate_min != null);
    if (!checklist.some((i) => i.estimate_min != null)) return null;
    return rest.reduce((a, i) => a + (i.estimate_min ?? 0), 0);
  });

  function save() {
    onsave(task, status, tags, due || null, memo, checklist, manualProgress, progressMode);
  }

  function setMode(mode: ProgressMode) {
    if (progressMode === mode) return;
    // 手動へ切り替えたときに数値が消えないよう、その時点の計算値を引き継ぐ
    if (mode === 'manual' && manualProgress === null) {
      manualProgress = localProgress ?? 0;
    }
    progressMode = mode;
    save();
  }

  // やることの操作は待たせず即保存する
  function toggleItem(index: number) {
    checklist[index].done = !checklist[index].done;
    save();
  }
  function removeItem(index: number) {
    checklist.splice(index, 1);
    save();
  }
  function addItem() {
    const text = newItemText.trim();
    if (!text) return;
    checklist.push({ text, done: false, estimate_min: null });
    newItemText = '';
    save();
  }
  // 単位つき自由入力("1.5h"/"90m"/"45分"、単位なしは分)を分に正規化して保存
  function setEstimate(index: number, value: string) {
    checklist[index].estimate_min = parseDuration(value);
    save();
  }

  // ステッパー: 15分刻みで増減（ボタンと↑↓キーの両方から使う）
  function stepEstimate(index: number, delta: number) {
    const next = (checklist[index].estimate_min ?? 0) + delta;
    checklist[index].estimate_min = next > 0 ? next : null;
    save();
  }

  function onEstimateKeydown(e: KeyboardEvent, index: number) {
    if (e.key === 'ArrowUp' || e.key === 'ArrowDown') {
      e.preventDefault();
      stepEstimate(index, e.key === 'ArrowUp' ? 15 : -15);
    }
  }
</script>

<aside class="panel">
  <header>
    <div class="path-line">
      <span class="mono path" title={task.path}>{task.path}</span>
      <button class="icon-btn" onclick={onclose} title="閉じる (Esc)" aria-label="閉じる">✕</button>
    </div>
    {#if renaming}
      <div class="rename-row">
        <input
          class="field rename-input"
          bind:this={renameInput}
          bind:value={renameText}
          onkeydown={onRenameKeydown}
          onblur={commitRename}
          aria-label="作業名"
        />
        <span class="rename-hint">
          Enterで確定。フォルダ名{task.date_prefix ? 'の日付以降' : ''}が変わります
        </span>
      </div>
    {:else}
      <h2>
        <span class="title-text">{task.name}</span>
        <button class="rename-btn" onclick={startRename} title="名前を変更">
          <svg
            width="13"
            height="13"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
          >
            <path d="M12 20h9" />
            <path d="M16.5 3.5a2.12 2.12 0 0 1 3 3L7 19l-4 1 1-4Z" />
          </svg>
        </button>
      </h2>
    {/if}
    <div class="actions">
      <button class="btn" onclick={() => onopen(task)}>エクスプローラーで開く</button>
      <button class="btn" onclick={() => oncopy(task)}>パスをコピー</button>
      {#if task.archived}
        <button class="btn" onclick={() => onreopen(task)}>作業に戻す</button>
        {#if candeep}
          <button class="btn" onclick={() => ondeeparchive(task)}>ディープアーカイブへ</button>
        {/if}
      {:else}
        <button class="btn primary" onclick={() => oncomplete(task)}>完了してアーカイブ</button>
      {/if}
    </div>
  </header>

  <section class="meta">
    {#if !task.archived}
      <div class="row">
        <span class="label">状態</span>
        <div class="seg" role="radiogroup" aria-label="状態">
          <button
            class="seg-btn"
            class:active={status === 'backlog'}
            onclick={() => (status = 'backlog')}>未着手</button
          >
          <button
            class="seg-btn"
            class:active={status === 'doing'}
            onclick={() => (status = 'doing')}>進行中</button
          >
        </div>
      </div>
    {:else}
      <div class="row">
        <span class="label">完了日</span>
        <span>{fmtDateTime(task.completed_at)}</span>
      </div>
    {/if}
    <!-- アーカイブ済みは完了日が状態を語るため、空の進捗操作は出さない -->
    {#if !task.archived}
    <div class="row progress-row">
      <span class="label">進捗</span>
      <div class="p-body">
        {#if manualMode}
          <div class="p-line">
            <!-- 自動モードのバーと同じく、進んだぶんを左から塗る -->
            <input
              type="range"
              min="0"
              max="100"
              step="5"
              value={manualProgress ?? 0}
              style="--filled: {manualProgress ?? 0}%"
              oninput={(e) => (manualProgress = Number(e.currentTarget.value))}
              onchange={save}
              aria-label="進捗率"
            />
            <span class="p-value mono">
              {manualProgress !== null ? `${manualProgress}%` : '—'}
            </span>
            {#if manualProgress !== null}
              <button
                class="icon-btn"
                onclick={() => {
                  manualProgress = null;
                  save();
                }}
                title={checklist.length > 0 ? '自動計算に戻す' : '進捗をクリア'}
              >
                ✕
              </button>
            {/if}
          </div>
        {:else}
          <div class="p-line">
            <div
              class="bar"
              role="progressbar"
              aria-valuenow={localProgress}
              aria-valuemin={0}
              aria-valuemax={100}
            >
              <div class="fill" style="width: {localProgress}%"></div>
            </div>
            <span class="p-value mono">{localProgress}%</span>
          </div>
        {/if}
        {#if (localRemaining !== null && localRemaining > 0) || checklist.length > 0}
          <div class="p-meta">
            {#if localRemaining !== null && localRemaining > 0}
              <span>残り{fmtMinutes(localRemaining)}</span>
            {/if}
            {#if checklist.length > 0}
              <span class="mode-seg" role="radiogroup" aria-label="進捗の計算方法">
                <button
                  class="mode-btn"
                  class:active={progressMode === 'auto'}
                  onclick={() => setMode('auto')}
                  title="やることの消化から自動計算">自動</button
                >
                <button
                  class="mode-btn"
                  class:active={progressMode === 'manual'}
                  onclick={() => setMode('manual')}
                  title="スライダーで手動入力">手動</button
                >
              </span>
            {/if}
          </div>
        {/if}
      </div>
    </div>
    {/if}
    <div class="row tag-row">
      <span class="label">タグ</span>
      <div class="tag-field" class:focused={tagFocused}>
        <div class="chips">
          {#each tags as tag, i (tag)}
            <span class="chip">
              {tag}
              <button class="chip-x" onclick={() => removeTag(i)} aria-label="{tag} を外す">✕</button>
            </span>
          {/each}
          <input
            class="tag-input"
            bind:value={tagInput}
            placeholder={tags.length ? '' : 'タグを追加'}
            onkeydown={onTagKeydown}
            onfocus={() => (tagFocused = true)}
            onblur={onTagBlur}
            aria-label="タグを追加"
          />
        </div>
        {#if tagFocused && suggestions.length > 0}
          <ul class="suggest">
            {#each suggestions as s, i (s)}
              <li>
                <button
                  class:active={i === tagHighlight}
                  onmousedown={(e) => e.preventDefault()}
                  onclick={() => addTag(s)}
                >
                  {s}
                </button>
              </li>
            {/each}
          </ul>
        {/if}
      </div>
    </div>
    <div class="row">
      <span class="label">期限</span>
      <input class="field" type="date" bind:value={due} />
    </div>
    <div class="row memo-row">
      <span class="label">メモ</span>
      <textarea class="field" rows="4" bind:value={memo} placeholder="フォルダ名に入れたくない補足を書く場所"
      ></textarea>
    </div>
    {#if dirty}
      <div class="save-row">
        <button class="btn primary" onclick={save}>変更を保存</button>
      </div>
    {/if}
  </section>

  <section class="checklist">
    <h3>やること</h3>
    {#if checklist.length === 0 && !task.archived}
      <p class="hint">追加すると、消化状況から進捗を自動計算します</p>
    {/if}

    {#if checklist.length > 0}
      <ul class="items">
        {#each checklist as item, i (i)}
          <li class:done={item.done}>
            <input
              type="checkbox"
              checked={item.done}
              onchange={() => toggleItem(i)}
              aria-label={item.text}
            />
            <span class="item-text">{item.text}</span>
            <span class="est-wrap">
              <input
                class="est mono"
                type="text"
                placeholder="見積"
                title="見積。1.5h / 90m / 1h30m / 45分 など。単位なしの数値は分。↑↓で15分刻み"
                value={item.estimate_min != null ? fmtDurationCompact(item.estimate_min) : ''}
                onchange={(e) => setEstimate(i, e.currentTarget.value)}
                onkeydown={(e) => onEstimateKeydown(e, i)}
              />
              <span class="steppers">
                <button
                  class="step"
                  onclick={() => stepEstimate(i, 15)}
                  aria-label="見積を15分増やす"
                  tabindex="-1">▴</button
                >
                <button
                  class="step"
                  onclick={() => stepEstimate(i, -15)}
                  aria-label="見積を15分減らす"
                  tabindex="-1">▾</button
                >
              </span>
            </span>
            <button class="icon-btn" onclick={() => removeItem(i)} aria-label="削除" title="削除">
              ✕
            </button>
          </li>
        {/each}
      </ul>
    {/if}

    <div class="add-row">
      <input
        class="field"
        bind:value={newItemText}
        placeholder="やることを追加（Enter）"
        onkeydown={(e) => e.key === 'Enter' && addItem()}
      />
      <button class="btn" onclick={addItem} disabled={!newItemText.trim()}>追加</button>
    </div>
  </section>

  <section class="files">
    <h3>フォルダの中身 <span class="count">{entries.length} 項目</span></h3>
    {#if entries.length === 0}
      <p class="empty">まだファイルがありません</p>
    {:else}
      <ul>
        {#each entries as entry (entry.name)}
          <li>
            <span class="ficon">{entry.is_dir ? '▸' : '·'}</span>
            <span class="fname" title={entry.name}>{entry.name}</span>
            <span class="fmeta mono">
              {entry.is_dir ? '' : fmtSize(entry.size)}
            </span>
            <span class="fmeta mono">{fmtDateTime(entry.modified).slice(5)}</span>
          </li>
        {/each}
      </ul>
    {/if}
  </section>
</aside>

<style>
  .panel {
    width: 340px;
    flex: none;
    background: var(--surface);
    border-left: 1px solid var(--line);
    display: flex;
    flex-direction: column;
    overflow-y: auto;
  }

  header {
    padding: 14px 16px 12px;
    border-bottom: 1px solid var(--line);
  }
  .path-line {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 6px;
  }
  .path {
    flex: 1;
    font-size: 10.5px;
    color: var(--ink-3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    direction: rtl;
    text-align: left;
  }
  .icon-btn {
    border: none;
    background: none;
    color: var(--ink-2);
    padding: 2px 6px;
    border-radius: 4px;
    flex: none;
  }
  .icon-btn:hover {
    background: var(--surface-2);
  }
  h2 {
    margin: 0 0 10px;
    font-size: 16px;
    line-height: 1.35;
    word-break: break-all;
    display: flex;
    align-items: baseline;
    gap: 6px;
  }
  .title-text {
    flex: 1;
  }
  .rename-btn {
    flex: none;
    border: none;
    background: none;
    color: var(--ink-3);
    padding: 2px 4px;
    border-radius: 4px;
    line-height: 0;
    opacity: 0;
    transition: opacity 0.12s ease;
  }
  header:hover .rename-btn,
  .rename-btn:focus-visible {
    opacity: 1;
  }
  .rename-btn:hover {
    color: var(--ink);
    background: var(--surface-2);
  }
  .rename-row {
    margin: 0 0 10px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .rename-input {
    font-size: 15px;
    font-weight: 600;
    padding: 5px 9px;
  }
  .rename-hint {
    font-size: 10.5px;
    color: var(--ink-3);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .actions .btn {
    padding: 5px 10px;
    font-size: 12px;
  }

  .meta {
    padding: 12px 16px;
    border-bottom: 1px solid var(--line);
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .row {
    display: grid;
    grid-template-columns: 44px 1fr;
    align-items: center;
    gap: 10px;
  }
  .memo-row {
    align-items: start;
  }
  /* タグは区切り記号を覚えずに済むよう、チップと候補で操作する */
  .tag-row {
    align-items: start;
  }
  .tag-field {
    position: relative;
    min-width: 0;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
    padding: 4px 6px;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius);
    background: var(--surface);
    min-height: 30px;
  }
  .tag-field.focused .chips {
    outline: 2px solid var(--focus);
    outline-offset: -1px;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    background: var(--manila-soft);
    border: 1px solid var(--manila-tab);
    color: #7a5310;
    border-radius: 99px;
    padding: 1px 4px 1px 9px;
    font-size: 11.5px;
    max-width: 100%;
  }
  .chip-x {
    border: none;
    background: none;
    color: #a8792c;
    font-size: 9px;
    line-height: 1;
    padding: 2px 3px;
    border-radius: 99px;
  }
  .chip-x:hover {
    background: var(--manila);
    color: #fff;
  }
  .tag-input {
    flex: 1;
    min-width: 70px;
    border: none;
    outline: none;
    background: none;
    padding: 2px;
    font-size: 12.5px;
  }
  .suggest {
    position: absolute;
    top: calc(100% + 3px);
    left: 0;
    right: 0;
    z-index: 5;
    list-style: none;
    margin: 0;
    padding: 3px;
    background: var(--surface);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius);
    box-shadow: var(--shadow-lift);
    max-height: 168px;
    overflow-y: auto;
  }
  .suggest button {
    width: 100%;
    text-align: left;
    border: none;
    background: none;
    padding: 5px 8px;
    border-radius: 5px;
    font-size: 12px;
  }
  .suggest button:hover,
  .suggest button.active {
    background: var(--manila-soft);
  }

  /* 進捗は状態の隣に置き、パネルを開いた時点で見えるようにする */
  .progress-row {
    align-items: center;
  }
  .p-body {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .p-line {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  /* 既定のトラックは濃く「満タン」に見えるため、進捗バーと同じ配色に揃える */
  .p-line input[type='range'] {
    flex: 1;
    min-width: 0;
    height: 5px;
    appearance: none;
    background: linear-gradient(
      to right,
      var(--manila) var(--filled, 0%),
      var(--slate-soft) var(--filled, 0%)
    );
    border-radius: 99px;
  }
  .p-line input[type='range']::-webkit-slider-thumb {
    appearance: none;
    width: 13px;
    height: 13px;
    border-radius: 50%;
    background: var(--manila);
    border: 2px solid var(--surface);
    box-shadow: 0 0 0 1px var(--line-strong);
    cursor: pointer;
  }
  .p-value {
    width: 38px;
    text-align: right;
    font-size: 11.5px;
    color: var(--ink-2);
    flex: none;
  }
  .p-meta {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 11px;
    color: var(--ink-3);
  }
  .p-meta .mode-seg {
    margin-left: auto;
  }
  .memo-row .label {
    padding-top: 7px;
  }
  .label {
    font-size: 11px;
    color: var(--ink-2);
  }
  textarea.field {
    resize: vertical;
    line-height: 1.5;
  }
  .seg {
    display: inline-flex;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius);
    overflow: hidden;
    width: fit-content;
  }
  .seg-btn {
    border: none;
    background: var(--surface);
    padding: 5px 12px;
    color: var(--ink-2);
  }
  .seg-btn + .seg-btn {
    border-left: 1px solid var(--line-strong);
  }
  .seg-btn.active {
    background: var(--ink);
    color: #fff;
  }
  .save-row {
    display: flex;
    justify-content: flex-end;
  }

  /* やること */
  .checklist {
    padding: 12px 16px;
    border-bottom: 1px solid var(--line);
  }
  .checklist h3 {
    margin: 0 0 8px;
    font-size: 12px;
    color: var(--ink-2);
    font-weight: 600;
  }
  .mode-seg {
    display: inline-flex;
    border: 1px solid var(--line-strong);
    border-radius: 6px;
    overflow: hidden;
  }
  .mode-btn {
    border: none;
    background: var(--surface);
    padding: 2px 9px;
    font-size: 10.5px;
    color: var(--ink-2);
  }
  .mode-btn + .mode-btn {
    border-left: 1px solid var(--line-strong);
  }
  .mode-btn.active {
    background: var(--ink);
    color: #fff;
  }
  .bar {
    flex: 1;
    height: 5px;
    border-radius: 99px;
    background: var(--slate-soft);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    border-radius: 99px;
    background: var(--manila);
    transition: width 0.2s ease;
  }
  .items {
    list-style: none;
    margin: 0 0 8px;
    padding: 0;
  }
  .items li {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 0;
    font-size: 12.5px;
  }
  .items li.done .item-text {
    color: var(--ink-3);
    text-decoration: line-through;
  }
  .item-text {
    flex: 1;
    word-break: break-all;
  }
  .est-wrap {
    display: inline-flex;
    align-items: stretch;
    flex: none;
  }
  .est {
    width: 62px;
    padding: 2px 6px;
    border: 1px solid var(--line);
    border-radius: 5px 0 0 5px;
    border-right: none;
    font-size: 11px;
    color: var(--ink-2);
    text-align: right;
  }
  .est::placeholder {
    color: var(--ink-3);
  }
  .steppers {
    display: flex;
    flex-direction: column;
    border: 1px solid var(--line);
    border-radius: 0 5px 5px 0;
    overflow: hidden;
  }
  .step {
    border: none;
    background: var(--surface-2);
    color: var(--ink-2);
    font-size: 7px;
    line-height: 1;
    padding: 2px 4px;
    flex: 1;
  }
  .step:hover {
    background: var(--slate-soft);
    color: var(--ink);
  }
  .step + .step {
    border-top: 1px solid var(--line);
  }
  .hint {
    margin: 2px 0 8px;
    font-size: 10.5px;
    color: var(--ink-3);
  }
  .add-row {
    display: flex;
    gap: 6px;
  }
  .add-row .field {
    flex: 1;
    padding: 5px 10px;
    font-size: 12px;
  }
  .add-row .btn {
    padding: 5px 12px;
    font-size: 12px;
  }

  .files {
    padding: 12px 16px 16px;
    flex: 1;
  }
  .files h3 {
    margin: 0 0 8px;
    font-size: 12px;
    color: var(--ink-2);
    font-weight: 600;
  }
  .count {
    font-weight: 400;
    color: var(--ink-3);
  }
  .empty {
    color: var(--ink-3);
    font-size: 12px;
  }
  .files ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .files li {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 4px 2px;
    border-bottom: 1px solid var(--surface-2);
    font-size: 12px;
  }
  .ficon {
    color: var(--manila);
    width: 10px;
    flex: none;
  }
  .fname {
    flex: 1;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .fmeta {
    color: var(--ink-3);
    font-size: 10.5px;
    flex: none;
  }
</style>
