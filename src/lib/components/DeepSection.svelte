<script lang="ts">
  import type { DeepView } from '$lib/deep';
  import { matchHint } from '$lib/search';
  import { fmtPrefix } from '$lib/format';

  let { deep }: { deep: DeepView } = $props();
</script>

<!-- 完了列の末尾に置く。普段のカードとは見た目を変え、ドラッグも選択もさせない
     （管理の外にあるので、触っても効かない操作を出さない） -->
<div class="deep">
  <div class="head">
    <span class="title">ディープアーカイブ</span>
    <span class="note">
      {#if deep.loading}探しています…{:else}{deep.tasks.length} 件{#if deep.capped}・上限で打ち切り{/if}{/if}
    </span>
    <button class="close" onclick={deep.onclose} aria-label="ディープアーカイブの結果を閉じる">✕</button>
  </div>
  {#if !deep.loading}
    {#each deep.tasks as t (t.path)}
      {@const hint = matchHint(t, deep.terms)}
      <!-- 行全体で右のペインを開く。中のボタンはそれぞれの操作だけをする -->
      <div
        class="item"
        class:selected={deep.selectedPath === t.path}
        title={t.path}
        role="button"
        tabindex="0"
        onclick={() => deep.onselect(t)}
        onkeydown={(e) => e.key === 'Enter' && deep.onselect(t)}
      >
        <div class="line">
          <span class="date mono">{fmtPrefix(t.date_prefix) ?? ''}</span>
          <span class="name">{t.name}</span>
        </div>
        <div class="line sub">
          {#if hint}<span class="hint">{hint} に一致</span>{/if}
          <span class="spacer"></span>
          <button class="act" onclick={(e) => { e.stopPropagation(); deep.onopen(t); }}>開く</button>
          <button class="act" onclick={(e) => { e.stopPropagation(); deep.onrestore(t); }}>
            作業に戻す
          </button>
        </div>
      </div>
    {:else}
      <p class="empty">一致する作業はありません</p>
    {/each}
    {#if deep.capped}
      <p class="empty">語を足して絞ると、打ち切られた分も探せます</p>
    {/if}
  {/if}
</div>

<style>
  .deep {
    margin-top: 6px;
    border-top: 1px dashed var(--line-strong);
    padding-top: 6px;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 2px 4px;
    font-size: 11px;
  }
  .title {
    color: var(--ink-2);
    font-weight: 600;
  }
  .note {
    color: var(--ink-3);
  }
  .close {
    margin-left: auto;
    border: none;
    background: none;
    color: var(--ink-3);
    cursor: pointer;
    padding: 0 2px;
  }
  .close:hover {
    color: var(--ink);
  }
  /* 普段のカードより一段沈める。管理の外にあることが見た目で分かるように */
  .item {
    margin-bottom: 6px;
    padding: 6px 10px;
    border: 1px dashed var(--line-strong);
    border-radius: 8px;
    background: var(--surface-sunk);
    cursor: pointer;
  }
  .item:hover {
    border-color: var(--ink-3);
  }
  .item.selected {
    border-style: solid;
    border-color: var(--ink-2);
  }
  .line {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .date {
    flex: none;
    color: var(--ink-3);
    font-size: 11px;
  }
  .name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--ink-2);
    font-size: 12.5px;
  }
  .sub {
    margin-top: 2px;
  }
  .hint {
    color: var(--ink-3);
    font-size: 11px;
  }
  .spacer {
    flex: 1;
  }
  .act {
    flex: none;
    border: none;
    background: none;
    color: var(--ink-2);
    font: inherit;
    font-size: 11px;
    padding: 1px 6px;
    border-radius: 4px;
    cursor: pointer;
  }
  .act:hover {
    background: var(--surface-2);
    color: var(--ink);
  }
  .empty {
    margin: 2px 2px 6px;
    color: var(--ink-3);
    font-size: 11px;
  }
</style>
