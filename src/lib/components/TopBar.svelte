<script lang="ts">
  import { applyTagToQuery, filterLabel, hasTerms, parseTerms, tagSuggestions } from '$lib/search';
  import type { ViewName } from '$lib/types';

  let {
    query = $bindable(),
    view = $bindable(),
    brandName,
    configured,
    alltags,
    pinned,
    total,
    shown,
    showFilterBar,
    oncreate,
    onplaces,
    onsettings,
    ontogglepin,
  }: {
    query: string;
    view: ViewName;
    brandName: string;
    configured: boolean;
    /** 入力中の #タグ に対する候補。使用頻度順 */
    alltags: string[];
    pinned: boolean;
    total: number;
    shown: number;
    showFilterBar: boolean;
    oncreate: () => void;
    onplaces: (e: MouseEvent) => void;
    onsettings: () => void;
    ontogglepin: () => void;
  } = $props();

  const views: { id: ViewName; label: string }[] = [
    { id: 'board', label: 'ボード' },
    { id: 'list', label: 'リスト' },
    { id: 'cal', label: 'カレンダー' },
    { id: 'dash', label: 'ダッシュボード' },
  ];

  let searchEl: HTMLInputElement | undefined = $state();
  let focused = $state(false);
  let highlight = $state(-1);
  let dismissed = $state(false);

  const terms = $derived(parseTerms(query));
  const suggestions = $derived(focused && !dismissed ? tagSuggestions(query, alltags) : []);

  /** Ctrl+F や `/` から呼ばれる */
  export function focusSearch(select = false) {
    searchEl?.focus();
    if (select) searchEl?.select();
  }

  export function clear() {
    query = '';
    searchEl?.focus();
  }

  function applySuggestion(tag: string) {
    query = applyTagToQuery(query, tag);
    highlight = -1;
    searchEl?.focus();
  }

  function onSearchKeydown(e: KeyboardEvent) {
    if (e.isComposing) return;
    if (e.key === 'Escape' && suggestions.length) {
      // 候補を閉じるだけで、絞り込みは解除しない
      e.stopPropagation();
      dismissed = true;
      highlight = -1;
      return;
    }
    if (!suggestions.length) return;
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      e.stopPropagation();
      highlight = Math.min(highlight + 1, suggestions.length - 1);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      e.stopPropagation();
      highlight = Math.max(highlight - 1, -1);
    } else if (e.key === 'Enter' && highlight >= 0) {
      e.preventDefault();
      e.stopPropagation();
      applySuggestion(suggestions[highlight]);
    }
  }
</script>

<header class="topbar">
  <div class="brand">
    <svg width="18" height="15" viewBox="0 0 18 15" aria-hidden="true">
      <path
        d="M1 3.5C1 2.7 1.7 2 2.5 2h4l1.5 2h7.5c.8 0 1.5.7 1.5 1.5v7c0 .8-.7 1.5-1.5 1.5h-13C1.7 14 1 13.3 1 12.5v-9z"
        fill="var(--manila)"
        opacity="0.85"
      />
    </svg>
    {#if brandName}<span class="brand-name">{brandName}</span>{/if}
  </div>
  <nav class="view-seg" aria-label="表示切り替え">
    {#each views as v (v.id)}
      <button class="view-btn" class:active={view === v.id} onclick={() => (view = v.id)}>
        {v.label}
      </button>
    {/each}
  </nav>
  <div class="search-wrap">
    <input
      class="search"
      data-search
      class:filtering={!!query}
      bind:this={searchEl}
      bind:value={query}
      placeholder="検索: 名前・タグ・補足・日付　　#タグ名 でタグだけに限定   (Ctrl+F)"
      disabled={!configured}
      oninput={() => {
        dismissed = false;
        highlight = -1;
      }}
      onkeydown={onSearchKeydown}
      onfocus={() => (focused = true)}
      onblur={() => (focused = false)}
    />
    {#if query}
      <button class="clear" onclick={clear} aria-label="絞り込みを解除" title="絞り込みを解除 (Esc)"
        >✕</button
      >
    {/if}
    {#if suggestions.length > 0}
      <ul class="search-suggest">
        {#each suggestions as tag, i (tag)}
          <li>
            <button
              class:active={i === highlight}
              onmousedown={(e) => e.preventDefault()}
              onclick={() => applySuggestion(tag)}
            >
              <span class="hash">#</span>{tag}
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </div>
  <div class="top-actions">
    <button class="btn primary" onclick={oncreate} disabled={!configured}> ＋ 新しい作業 </button>
    <button
      class="btn gear"
      class:pinned
      onclick={ontogglepin}
      aria-pressed={pinned}
      aria-label="常に最前面に表示"
      title={pinned ? '常に最前面: オン' : '常に最前面に表示'}
    >
      <svg
        width="16"
        height="16"
        viewBox="0 0 24 24"
        fill={pinned ? 'currentColor' : 'none'}
        stroke="currentColor"
        stroke-width="1.8"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"
      >
        <path d="M12 17v5" />
        <path
          d="M9 10.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24V16a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V7a1 1 0 0 1 1-1 2 2 0 0 0 0-4H8a2 2 0 0 0 0 4 1 1 0 0 1 1 1z"
        />
      </svg>
    </button>
    <!-- タスクに属さない場所への入口。一覧の中身は親が組み立てる -->
    <button
      class="btn gear"
      onclick={onplaces}
      disabled={!configured}
      aria-label="フォルダを開く"
      title="フォルダを開く"
    >
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
        <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z" />
      </svg>
    </button>
    <button class="btn gear" onclick={onsettings} aria-label="設定" title="設定">
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

<!-- 記号だけ打っている途中は条件が無いので、バーはまだ出さない -->
{#if showFilterBar && hasTerms(terms)}
  <div class="filter-bar" role="status">
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
      <path d="M22 3H2l8 9.46V19l4 2v-8.54L22 3z" />
    </svg>
    <span class="filter-term">{filterLabel(terms)}</span>
    <span class="filter-count">{total} 件中 {shown} 件</span>
    <button class="filter-clear" onclick={clear}>絞り込みを解除 (Esc)</button>
  </div>
{/if}

<style>
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
  .search.filtering {
    background: var(--manila-soft);
    border-color: var(--manila);
    color: var(--ink);
    font-weight: 600;
  }

  /* 絞り込み中であることと、解除方法を常に見える場所に出す */
  .filter-bar {
    display: flex;
    align-items: center;
    gap: 10px;
    flex: none;
    padding: 7px 16px;
    background: var(--manila-soft);
    border-bottom: 1px solid var(--manila);
    color: var(--manila-ink);
    font-size: 12.5px;
  }
  .filter-term {
    font-weight: 700;
  }
  .filter-count {
    color: var(--ink-2);
  }
  .filter-clear {
    margin-left: auto;
    border: 1px solid var(--manila);
    background: var(--surface);
    color: var(--manila-ink);
    border-radius: 99px;
    padding: 3px 12px;
    font-size: 11.5px;
  }
  .filter-clear:hover {
    background: var(--manila);
    color: var(--on-solid);
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
  .search-suggest {
    position: absolute;
    top: calc(100% + 4px);
    left: 12px;
    right: 12px;
    z-index: 30;
    list-style: none;
    margin: 0;
    padding: 4px;
    background: var(--surface);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius);
    box-shadow: var(--shadow-lift);
    max-height: 240px;
    overflow-y: auto;
  }
  .search-suggest button {
    width: 100%;
    text-align: left;
    border: none;
    background: none;
    padding: 6px 10px;
    border-radius: 5px;
    font-size: 12.5px;
  }
  .search-suggest button:hover,
  .search-suggest button.active {
    background: var(--manila-soft);
  }
  .search-suggest .hash {
    color: var(--manila);
    font-weight: 700;
    margin-right: 1px;
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
    color: var(--on-solid);
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
  .gear.pinned {
    background: var(--manila-soft);
    border-color: var(--manila);
    color: var(--manila-ink);
  }

  /* 付箋のように細くしたときは、ヘッダーを削って一覧に幅を渡す */
  @media (max-width: 720px) {
    .topbar {
      gap: 8px;
      padding: 8px 10px;
    }
    .brand-name {
      display: none;
    }
    .view-btn {
      padding: 6px 9px;
    }
  }
  /* さらに狭いときは操作を隠さず、検索欄を次の行へ送る */
  @media (max-width: 620px) {
    .topbar {
      flex-wrap: wrap;
    }
    .search-wrap {
      order: 3;
      flex-basis: 100%;
      max-width: none;
      margin: 0;
    }
  }
</style>
