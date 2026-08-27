<script lang="ts">
  import { todayPrefix } from '$lib/format';

  let {
    templates,
    oncreate,
    onclose,
  }: {
    templates: string[];
    oncreate: (name: string, useTemplate: boolean) => void;
    onclose: () => void;
  } = $props();

  let name = $state('');
  let useTemplate = $state(true);
  let inputEl: HTMLInputElement | undefined = $state();

  $effect(() => {
    inputEl?.focus();
  });

  const folderName = $derived(`${todayPrefix()}_${name.trim() || '作業名'}`);

  function submit() {
    if (!name.trim()) return;
    oncreate(name.trim(), useTemplate);
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') submit();
    if (e.key === 'Escape') onclose();
  }
</script>

<div
  class="overlay"
  onclick={onclose}
  onkeydown={(e) => e.key === 'Escape' && onclose()}
  role="presentation"
>
  <div
    class="modal"
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => e.stopPropagation()}
    role="dialog"
    aria-label="新しい作業"
    tabindex="-1"
  >
    <h2>新しい作業</h2>
    <input
      class="field big"
      bind:this={inputEl}
      bind:value={name}
      placeholder="作業名（例: 障害調査）"
      onkeydown={onkeydown}
    />
    <p class="preview">
      作成されるフォルダ: <span class="mono">{folderName}</span>
    </p>
    {#if templates.length > 0}
      <label class="tpl">
        <input type="checkbox" bind:checked={useTemplate} />
        テンプレートを展開（{templates.join(' / ')}）
      </label>
    {/if}
    <div class="buttons">
      <button class="btn" onclick={onclose}>キャンセル</button>
      <button class="btn primary" onclick={submit} disabled={!name.trim()}>作成 (Enter)</button>
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: var(--scrim);
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding-top: 18vh;
    z-index: 40;
  }
  .modal {
    width: 440px;
    background: var(--surface);
    border-radius: 12px;
    box-shadow: var(--shadow-lift);
    padding: 20px 22px 18px;
  }
  h2 {
    margin: 0 0 12px;
    font-size: 15px;
  }
  .big {
    font-size: 15px;
    padding: 10px 12px;
  }
  .preview {
    margin: 10px 0 0;
    font-size: 12px;
    color: var(--ink-2);
  }
  .preview .mono {
    color: var(--manila);
    font-weight: 600;
  }
  .tpl {
    display: flex;
    align-items: center;
    gap: 7px;
    margin-top: 10px;
    font-size: 12px;
    color: var(--ink-2);
  }
  .buttons {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 16px;
  }
</style>
