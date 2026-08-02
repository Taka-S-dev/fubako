<script lang="ts">
  import TaskCard from './TaskCard.svelte';
  import type { Status, Task } from '$lib/types';

  let {
    tasks,
    selectedPath,
    dragOver,
    dragging,
    filtering,
    onselect,
    onpointerdown,
    oncontext,
  }: {
    tasks: Task[];
    selectedPath: string | null;
    /** ドロップ先として光らせる列 */
    dragOver: Status | null;
    dragging: boolean;
    /** 絞り込み中は空の列の案内文を変える */
    filtering: boolean;
    onselect: (t: Task) => void;
    onpointerdown: (e: PointerEvent, t: Task) => void;
    oncontext: (e: MouseEvent, t: Task) => void;
  } = $props();

  const columnDefs: { status: Status; title: string; empty: string }[] = [
    { status: 'backlog', title: '未着手', empty: '予定の作業はここに並びます' },
    { status: 'doing', title: '進行中', empty: '「＋ 新しい作業」で今日のフォルダを作成' },
    { status: 'done', title: '完了・アーカイブ', empty: '完了した作業はここに並びます' },
  ];

  function byActivity(a: Task, b: Task) {
    return (b.last_activity ?? '').localeCompare(a.last_activity ?? '');
  }

  const columns = $derived.by(() => ({
    backlog: tasks.filter((t) => t.status === 'backlog').sort(byActivity),
    doing: tasks.filter((t) => t.status === 'doing').sort(byActivity),
    // 完了列は作業実績ではなく、完了した順に見たい
    done: tasks
      .filter((t) => t.status === 'done')
      .sort((a, b) =>
        (b.completed_at ?? b.folder_name).localeCompare(a.completed_at ?? a.folder_name)
      ),
  }));
</script>

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
            {onselect}
            {onpointerdown}
            {oncontext}
          />
        {:else}
          <p class="col-empty">
            {filtering ? '検索に一致する作業はありません' : col.empty}
          </p>
        {/each}
      </div>
    </section>
  {/each}
</div>

<style>
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
</style>
