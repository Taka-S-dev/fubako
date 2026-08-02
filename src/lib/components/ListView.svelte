<script lang="ts">
  import type { ListSortKey, ListViewState, Status, Task } from '$lib/types';
  import { fmtDate, fmtPrefix, isOverdue, relativeDays } from '$lib/format';

  let {
    tasks,
    selectedPath,
    state,
    onselect,
  }: {
    tasks: Task[];
    selectedPath: string | null;
    /** 表示条件は親が持つ。ビューを切り替えても選び直さずに済む */
    state: ListViewState;
    onselect: (t: Task) => void;
  } = $props();

  type SortKey = ListSortKey;

  const statusLabel: Record<Status, string> = {
    backlog: '未着手',
    doing: '進行中',
    done: '完了',
  };
  const statusOrder: Record<Status, number> = { doing: 0, backlog: 1, done: 2 };

  function setSort(key: SortKey) {
    if (state.sortKey === key) {
      state.sortAsc = !state.sortAsc;
    } else {
      state.sortKey = key;
      state.sortAsc = key === 'name' || key === 'status';
    }
  }

  const rows = $derived.by(() => {
    const filtered =
      state.statusFilter === 'all' ? tasks : tasks.filter((t) => t.status === state.statusFilter);
    const dir = state.sortAsc ? 1 : -1;
    return [...filtered].sort((a, b) => {
      let cmp = 0;
      switch (state.sortKey) {
        case 'status':
          cmp = statusOrder[a.status] - statusOrder[b.status];
          break;
        case 'name':
          cmp = a.name.localeCompare(b.name, 'ja');
          break;
        case 'date':
          cmp = (a.date_prefix ?? '').localeCompare(b.date_prefix ?? '');
          break;
        case 'progress':
          cmp = (a.progress ?? -1) - (b.progress ?? -1);
          break;
        case 'due':
          cmp = (a.due ?? '9999').localeCompare(b.due ?? '9999');
          break;
        case 'activity':
          cmp = (a.last_activity ?? '').localeCompare(b.last_activity ?? '');
          break;
      }
      return cmp * dir || a.folder_name.localeCompare(b.folder_name, 'ja');
    });
  });

  const filterDefs: { value: 'all' | Status; label: string }[] = [
    { value: 'all', label: 'すべて' },
    { value: 'backlog', label: '未着手' },
    { value: 'doing', label: '進行中' },
    { value: 'done', label: '完了' },
  ];

  const headers: { key: SortKey; label: string; cls?: string }[] = [
    { key: 'status', label: '状態' },
    { key: 'date', label: '日付' },
    { key: 'name', label: '作業名' },
    { key: 'progress', label: '進捗' },
    { key: 'due', label: '期限' },
    { key: 'activity', label: '更新' },
  ];
</script>

<div class="list-wrap">
  <div class="filters">
    {#each filterDefs as f (f.value)}
      <button
        class="chip"
        class:active={state.statusFilter === f.value}
        onclick={() => (state.statusFilter = f.value)}
      >
        {f.label}
        <span class="chip-count">
          {f.value === 'all' ? tasks.length : tasks.filter((t) => t.status === f.value).length}
        </span>
      </button>
    {/each}
  </div>

  <div class="table-scroll">
    <table>
      <thead>
        <tr>
          {#each headers as h (h.key)}
            <th>
              <button class="th-btn" onclick={() => setSort(h.key)}>
                {h.label}
                {#if state.sortKey === h.key}<span class="arrow">{state.sortAsc ? '▲' : '▼'}</span>{/if}
              </button>
            </th>
          {/each}
          <th class="th-plain">タグ</th>
        </tr>
      </thead>
      <tbody>
        {#each rows as t (t.path)}
          <tr class:selected={t.path === selectedPath} onclick={() => onselect(t)}>
            <td>
              <span class="st st-{t.status}">
                <span class="st-dot"></span>{statusLabel[t.status]}
              </span>
              {#if t.stale}<span class="stale-mini" title="放置">!</span>{/if}
            </td>
            <td class="mono dim">{fmtPrefix(t.date_prefix) ?? '—'}</td>
            <td class="name-cell" title={t.path}>{t.name}</td>
            <td>
              {#if t.progress !== null && !t.archived}
                <span class="mini-bar-wrap">
                  <span class="mini-bar"><span class="mini-fill" style="width:{t.progress}%"
                    ></span></span>
                  <span class="mono dim">{t.progress}%</span>
                </span>
              {:else}
                <span class="dim">—</span>
              {/if}
            </td>
            <td class="mono" class:overdue={!t.archived && isOverdue(t.due)}>
              {t.due ? t.due.replaceAll('-', '/').slice(5) : '—'}
            </td>
            <td class="dim">
              {t.archived ? `完了 ${fmtDate(t.completed_at)}` : relativeDays(t.last_activity)}
            </td>
            <td>
              {#each t.tags.slice(0, 3) as tag (tag)}<span class="tag">{tag}</span>{/each}
            </td>
          </tr>
        {:else}
          <tr><td colspan="7" class="empty">該当する作業はありません</td></tr>
        {/each}
      </tbody>
    </table>
  </div>
</div>

<style>
  .list-wrap {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    padding: 12px 16px 16px;
    gap: 10px;
  }
  .filters {
    display: flex;
    gap: 6px;
    flex: none;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    border: 1px solid var(--line-strong);
    background: var(--surface);
    color: var(--ink-2);
    border-radius: 99px;
    padding: 4px 12px;
    font-size: 12px;
  }
  .chip.active {
    background: var(--ink);
    border-color: var(--ink);
    color: #fff;
  }
  .chip-count {
    font-size: 10.5px;
    opacity: 0.7;
  }

  .table-scroll {
    flex: 1;
    overflow: auto;
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: 12px;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 12.5px;
  }
  thead th {
    position: sticky;
    top: 0;
    background: var(--surface-2);
    border-bottom: 1px solid var(--line);
    text-align: left;
    padding: 0;
    z-index: 1;
  }
  .th-btn {
    width: 100%;
    text-align: left;
    border: none;
    background: none;
    padding: 9px 12px;
    font-size: 11.5px;
    font-weight: 600;
    color: var(--ink-2);
    white-space: nowrap;
  }
  .th-btn:hover {
    color: var(--ink);
  }
  .th-plain {
    padding: 9px 12px;
    font-size: 11.5px;
    font-weight: 600;
    color: var(--ink-2);
  }
  .arrow {
    font-size: 9px;
    margin-left: 3px;
  }
  tbody tr {
    border-bottom: 1px solid var(--surface-2);
    cursor: pointer;
  }
  tbody tr:hover {
    background: var(--surface-2);
  }
  tbody tr.selected {
    background: var(--manila-soft);
  }
  td {
    padding: 7px 12px;
    white-space: nowrap;
  }
  .name-cell {
    font-weight: 600;
    max-width: 260px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .dim {
    color: var(--ink-3);
    font-size: 11.5px;
  }
  .st {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 11.5px;
    color: var(--ink-2);
  }
  .st-dot {
    width: 7px;
    height: 7px;
    border-radius: 2px;
    background: var(--slate);
  }
  .st-doing .st-dot {
    background: var(--manila);
  }
  .st-done .st-dot {
    background: var(--green);
  }
  .stale-mini {
    display: inline-block;
    margin-left: 5px;
    color: var(--red);
    font-weight: 700;
  }
  .mini-bar-wrap {
    display: inline-flex;
    align-items: center;
    gap: 7px;
  }
  .mini-bar {
    display: inline-block;
    width: 72px;
    height: 4px;
    border-radius: 99px;
    background: var(--slate-soft);
    overflow: hidden;
  }
  .mini-fill {
    display: block;
    height: 100%;
    border-radius: 99px;
    background: var(--manila);
  }
  .overdue {
    color: var(--red);
    font-weight: 600;
  }
  .tag {
    background: var(--surface-2);
    border: 1px solid var(--line);
    color: var(--ink-2);
    padding: 0 6px;
    border-radius: 4px;
    font-size: 10.5px;
    margin-right: 4px;
  }
  .empty {
    text-align: center;
    color: var(--ink-3);
    padding: 32px;
  }
</style>
