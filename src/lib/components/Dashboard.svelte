<script lang="ts">
  import type { Task } from '$lib/types';
  import { daysSince, fmtMinutes, relativeDays } from '$lib/format';

  let {
    tasks,
    deepMonths,
    hasDeepRoot,
    onselect,
    ondeepsweep,
  }: {
    tasks: Task[];
    deepMonths: number;
    hasDeepRoot: boolean;
    onselect: (t: Task) => void;
    ondeepsweep: (candidates: Task[]) => void;
  } = $props();

  const active = $derived(tasks.filter((t) => !t.archived));
  const doingCount = $derived(active.filter((t) => t.status === 'doing').length);
  const backlogCount = $derived(active.filter((t) => t.status === 'backlog').length);

  function monthKey(iso: string): string {
    return iso.slice(0, 7); // YYYY-MM
  }

  const thisMonth = $derived(new Date().toISOString().slice(0, 7));
  const doneThisMonth = $derived(
    tasks.filter((t) => t.archived && t.completed_at && monthKey(t.completed_at) === thisMonth)
      .length
  );
  const remainingTotal = $derived(
    active.reduce((sum, t) => sum + (t.remaining_min ?? 0), 0)
  );

  const staleList = $derived(
    active
      .filter((t) => t.stale)
      .sort((a, b) => (a.last_activity ?? '').localeCompare(b.last_activity ?? ''))
      .slice(0, 6)
  );

  const dueList = $derived.by(() => {
    const limit = new Date();
    limit.setDate(limit.getDate() + 7);
    const limitStr = limit.toISOString().slice(0, 10);
    return active
      .filter((t) => t.due && t.due <= limitStr)
      .sort((a, b) => (a.due ?? '').localeCompare(b.due ?? ''))
      .slice(0, 6);
  });

  // 直近6か月の完了数（アーカイブの completed_at ベース）
  const monthly = $derived.by(() => {
    const months: { key: string; label: string; count: number }[] = [];
    const now = new Date();
    for (let i = 5; i >= 0; i--) {
      const d = new Date(now.getFullYear(), now.getMonth() - i, 1);
      const key = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}`;
      months.push({ key, label: `${d.getMonth() + 1}月`, count: 0 });
    }
    for (const t of tasks) {
      if (!t.archived || !t.completed_at) continue;
      const m = months.find((x) => x.key === monthKey(t.completed_at!));
      if (m) m.count++;
    }
    return months;
  });
  const monthlyMax = $derived(Math.max(1, ...monthly.map((m) => m.count)));

  let hoverMonth = $state<string | null>(null);

  const today = new Date().toISOString().slice(0, 10);

  // ディープアーカイブ整理の対象: 完了(なければ最終更新)から deepMonths か月超
  const deepCandidates = $derived.by(() => {
    const cutoff = new Date();
    cutoff.setMonth(cutoff.getMonth() - (deepMonths || 6));
    const cutoffMs = cutoff.getTime();
    return tasks.filter((t) => {
      if (!t.archived) return false;
      const basis = t.completed_at ?? t.last_activity;
      if (!basis) return false;
      const ms = new Date(basis).getTime();
      return !isNaN(ms) && ms < cutoffMs;
    });
  });
</script>

<div class="dash">
  <div class="tiles">
    <div class="tile">
      <span class="tile-num">{doingCount}</span>
      <span class="tile-label">進行中</span>
    </div>
    <div class="tile">
      <span class="tile-num">{backlogCount}</span>
      <span class="tile-label">未着手</span>
    </div>
    <div class="tile">
      <span class="tile-num">{doneThisMonth}</span>
      <span class="tile-label">今月の完了</span>
    </div>
    <div class="tile">
      <span class="tile-num">{remainingTotal > 0 ? fmtMinutes(remainingTotal) : '—'}</span>
      <span class="tile-label">残り見積の合計</span>
    </div>
  </div>

  <div class="cards">
    <div class="card chart-card">
      <h3>月別の完了数（直近6か月）</h3>
      <div class="chart" role="img" aria-label="月別完了数の棒グラフ">
        {#each monthly as m (m.key)}
          <div
            class="col"
            role="presentation"
            onpointerenter={() => (hoverMonth = m.key)}
            onpointerleave={() => (hoverMonth = null)}
          >
            {#if hoverMonth === m.key}
              <span class="tip">{m.count}件</span>
            {/if}
            <div class="bar-area">
              <div
                class="vbar"
                class:zero={m.count === 0}
                style="height: {Math.round((m.count / monthlyMax) * 100)}%"
              ></div>
            </div>
            <span class="x-label">{m.label}</span>
          </div>
        {/each}
      </div>
    </div>

    <div class="card">
      <h3>期限が近い・超過</h3>
      {#if dueList.length === 0}
        <p class="empty">7日以内に期限を迎える作業はありません</p>
      {:else}
        <ul>
          {#each dueList as t (t.path)}
            <li>
              <button class="item" onclick={() => onselect(t)}>
                <span class="item-name">{t.name}</span>
                <span class="item-meta mono" class:danger={(t.due ?? '') < today}>
                  {t.due?.replaceAll('-', '/').slice(5)}
                  {(t.due ?? '') < today ? '超過' : ''}
                </span>
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </div>

    <div class="card">
      <h3>アーカイブ整理</h3>
      {#if !hasDeepRoot}
        <p class="empty">
          設定でディープアーカイブ先を指定すると、古い完了タスクを表示・検索対象外の場所へ退避できます
        </p>
      {:else if deepCandidates.length === 0}
        <p class="empty">完了から{deepMonths}か月を超えたタスクはありません</p>
      {:else}
        <p class="sweep-info">
          完了から{deepMonths}か月超のタスクが <strong>{deepCandidates.length} 件</strong> あります
        </p>
        <button class="btn" onclick={() => ondeepsweep(deepCandidates)}>
          ディープアーカイブへ整理する
        </button>
      {/if}
    </div>

    <div class="card">
      <h3>放置されている作業</h3>
      {#if staleList.length === 0}
        <p class="empty">放置中の作業はありません</p>
      {:else}
        <ul>
          {#each staleList as t (t.path)}
            <li>
              <button class="item" onclick={() => onselect(t)}>
                <span class="item-name">{t.name}</span>
                <span class="item-meta">
                  {daysSince(t.last_activity)}日 ({relativeDays(t.last_activity)})
                </span>
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  </div>
</div>

<style>
  .dash {
    flex: 1;
    overflow-y: auto;
    padding: 16px;
    min-width: 0;
  }

  .tiles {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 12px;
    margin-bottom: 14px;
  }
  .tile {
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: 12px;
    padding: 14px 16px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .tile-num {
    font-size: 26px;
    font-weight: 700;
    letter-spacing: 0.01em;
  }
  .tile-label {
    font-size: 11.5px;
    color: var(--ink-2);
  }

  .cards {
    display: grid;
    grid-template-columns: 1.3fr 1fr 1fr;
    gap: 12px;
    align-items: start;
  }
  .card {
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: 12px;
    padding: 14px 16px;
  }
  .card h3 {
    margin: 0 0 10px;
    font-size: 12px;
    font-weight: 600;
    color: var(--ink-2);
  }

  /* 単一系列の棒グラフ: 単色マニラ、上端4px丸め、ベースライン付き */
  .chart {
    display: flex;
    align-items: stretch;
    gap: 10px;
    height: 150px;
  }
  .col {
    flex: 1;
    display: flex;
    flex-direction: column;
    position: relative;
    min-width: 0;
  }
  .bar-area {
    flex: 1;
    display: flex;
    align-items: flex-end;
    justify-content: center;
    border-bottom: 1px solid var(--line-strong);
  }
  .vbar {
    width: 55%;
    max-width: 34px;
    background: var(--manila);
    border-radius: 4px 4px 0 0;
    min-height: 2px;
    transition: height 0.2s ease;
  }
  .vbar.zero {
    background: var(--slate-soft);
  }
  .col:hover .vbar:not(.zero) {
    background: #b87a17;
  }
  .x-label {
    text-align: center;
    font-size: 10.5px;
    color: var(--ink-3);
    padding-top: 5px;
  }
  .tip {
    position: absolute;
    top: -4px;
    left: 50%;
    transform: translateX(-50%);
    background: var(--ink);
    color: #fff;
    font-size: 10.5px;
    padding: 2px 8px;
    border-radius: 6px;
    white-space: nowrap;
    z-index: 2;
    pointer-events: none;
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  li + li {
    border-top: 1px solid var(--surface-2);
  }
  .item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    width: 100%;
    border: none;
    background: none;
    padding: 7px 2px;
    text-align: left;
    font-size: 12.5px;
    border-radius: 6px;
  }
  .item:hover {
    background: var(--surface-2);
  }
  .item-name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 600;
  }
  .item-meta {
    flex: none;
    font-size: 11px;
    color: var(--ink-2);
  }
  .item-meta.danger {
    color: var(--red);
    font-weight: 700;
  }
  .empty {
    color: var(--ink-3);
    font-size: 12px;
    margin: 4px 0;
  }
  .sweep-info {
    font-size: 12.5px;
    color: var(--ink-2);
    margin: 4px 0 10px;
  }
  .sweep-info strong {
    color: var(--ink);
  }

  @media (max-width: 1050px) {
    .cards {
      grid-template-columns: 1fr;
    }
    .tiles {
      grid-template-columns: repeat(2, 1fr);
    }
  }
</style>
