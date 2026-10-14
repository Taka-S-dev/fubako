<script lang="ts">
  import { onMount } from 'svelte';

  let {
    value = $bindable(),
    wrap = $bindable(),
    title,
    dirty,
    onsave,
    onclose,
  }: {
    /** 詳細パネルの補足そのもの。閉じても書いた内容はパネルに残る */
    value: string;
    /** 折り返すかどうか。パネルの欄と共通の設定 */
    wrap: boolean;
    title: string;
    /** 保存していない変更がある */
    dirty: boolean;
    onsave: () => void;
    onclose: () => void;
  } = $props();

  let area: HTMLTextAreaElement | undefined = $state();
  onMount(() => area?.focus());

  // 画面全体の Esc（絞り込み解除やトレイ格納）まで伝えない。この画面を閉じるだけにする
  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      onclose();
    } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's') {
      e.preventDefault();
      e.stopPropagation();
      onsave();
    }
  }
</script>

<!-- 補足を広い画面で書くための重ね窓。編集しているのはパネルの補足と同じ値で、
     閉じても消えない（保存していなければ、パネルに「変更を保存」が残る） -->
<!-- Esc と Ctrl+S をこの重ね窓の中だけで受けるため、ダイアログ自体でキー入力を拾う。
     Svelte はダイアログを操作できない要素とみなすので、この 1 つだけ外す -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div class="overlay" role="dialog" aria-modal="true" aria-label="補足を編集" tabindex="-1" onkeydown={onKeydown}>
  <div class="editor">
    <header>
      <span class="title">補足 <span class="task">— {title}</span></span>
      <label class="wrap-toggle">
        <input type="checkbox" bind:checked={wrap} />
        折り返す
      </label>
      <span class="status">{dirty ? '未保存' : '保存済み'}</span>
      <button class="btn primary" onclick={onsave} disabled={!dirty}>保存 <span class="key">Ctrl+S</span></button>
      <button class="btn" onclick={onclose}>閉じる <span class="key">Esc</span></button>
    </header>
    <textarea
      bind:this={area}
      bind:value
      class:nowrap={!wrap}
      wrap={wrap ? 'soft' : 'off'}
      spellcheck="false"
      placeholder="作業名に書ききれない補足。調査の結果や経緯をそのまま書けます"
    ></textarea>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: var(--scrim);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 45;
    outline: none;
  }
  .editor {
    width: min(1200px, 92vw);
    height: 86vh;
    display: flex;
    flex-direction: column;
    background: var(--surface);
    border-radius: 12px;
    box-shadow: var(--shadow-lift);
    overflow: hidden;
  }
  header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 14px;
    border-bottom: 1px solid var(--line);
    font-size: 12.5px;
  }
  .title {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 600;
  }
  .task {
    font-weight: 400;
    color: var(--ink-2);
  }
  .wrap-toggle {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    color: var(--ink-2);
    cursor: pointer;
  }
  .status {
    color: var(--ink-3);
    font-size: 11.5px;
  }
  .key {
    margin-left: 4px;
    font-family: var(--mono);
    font-size: 10.5px;
    opacity: 0.7;
  }
  textarea {
    flex: 1;
    width: 100%;
    resize: none;
    border: none;
    outline: none;
    padding: 14px 18px;
    background: var(--surface);
    color: var(--ink);
    font: inherit;
    font-size: 13.5px;
    line-height: 1.65;
  }
  /* 折り返さないときは横にスクロールし、桁がそろうよう等幅にする（ログや表を貼ったとき） */
  textarea.nowrap {
    white-space: pre;
    overflow-x: auto;
    font-family: var(--mono);
  }
</style>
