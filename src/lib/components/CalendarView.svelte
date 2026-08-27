<script lang="ts">
  import type { CalendarViewState, Task } from '$lib/types';

  let {
    tasks,
    selectedPath,
    state: view,
    onselect,
  }: {
    tasks: Task[];
    selectedPath: string | null;
    /** 表示月と凡例は親が持つ。ビューを切り替えても見ていた月に戻れる */
    state: CalendarViewState;
    onselect: (t: Task) => void;
  } = $props();

  const today = new Date();

  function pad(n: number): string {
    return String(n).padStart(2, '0');
  }
  function keyOf(d: Date): string {
    return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
  }
  const todayKey = keyOf(today);

  function move(delta: number) {
    const d = new Date(view.year, view.month + delta, 1);
    view.year = d.getFullYear();
    view.month = d.getMonth();
  }
  function goToday() {
    view.year = today.getFullYear();
    view.month = today.getMonth();
  }

  // 日曜始まり6週間(42マス)
  const cells = $derived.by(() => {
    const first = new Date(view.year, view.month, 1);
    const start = new Date(view.year, view.month, 1 - first.getDay());
    return Array.from({ length: 42 }, (_, i) => {
      const d = new Date(start.getFullYear(), start.getMonth(), start.getDate() + i);
      return {
        key: keyOf(d),
        day: d.getDate(),
        inMonth: d.getMonth() === view.month,
        dow: d.getDay(),
      };
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
      if (view.showStart && t.date_prefix && t.date_prefix.length === 8) {
        add(
          `${t.date_prefix.slice(0, 4)}-${t.date_prefix.slice(4, 6)}-${t.date_prefix.slice(6, 8)}`,
          t,
          'start'
        );
      }
      if (view.showDue && t.due && !t.archived) add(t.due, t, 'due');
      if (view.showDone && t.completed_at) add(t.completed_at.slice(0, 10), t, 'done');
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

  // 入り切らない日は、そのマスだけを他のマスの上へ広げて全件出す。
  // マス自体を高くすると週の高さが揃わなくなるため、重ねて表示する
  let expandedDay = $state<string | null>(null);

  // 広げているマスの外を触ったときだけ畳む。要素ごとに伝播を止める作りだと
  // 余白や日付の数字など、止め忘れた場所を押したときに畳まれてしまう
  function closeIfOutside(e: PointerEvent) {
    if (!expandedDay) return;
    if (e.target instanceof Element && e.target.closest('.cell.expanded')) return;
    expandedDay = null;
  }
</script>

<svelte:window
  onpointerdown={closeIfOutside}
  onwheel={() => (expandedDay = null)}
  onresize={() => (expandedDay = null)}
/>

<div class="cal">
  <div class="cal-head">
    <div class="nav">
      <button class="btn nav-btn" onclick={() => move(-1)} aria-label="前の月">‹</button>
      <span class="ym">{view.year}年{view.month + 1}月</span>
      <button class="btn nav-btn" onclick={() => move(1)} aria-label="次の月">›</button>
      <button class="btn today-btn" onclick={goToday}>今日</button>
    </div>
    <div class="legend">
      <label><input type="checkbox" bind:checked={view.showDue} /><span class="dot due"></span
        >期限</label
      >
      <label
        ><input type="checkbox" bind:checked={view.showDone} /><span class="dot done"></span
        >完了</label
      >
      <label
        ><input type="checkbox" bind:checked={view.showStart} /><span class="dot start"></span
        >開始</label
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
      {@const open = expandedDay === cell.key}
      <div
        class="cell"
        class:out={!cell.inMonth}
        class:today={cell.key === todayKey}
        class:expanded={open}
      >
        <!-- 広げたときだけ、この箱が他のマスの上へせり出す -->
        <div class="stack">
          <span class="daynum" class:sun={cell.dow === 0} class:sat={cell.dow === 6}>
            {cell.day}
          </span>
          {#each open ? entries : entries.slice(0, MAX_CHIPS) as e (e.task.path + e.kind)}
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
            <button
              class="more"
              onclick={() => (expandedDay = open ? null : cell.key)}
              onkeydown={(e) => {
                // 広げた直後はこのボタンに焦点があるので、Esc をここで受けて
                // 画面全体の Esc（絞り込み解除やトレイ格納）まで伝えない
                if (open && e.key === 'Escape') {
                  e.stopPropagation();
                  expandedDay = null;
                }
              }}
            >
              {open ? '閉じる' : `他 ${entries.length - MAX_CHIPS} 件`}
            </button>
          {/if}
        </div>
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
    min-height: 84px;
    overflow: hidden;
  }
  .stack {
    height: 100%;
    padding: 5px 6px;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  /* 広げている間は、中身がマスの外へ出られるようにする。
     マス自体を高くすると週の高さが崩れるため、中身だけを重ねて伸ばす */
  .cell.expanded {
    overflow: visible;
    position: relative;
    z-index: 3;
  }
  .cell.expanded .stack {
    position: absolute;
    inset: -1px -1px auto;
    height: auto;
    background: var(--surface);
    border: 1px solid var(--line-strong);
    border-radius: 8px;
    box-shadow: var(--shadow-lift);
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
    color: var(--on-solid);
    box-shadow: var(--shadow);
  }
  .chip.selected .chip-name {
    color: var(--on-solid);
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
    padding: 1px 2px;
    border: 0;
    border-radius: 4px;
    background: none;
    text-align: left;
    cursor: pointer;
  }
  .more:hover {
    color: var(--ink-2);
    background: var(--surface-2);
  }
</style>

