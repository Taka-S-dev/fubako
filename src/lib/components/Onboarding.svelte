<script lang="ts">
  let {
    work,
    archive,
    onpick,
    onstart,
  }: {
    work: string;
    archive: string;
    onpick: (target: 'work' | 'archive') => void;
    onstart: () => void;
  } = $props();
</script>

<div class="onboarding">
  <div class="ob-card">
    <h1>作業フォルダを、そのままタスクに</h1>
    <p>
      管理したい作業ディレクトリと、完了後の移動先（アーカイブ）を選ぶと、
      中のフォルダがカードとして並びます。
    </p>
    <div class="ob-row">
      <span class="ob-label">作業ディレクトリ</span>
      <div class="ob-pick">
        <span class="mono ob-path">{work || '未選択'}</span>
        <button class="btn" onclick={() => onpick('work')}>選択…</button>
      </div>
    </div>
    <div class="ob-row">
      <span class="ob-label">アーカイブ先</span>
      <div class="ob-pick">
        <span class="mono ob-path">{archive || '未選択'}</span>
        <button class="btn" onclick={() => onpick('archive')}>選択…</button>
      </div>
    </div>
    <button class="btn primary start" onclick={onstart} disabled={!work || !archive}>
      はじめる
    </button>
  </div>
</div>

<style>
  .onboarding {
    flex: 1;
    display: grid;
    place-items: center;
    padding: 24px;
  }
  .ob-card {
    width: 480px;
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: 14px;
    box-shadow: var(--shadow);
    padding: 28px 30px;
  }
  .ob-card h1 {
    margin: 0 0 8px;
    font-size: 18px;
  }
  .ob-card p {
    margin: 0 0 18px;
    color: var(--ink-2);
    line-height: 1.7;
    font-size: 12.5px;
  }
  .ob-row {
    margin-bottom: 12px;
  }
  .ob-label {
    display: block;
    font-size: 11.5px;
    color: var(--ink-2);
    margin-bottom: 4px;
  }
  .ob-pick {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .ob-path {
    flex: 1;
    font-size: 11.5px;
    color: var(--ink-2);
    background: var(--surface-2);
    border: 1px dashed var(--line-strong);
    border-radius: var(--radius);
    padding: 7px 10px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .start {
    width: 100%;
    justify-content: center;
    margin-top: 10px;
    padding: 10px;
  }
</style>
