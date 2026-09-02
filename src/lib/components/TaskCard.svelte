<script lang="ts">
  import type { Task } from '$lib/types';
  import { daysSince, fmtMinutes, fmtPrefix, isOverdue, relativeDays } from '$lib/format';

  let {
    task,
    selected = false,
    marked = false,
    hint = null,
    onselect,
    onopen,
    onpointerdown,
    oncontext,
  }: {
    task: Task;
    selected?: boolean;
    /** 一括操作の対象として選ばれている */
    marked?: boolean;
    /** 検索の一致箇所。カードに出ていない場所で当たったときだけ入る */
    hint?: string | null;
    onselect: (t: Task, e: MouseEvent) => void;
    onopen: (t: Task) => void;
    onpointerdown: (e: PointerEvent, t: Task) => void;
    oncontext: (e: MouseEvent, t: Task) => void;
  } = $props();

  const dateLabel = $derived(
    fmtPrefix(task.date_prefix) ??
      (task.created_at ? task.created_at.slice(0, 10).replaceAll('-', '.') : null)
  );
</script>

<button
  class="card status-{task.status}"
  data-task={task.path}
  class:on-hold={task.on_hold_since}
  class:selected
  class:marked
  onpointerdown={(e) => onpointerdown(e, task)}
  onclick={(e) => onselect(task, e)}
  ondblclick={() => onopen(task)}
  oncontextmenu={(e) => oncontext(e, task)}
  title="{task.folder_name}&#10;ダブルクリックでエクスプローラーを開く"
>
  <div class="top">
    {#if dateLabel}<span class="date mono">{dateLabel}</span>{/if}
    <span class="spacer"></span>
    {#if task.on_hold_since}
      <!-- 日数は「長く止まっている」に気づくためのもの。当日は数字を出さない -->
      {@const held = daysSince(task.on_hold_since) ?? 0}
      <span class="badge hold" title="保留中。放置の判定はされません">
        保留{held > 0 ? ` ${held}日` : ''}
      </span>
    {/if}
    {#if task.stale}
      <span class="badge stale">放置 {daysSince(task.last_activity)}日</span>
    {/if}
    {#if task.due && !task.archived}
      <span class="badge due" class:overdue={isOverdue(task.due)}>
        期限 {task.due.slice(5).replace('-', '/')}
      </span>
    {/if}
  </div>
  <div class="name">{task.name}</div>
  {#if task.progress !== null && !task.archived}
    <div class="progress-row">
      <div
        class="bar"
        role="progressbar"
        aria-valuenow={task.progress}
        aria-valuemin={0}
        aria-valuemax={100}
      >
        <div class="fill" style="width: {task.progress}%"></div>
      </div>
      <span class="p-label mono">{task.progress}%</span>
      {#if task.remaining_min !== null && task.remaining_min > 0}
        <span class="p-label remain">残り{fmtMinutes(task.remaining_min)}</span>
      {/if}
    </div>
  {/if}
  <div class="bottom">
    <span>{task.file_count} 項目</span>
    <span class="sep">·</span>
    <span>
      {task.archived ? `完了 ${relativeDays(task.completed_at)}` : `更新 ${relativeDays(task.last_activity)}`}
    </span>
    {#each task.tags.slice(0, 3) as tag (tag)}
      <span class="tag">{tag}</span>
    {/each}
  </div>
  <!-- 検索で当たった場所がカードに出ていないときだけ、その場所を伝える -->
  {#if hint}
    <div class="hit" title="この作業が検索に一致した場所">{hint} に一致</div>
  {/if}
</button>

<style>
  /* マニラフォルダのタブを模した左肩の出っ張りがカードの署名要素 */
  .card {
    position: relative;
    display: block;
    width: 100%;
    text-align: left;
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: 2px var(--radius) var(--radius) var(--radius);
    padding: 9px 12px 10px;
    box-shadow: var(--shadow);
    transition: box-shadow 0.12s ease, transform 0.12s ease, border-color 0.12s ease;
  }
  .card::before {
    content: '';
    position: absolute;
    top: -7px;
    left: -1px;
    width: 38px;
    height: 7px;
    border: 1px solid var(--line);
    border-bottom: none;
    border-radius: 4px 7px 0 0;
    background: var(--slate-soft);
  }
  .card.status-doing::before {
    background: var(--manila-tab);
  }
  .card.status-done::before {
    background: var(--green-soft);
  }
  /* 保留は「今は見なくていい」状態なので、白を落として背景側へ沈める。
     状態を表すタブの色には触れない */
  .card.on-hold {
    /* 影を消してあるぶん、塗りは弱くても十分沈む */
    background: var(--surface-sunk);
    box-shadow: none;
  }
  /* 沈めたぶん進捗のオレンジだけが浮くため、こちらも色を落とす。
     帯の長さは残るので、進み具合は読める */
  .card.on-hold .fill {
    background: var(--slate);
  }
  .card.on-hold .p-label {
    color: var(--ink-3);
  }
  .card:hover {
    box-shadow: var(--shadow-lift);
    transform: translateY(-1px);
  }
  .card.selected {
    border-color: var(--ink);
  }
  /* 一括操作の対象。詳細パネルの選択（枠が濃くなる）とは別の合図なので、
     枠ではなく地の色で示し、両方が同時に付いても読み分けられるようにする */
  .card.marked {
    background: var(--manila-soft);
    border-color: var(--manila-strong);
  }
  .card.marked::before {
    border-color: var(--manila-strong);
  }
  .card.selected::before {
    border-color: var(--ink);
    border-bottom: none;
  }
  .card:active {
    cursor: grabbing;
  }

  .top {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 3px;
    min-height: 16px;
  }
  .date {
    font-size: 11px;
    letter-spacing: 0.04em;
    color: var(--ink-2);
  }
  .spacer {
    flex: 1;
  }
  .badge {
    font-size: 10px;
    padding: 1px 6px;
    border-radius: 99px;
    white-space: nowrap;
  }
  .badge.stale {
    background: var(--red-soft);
    color: var(--red);
  }
  /* 放置とは違い、こちらは意図して止めている状態なので警告色にしない */
  .badge.hold {
    background: var(--slate-soft);
    color: var(--slate);
  }
  .badge.due {
    background: var(--slate-soft);
    color: var(--ink-2);
  }
  .badge.due.overdue {
    background: var(--red);
    color: var(--on-solid);
  }

  .name {
    font-size: 13.5px;
    font-weight: 600;
    line-height: 1.4;
    overflow: hidden;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    word-break: break-all;
  }

  .progress-row {
    display: flex;
    align-items: center;
    gap: 7px;
    margin-top: 7px;
  }
  .bar {
    flex: 1;
    height: 4px;
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
  .p-label {
    font-size: 10.5px;
    color: var(--ink-2);
    flex: none;
  }
  .p-label.remain {
    color: var(--ink-3);
  }

  .bottom {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 5px;
    margin-top: 6px;
    font-size: 11px;
    color: var(--ink-3);
  }
  .sep {
    color: var(--line-strong);
  }
  /* 絞り込み中だけ現れる行。作業そのものの情報ではないので控えめに置く */
  .hit {
    margin-top: 5px;
    padding-top: 5px;
    border-top: 1px dashed var(--line);
    font-size: 11px;
    color: var(--ink-2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tag {
    background: var(--surface-2);
    border: 1px solid var(--line);
    color: var(--ink-2);
    padding: 0 6px;
    border-radius: 4px;
  }

  /* 手が動いていない作業は名前も弱める。名前が一番太く大きいので、
     ここを落とさないとカードだけ沈めても視線を引き続ける */
  .card.status-done .name {
    color: var(--ink-2);
    font-weight: 500;
  }
  /* 保留は完了より弱くてよい。完了は結果として残るが、保留は
     見なくていい状態そのものなので、名前も日付も一段落とす */
  .card.on-hold .name {
    color: var(--ink-3);
    font-weight: 500;
  }
  .card.on-hold .date {
    color: var(--ink-3);
  }
</style>
