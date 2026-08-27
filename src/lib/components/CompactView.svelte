<script lang="ts">
  import type { Task } from '$lib/types';
  import { daysSince, isOverdue } from '$lib/format';

  let {
    tasks,
    pinned,
    ontogglepin,
    onopen,
  }: {
    tasks: Task[];
    pinned: boolean;
    ontogglepin: () => void;
    /** 行を押したときの動作。付箋から作業へ入る唯一の経路 */
    onopen: (t: Task) => void;
  } = $props();

  // 付箋に出すのは今動いている作業だけ。未着手はまだ始まっておらず、
  // 完了は終わっている。脇に置いて思い出したいのは進行中のものに限られる
  const rows = $derived(
    tasks
      .filter((t) => t.status === 'doing')
      .sort((a, b) => (b.last_activity ?? '').localeCompare(a.last_activity ?? ''))
  );
</script>

<div class="compact">
  <header>
    <span class="count">進行中 {rows.length}</span>
    <button
      class="pin"
      class:on={pinned}
      onclick={ontogglepin}
      aria-label={pinned ? '最前面を解除' : '最前面に固定'}
      title={pinned ? '最前面を解除' : '最前面に固定'}
    >
      <svg
        width="14"
        height="14"
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
  </header>

  {#if rows.length === 0}
    <p class="empty">進行中の作業はありません</p>
  {:else}
    <ul>
      {#each rows as task (task.path)}
        <li>
          <!-- 詳細パネルを出す幅が無いので、選択ではなくフォルダを開く -->
          <button
            class="row"
            class:held={task.on_hold_since}
            onclick={() => onopen(task)}
            title="{task.folder_name}&#10;クリックでエクスプローラーを開く"
          >
            <span class="name">{task.name}</span>
            {#if task.on_hold_since}
              <span class="badge hold">保留</span>
            {:else if task.stale}
              <span class="badge stale" title="この日数、フォルダに動きがありません">
                放置{daysSince(task.last_activity)}日
              </span>
            {/if}
            {#if task.due}
              <span class="badge due" class:overdue={isOverdue(task.due)}>
                {task.due.slice(5).replace('-', '/')}
              </span>
            {/if}
          </button>
        </li>
      {/each}
    </ul>
  {/if}

</div>

<style>
  .compact {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 0;
    padding: 6px 8px 8px;
    gap: 6px;
  }
  header {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: none;
  }
  .count {
    flex: 1;
    font-size: 11px;
    color: var(--ink-2);
  }
  .pin {
    border: 1px solid transparent;
    background: none;
    color: var(--ink-3);
    border-radius: 6px;
    padding: 3px;
    display: inline-flex;
  }
  .pin.on {
    color: var(--accent);
    border-color: var(--manila-tab);
    background: var(--manila-soft);
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    overflow-y: auto;
    min-height: 0;
    flex: 1;
  }
  li + li {
    margin-top: 3px;
  }
  .row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 6px 8px;
    border: 1px solid var(--line);
    border-left: 3px solid var(--manila);
    border-radius: 6px;
    background: var(--surface);
    font: inherit;
    font-size: 12px;
    color: inherit;
    text-align: left;
    cursor: pointer;
  }
  .row:hover {
    background: var(--surface-2);
  }
  /* 保留は手を止めている印。ボードと同じく沈めて、色の意味を揃える */
  .row.held {
    border-left-color: var(--slate);
    background: var(--surface-sunk);
    color: var(--ink-2);
  }
  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .badge {
    flex: none;
    font-size: 10px;
    padding: 1px 5px;
    border-radius: 99px;
    white-space: nowrap;
  }
  .badge.hold {
    background: var(--slate-soft);
    color: var(--slate);
  }
  .badge.stale {
    background: var(--red-soft);
    color: var(--red);
  }
  .badge.due {
    background: var(--slate-soft);
    color: var(--ink-2);
  }
  .badge.due.overdue {
    background: var(--red);
    color: var(--on-solid);
  }

  .empty {
    flex: 1;
    margin: 0;
    display: grid;
    place-items: center;
    font-size: 11px;
    color: var(--ink-3);
  }
</style>
