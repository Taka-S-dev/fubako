<script lang="ts">
  import type { Task } from '$lib/types';

  let {
    tasks,
    selectedPath,
    onselect,
  }: {
    tasks: Task[];
    selectedPath: string | null;
    onselect: (t: Task) => void;
  } = $props();

  const today = new Date();
  let year = $state(today.getFullYear());
  let month = $state(today.getMonth()); // 0-11

  let showStart = $state(true);
  let showDue = $state(true);
  let showDone = $state(true);

  function pad(n: number): string {
    return String(n).padStart(2, '0');
  }
  function keyOf(d: Date): string {
    return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
  }
  const todayKey = keyOf(today);

  function move(delta: number) {
    const d = new Date(year, month + delta, 1);
    year = d.getFullYear();
    month = d.getMonth();
  }
  function goToday() {
    year = today.getFullYear();
    month = today.getMonth();
  }

  // 日曜始まり6週間(42マス)
  const cells = $derived.by(() => {
    const first = new Date(year, month, 1);
    const start = new Date(year, month, 1 - first.getDay());
    return Array.from({ length: 42 }, (_, i) => {
      const d = new Date(start.getFullYear(), start.getMonth(), start.getDate() + i);
      return { key: keyOf(d), day: d.getDate(), inMonth: d.getMonth() === month, dow: d.getDay() };
    });
  });

  type Entry = { task: Task; kind: 'start' | 'due' | 'done' };

  const entriesByDay = $derived.by(() => {
    const map = new Map<string, Entry[]>();
    const add = (key: string | null, task: Task, kind: Entry['kind']) => {
      if (!key) return;
      if (!map.has(key)) map.set(key, []);
      map.get(key)!.push({ task, kind });
    };
    for (const t of tasks) {
      if (showStart && t.date_prefix && t.date_prefix.length === 8) {
        add(
          `${t.date_prefix.slice(0, 4)}-${t.date_prefix.slice(4, 6)}-${t.date_prefix.slice(6, 8)}`,
          t,
          'start'
        );
      }
      if (showDue && t.due && !t.archived) add(t.due, t, 'due');
      if (showDone && t.completed_at) add(t.completed_at.slice(0, 10), t, 'done');
    }
    // 期限 > 完了 > 開始 の順に表示
    const order = { due: 0, done: 1, start: 2 };
    for (const list of map.values()) {
      list.sort((a, b) => order[a.kind] - order[b.kind]);
    }
    return map;
  });

  const MAX_CHIPS = 3;

  const kindLabel = { start: '開始', due: '期限', done: '完了' };
</script>

<div class="cal">
  <div class="cal-head">
    <div class="nav">
      <button class="btn nav-btn" onclick={() => move(-1)} aria-label="前の月">‹</button>
      <span class="ym">{year}年{month + 1}月</span>
      <button class="btn nav-btn" onclick={() => move(1)} aria-label="次の月">›</button>
      <button class="btn today-btn" onclick={goToday}>今日</button>
    </div>
    <div class="legend">
      <label><input type="checkbox" bind:checked={showDue} /><span class="dot due"></span>期限</label>
      <label
        ><input type="checkbox" bind:checked={showDone} /><span class="dot done"></span>完了</label
      >
      <label
        ><input type="checkbox" bind:checked={showStart} /><span class="dot start"></span>開始</label
      >
    </div>
  </div>

  <div class="grid-head">
    {#each ['日', '月', '火', '水', '木', '金', '土'] as w, i (w)}
      <div class="dow" class:sun={i === 0} class:sat={i === 6}>{w}</div>
    {/each}
  </div>

  <div class="grid">
    {#each cells as cell (cell.key)}
      {@const entries = entriesByDay.get(cell.key) ?? []}
      <div
        class="cell"
        class:out={!cell.inMonth}
        class:today={cell.key === todayKey}
      >
        <span class="daynum" class:sun={cell.dow === 0} class:sat={cell.dow === 6}>
          {cell.day}
        </span>
        {#each entries.slice(0, MAX_CHIPS) as e (e.task.path + e.kind)}
          <button
            class="chip {e.kind}"
            class:selected={e.task.path === selectedPath}
            class:overdue={e.kind === 'due' && cell.key < todayKey}
            title="{kindLabel[e.kind]}: {e.task.name}"
            onclick={() => onselect(e.task)}
          >
            <span class="chip-dot"></span>
            <span class="chip-name">{e.task.name}</span>
          </button>
        {/each}
        {#if entries.length > MAX_CHIPS}
          <span class="more">+{entries.length - MAX_CHIPS}</span>
        {/if}
      </div>
    {/each}
  </div>
</div>

<style>
  .cal {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    padding: 12px 16px 16px;
  }

  .cal-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 10px;
    flex: none;
  }
  .nav {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .ym {
    font-size: 15px;
    font-weight: 700;
    min-width: 110px;
    text-align: center;
  }
  .nav-btn {
    padding: 3px 11px;
    font-size: 15px;
    line-height: 1.4;
  }
  .today-btn {
    padding: 5px 12px;
    font-size: 12px;
    margin-left: 4px;
  }
  .legend {
    display: flex;
    gap: 14px;
  }
  .legend label {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 12px;
    color: var(--ink-2);
    cursor: pointer;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 2px;
  }
  .dot.due {
    background: var(--manila);
  }
  .dot.done {
    background: var(--green);
  }
  .dot.start {
    background: var(--slate);
  }

  .grid-head {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    flex: none;
  }
  .dow {
    text-align: center;
    font-size: 11px;
    color: var(--ink-2);
    padding: 4px 0;
  }
  .dow.sun {
    color: var(--red);
  }
  .dow.sat {
    color: var(--focus);
  }

  .grid {
    flex: 1;
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    grid-template-rows: repeat(6, 1fr);
    gap: 4px;
    min-height: 0;
    overflow-y: auto;
  }
  .cell {
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: 8px;
    padding: 5px 6px;
    min-height: 84px;
    display: flex;
    flex-direction: column;
    gap: 3px;
    overflow: hidden;
  }
  .cell.out {
    background: transparent;
    border-color: var(--surface-2);
  }
  .cell.out .daynum {
    color: var(--ink-3);
    opacity: 0.6;
  }
  .cell.today {
    border-color: var(--manila);
    box-shadow: inset 0 0 0 1px var(--manila);
  }
  .daynum {
    font-size: 11px;
    font-weight: 600;
    color: var(--ink-2);
    flex: none;
  }
  .cell.today .daynum {
    color: var(--manila);
  }
  .daynum.sun {
    color: var(--red);
  }
  .daynum.sat {
    color: var(--focus);
  }

  .chip {
    display: flex;
    align-items: center;
    gap: 5px;
    border: none;
    background: var(--surface-2);
    border-radius: 5px;
    padding: 2px 6px;
    font-size: 11px;
    text-align: left;
    width: 100%;
    color: var(--ink);
  }
  .chip:hover {
    background: var(--slate-soft);
  }
  .chip.selected,
  .chip.selected:hover {
    background: var(--ink);
    color: #fff;
    box-shadow: var(--shadow);
  }
  .chip.selected .chip-name {
    color: #fff;
    font-weight: 600;
  }
  .chip-dot {
    width: 6px;
    height: 6px;
    border-radius: 2px;
    flex: none;
    background: var(--slate);
  }
  .chip.due .chip-dot {
    background: var(--manila);
  }
  .chip.done .chip-dot {
    background: var(--green);
  }
  .chip.done .chip-name {
    color: var(--ink-2);
  }
  .chip.overdue .chip-dot {
    background: var(--red);
  }
  .chip.overdue {
    background: var(--red-soft);
  }
  .chip-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .more {
    font-size: 10px;
    color: var(--ink-3);
    padding-left: 2px;
  }
</style>

