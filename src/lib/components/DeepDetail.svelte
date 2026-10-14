<script lang="ts">
  import type { FolderEntry, FolderListing, Task } from '$lib/types';
  import { fmtDate } from '$lib/format';

  let {
    task,
    listing,
    onclose,
    onopen,
    oncopy,
    onrestore,
    onopenentry,
    onopenlink,
  }: {
    task: Task;
    listing: FolderListing;
    onclose: () => void;
    onopen: (t: Task) => void;
    oncopy: (t: Task) => void;
    onrestore: (t: Task) => void;
    onopenentry: (t: Task, e: FolderEntry) => void;
    onopenlink: (url: string) => void;
  } = $props();
</script>

<!-- ディープアーカイブの作業を読むためのペイン。管理の外にあるので書き換えはさせない。
     直したいときは作業に戻してから、普段の詳細パネルで扱う -->
<aside class="panel">
  <header>
    <div class="path-line">
      <span class="mono path" title={task.path}>{task.path}</span>
      <button class="icon-btn" onclick={onclose} title="閉じる" aria-label="閉じる">✕</button>
    </div>
    <h2 class="name">{task.name}</h2>
    <p class="badge">ディープアーカイブ・読み取り専用</p>
    <div class="actions">
      <button class="btn" onclick={() => onopen(task)}>エクスプローラーで開く</button>
      <button class="btn" onclick={() => oncopy(task)}>パスをコピー</button>
      <button class="btn" onclick={() => onrestore(task)}>作業に戻す</button>
    </div>
    <p class="note">書き換えるには、作業に戻してください</p>
  </header>

  <section class="rows">
    {#if task.completed_at}
      <div class="row"><span class="label">完了日</span><span>{fmtDate(task.completed_at)}</span></div>
    {/if}
    {#if task.tags.length}
      <div class="row">
        <span class="label">タグ</span>
        <span class="chips">{#each task.tags as tag (tag)}<span class="chip">{tag}</span>{/each}</span>
      </div>
    {/if}
    {#if task.due}
      <div class="row"><span class="label">期限</span><span class="mono">{task.due}</span></div>
    {/if}
    <div class="row block">
      <span class="label">補足</span>
      {#if task.memo}<p class="memo">{task.memo}</p>{:else}<span class="dim">—</span>{/if}
    </div>
  </section>

  {#if task.links.length}
    <section>
      <h3>参照 <span class="dim">{task.links.length}</span></h3>
      <ul class="plain">
        {#each task.links as link (link.url)}
          <li>
            <button class="link" onclick={() => onopenlink(link.url)} title={link.url}>
              {link.label || link.url}
            </button>
          </li>
        {/each}
      </ul>
    </section>
  {/if}

  {#if task.checklist.length}
    <section>
      <h3>やること <span class="dim">{task.checklist.filter((i) => i.done).length}/{task.checklist.length}</span></h3>
      <ul class="plain">
        {#each task.checklist as item, i (i)}
          <li class:done={item.done}><span class="mark">{item.done ? '✓' : '・'}</span>{item.text}</li>
        {/each}
      </ul>
    </section>
  {/if}

  <section>
    <h3>フォルダの中身 <span class="dim">{listing.entries.length} 項目</span></h3>
    <ul class="plain">
      {#each listing.entries as entry (entry.rel)}
        <li>
          <button class="entry" onclick={() => onopenentry(task, entry)} title={entry.rel}>
            {#if entry.icon}<img src={entry.icon} alt="" width="16" height="16" />{:else}<span class="ph">{entry.is_dir ? '📁' : '·'}</span>{/if}
            <span class="rel mono">{entry.rel}</span>
          </button>
        </li>
      {/each}
    </ul>
  </section>
</aside>

<style>
  .panel {
    /* 幅は親が持つ。左端の仕切りで変えられ、ボード側には最低限の幅を残す */
    width: var(--panel-width, 340px);
    max-width: calc(100% - 360px);
    flex: none;
    background: var(--surface);
    border-left: 1px solid var(--line);
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    font-size: 12.5px;
  }
  header,
  section {
    padding: 12px 16px;
    border-bottom: 1px solid var(--line);
  }
  .path-line {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  /* 頭から詰めると共通の親だけが残るので、末尾を見せる */
  .path {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    direction: rtl;
    text-align: left;
    font-size: 11px;
    color: var(--ink-3);
  }
  .icon-btn {
    border: none;
    background: none;
    color: var(--ink-3);
    cursor: pointer;
  }
  .icon-btn:hover {
    color: var(--ink);
  }
  .name {
    margin: 6px 0 2px;
    font-size: 15px;
  }
  .badge {
    margin: 0 0 8px;
    font-size: 11px;
    color: var(--ink-3);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .note {
    margin: 6px 0 0;
    font-size: 11px;
    color: var(--ink-3);
  }
  .row {
    display: flex;
    gap: 12px;
    padding: 3px 0;
  }
  .row.block {
    flex-direction: column;
    gap: 4px;
  }
  .label {
    flex: none;
    width: 4.5em;
    color: var(--ink-2);
  }
  .memo {
    margin: 0;
    white-space: pre-wrap;
    color: var(--ink);
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .chip {
    padding: 0 8px;
    border-radius: 99px;
    background: var(--manila-soft);
    color: var(--manila-ink);
    font-size: 11.5px;
  }
  h3 {
    margin: 0 0 6px;
    font-size: 12px;
    font-weight: 600;
    color: var(--ink-2);
  }
  .dim {
    color: var(--ink-3);
    font-weight: 400;
  }
  .plain {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .plain li {
    padding: 2px 0;
  }
  .plain li.done {
    color: var(--ink-3);
    text-decoration: line-through;
  }
  .mark {
    display: inline-block;
    width: 1.2em;
    color: var(--ink-3);
  }
  .link,
  .entry {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    border: none;
    background: none;
    padding: 2px 4px;
    border-radius: 4px;
    color: var(--ink);
    font: inherit;
    text-align: left;
    cursor: pointer;
    min-width: 0;
  }
  .link {
    color: var(--accent);
  }
  .link:hover,
  .entry:hover {
    background: var(--surface-2);
  }
  .rel {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 11.5px;
  }
  .ph {
    width: 16px;
    text-align: center;
    color: var(--ink-3);
  }
</style>
