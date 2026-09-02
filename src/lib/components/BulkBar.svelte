<script lang="ts">
  import type { Task } from '$lib/types';

  let {
    marked,
    oncomplete,
    onreopen,
    ondeep,
    onaddtag,
    onremovetag,
    onclear,
  }: {
    marked: Task[];
    oncomplete: () => void;
    onreopen: () => void;
    ondeep: () => void;
    onaddtag: (tag: string) => void;
    onremovetag: (tag: string) => void;
    onclear: () => void;
  } = $props();

  let tagInput = $state('');

  // 移動系はどちらかの側にあるものにしか効かない。押せるかどうかではなく
  // 何件に効くかを出すので、混ざった選択でも数を見て判断できる
  const inWork = $derived(marked.filter((t) => !t.archived).length);
  const inArchive = $derived(marked.filter((t) => t.archived).length);

  /** 全部に付いているタグだけ外せる。一部にしか無いものを外すと、選んだ覚えのない変化になる */
  const commonTags = $derived(
    marked.length === 0
      ? []
      : marked
          .map((t) => t.tags)
          .reduce((acc, tags) => acc.filter((tag) => tags.includes(tag)))
          .slice()
          .sort()
  );

  function submitTag(e: KeyboardEvent) {
    if (e.key !== 'Enter') return;
    const tag = tagInput.trim();
    if (!tag) return;
    onaddtag(tag);
    tagInput = '';
  }
</script>

<div class="bulkbar">
  <span class="count">{marked.length} 件を選択中</span>

  <button class="btn" disabled={inWork === 0} onclick={oncomplete}>
    完了してアーカイブ{#if inWork !== marked.length}<span class="sub">（{inWork}）</span>{/if}
  </button>
  <button class="btn" disabled={inArchive === 0} onclick={onreopen}>
    作業に戻す{#if inArchive !== marked.length}<span class="sub">（{inArchive}）</span>{/if}
  </button>
  <button class="btn" disabled={inArchive === 0} onclick={ondeep}>
    ディープアーカイブへ{#if inArchive !== marked.length}<span class="sub">（{inArchive}）</span>{/if}
  </button>

  <span class="sep"></span>

  <div class="tags">
    {#each commonTags as tag (tag)}
      <span class="chip">
        {tag}
        <button class="chip-x" onclick={() => onremovetag(tag)} aria-label="{tag} を全件から外す">
          ✕
        </button>
      </span>
    {/each}
    <input
      class="tag-input"
      bind:value={tagInput}
      placeholder="タグを追加（Enter）"
      onkeydown={submitTag}
      aria-label="選択した全件にタグを追加"
    />
  </div>

  <span class="spacer"></span>
  <button class="btn ghost" onclick={onclear}>選択解除<span class="key">Esc</span></button>
</div>

<style>
  .bulkbar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 16px;
    background: var(--manila-soft);
    border-bottom: 1px solid var(--line);
    flex-wrap: wrap;
  }
  .count {
    font-weight: 600;
    color: var(--manila-ink);
    white-space: nowrap;
  }
  .btn {
    padding: 5px 11px;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius);
    background: var(--surface);
    color: var(--ink);
    font: inherit;
    cursor: pointer;
    white-space: nowrap;
  }
  .btn:hover:not(:disabled) {
    background: var(--ink-hover);
  }
  .btn:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .btn.ghost {
    border-color: transparent;
    background: none;
    color: var(--ink-2);
  }
  .sub {
    color: var(--ink-3);
  }
  .key {
    margin-left: 6px;
    font-family: var(--mono);
    font-size: 0.85em;
    color: var(--ink-3);
  }
  .sep {
    width: 1px;
    align-self: stretch;
    background: var(--line);
  }
  .tags {
    display: flex;
    align-items: center;
    gap: 4px;
    flex-wrap: wrap;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    padding: 2px 4px 2px 8px;
    border-radius: 999px;
    background: var(--manila);
    color: var(--manila-ink);
    font-size: 0.85rem;
  }
  .chip-x {
    border: none;
    background: none;
    color: inherit;
    cursor: pointer;
    padding: 0 2px;
    font-size: 0.8em;
    opacity: 0.7;
  }
  .chip-x:hover {
    opacity: 1;
  }
  .tag-input {
    width: 12ch;
    padding: 4px 8px;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius);
    background: var(--surface);
    color: var(--ink);
    font: inherit;
    font-size: 0.9rem;
  }
  .tag-input::placeholder {
    color: var(--ink-3);
  }
  .spacer {
    flex: 1;
  }
</style>
