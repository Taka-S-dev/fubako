<script lang="ts">
  import { open as openDialog } from '@tauri-apps/plugin-dialog';
  import { accelFromEvent, formatAccel, isModifier } from '$lib/hotkey';
  import type { AppConfig } from '$lib/types';

  let {
    config,
    autostart,
    onsave,
    onclose,
  }: {
    config: AppConfig;
    autostart: boolean;
    onsave: (config: AppConfig, autostart: boolean) => void;
    onclose: () => void;
  } = $props();

  // モーダルは開くたびに再マウントされるため、開いた時点の値で初期化する
  // svelte-ignore state_referenced_locally
  const initial = $state.snapshot(config);
  // svelte-ignore state_referenced_locally
  const initialAutostart = autostart;

  let workRoot = $state(initial.work_root ?? '');
  let archiveRoot = $state(initial.archive_root ?? '');
  let deepRoot = $state(initial.deep_archive_root ?? '');
  let deepMonths = $state(initial.deep_archive_months);
  let hotkey = $state(initial.hotkey);
  let staleDays = $state(initial.stale_days);
  let templatesText = $state(initial.template_files.join('\n'));
  let placesText = $state((initial.places ?? []).join('\n'));
  let auto = $state(initialAutostart);

  async function pick(target: 'work' | 'archive' | 'deep') {
    const dir = await openDialog({ directory: true, title: 'フォルダを選択' });
    if (typeof dir === 'string') {
      if (target === 'work') workRoot = dir;
      else if (target === 'archive') archiveRoot = dir;
      else deepRoot = dir;
    }
  }

  // 既存の行を保ったまま末尾へ足す。並べ替えと削除は欄で直接できる
  async function addPlace() {
    const dir = await openDialog({ directory: true, title: 'フォルダを選択' });
    if (typeof dir !== 'string') return;
    const lines = placesText.split('\n').filter((l) => l.trim());
    if (lines.includes(dir)) return;
    lines.push(dir);
    placesText = lines.join('\n');
  }

  function save() {
    onsave(
      {
        // 隠し設定(display_name等)はUIに出さずそのまま引き継ぐ
        ...initial,
        work_root: workRoot || null,
        archive_root: archiveRoot || null,
        deep_archive_root: deepRoot || null,
        deep_archive_months: Math.max(1, Number(deepMonths) || 6),
        hotkey: hotkey.trim(),
        stale_days: Math.max(1, Number(staleDays) || 14),
        template_files: templatesText
          .split('\n')
          .map((s) => s.trim())
          .filter(Boolean),
        places: placesText
          .split('\n')
          .map((s) => s.trim())
          .filter(Boolean),
      },
      auto
    );
  }

  // ホットキーは記法を覚えずに済むよう、実際に押した組み合わせを取り込む
  let recording = $state(false);
  let rejected = $state(false);

  function startRecording() {
    recording = true;
    rejected = false;
  }

  function onHotkeyKeydown(e: KeyboardEvent) {
    if (!recording) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.key === 'Escape') {
      recording = false;
      return;
    }
    // 修飾キーだけ押している間は待つ
    if (isModifier(e.code)) return;
    const accel = accelFromEvent(e);
    if (!accel) {
      rejected = true;
      return;
    }
    hotkey = accel;
    recording = false;
    rejected = false;
  }

  function clearHotkey() {
    hotkey = '';
    recording = false;
    rejected = false;
  }

  function onkeydown(e: KeyboardEvent) {
    // 記録中の Esc は記録の取り消しに使うので、設定は閉じない
    if (e.key === 'Escape' && !recording) onclose();
  }
</script>

<div class="overlay" onclick={onclose} onkeydown={onkeydown} role="presentation">
  <div
    class="modal"
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => e.stopPropagation()}
    role="dialog"
    aria-label="設定"
    tabindex="-1"
  >
    <h2>設定</h2>

    <div class="row">
      <span class="label">作業ディレクトリ</span>
      <div class="pick">
        <input class="field mono" bind:value={workRoot} placeholder="C:\work" />
        <button class="btn" onclick={() => pick('work')}>選択…</button>
      </div>
    </div>
    <div class="row">
      <span class="label">アーカイブ先</span>
      <div class="pick">
        <input class="field mono" bind:value={archiveRoot} placeholder="C:\work\archive" />
        <button class="btn" onclick={() => pick('archive')}>選択…</button>
      </div>
    </div>
    <div class="row">
      <span class="label">ディープアーカイブ先（任意）</span>
      <div class="pick">
        <input class="field mono" bind:value={deepRoot} placeholder="C:\work\deep_archive" />
        <button class="btn" onclick={() => pick('deep')}>選択…</button>
      </div>
      <p class="hint">
        古い完了タスクの退避先。ここはスキャン・検索の対象外になります（コールドストレージ）。
        指定すると、完了したタスクを1件ずつ退避できるようになります
      </p>
    </div>
    <div class="row">
      <span class="label">ディープアーカイブへ移す目安</span>
      <div class="pick months">
        <span>完了から</span>
        <input class="field days" type="number" min="1" bind:value={deepMonths} />
        <span>か月で整理対象にする</span>
      </div>
    </div>
    <div class="row">
      <span class="label">呼び出しホットキー</span>
      <div class="hk-row">
        <button
          class="field hk"
          class:recording
          onclick={startRecording}
          onkeydown={onHotkeyKeydown}
        >
          {#if recording}
            <span class="hk-prompt">キーを押してください…</span>
          {:else if hotkey}
            <span class="hk-keys">{formatAccel(hotkey)}</span>
          {:else}
            <span class="hk-prompt">未設定</span>
          {/if}
        </button>
        <button class="btn" onclick={clearHotkey} disabled={!hotkey && !recording}>クリア</button>
      </div>
      <p class="hint">
        {#if recording}
          {#if rejected}
            そのキーは使えません。Ctrl・Alt・Shift のいずれかと文字キーを組み合わせてください
          {:else}
            Esc で取り消し
          {/if}
        {:else}
          枠をクリックしてから使いたいキーを押します。修飾キーとの組み合わせが必要です
        {/if}
      </p>
    </div>
    <div class="row">
      <span class="label">放置とみなす日数</span>
      <input class="field days" type="number" min="1" bind:value={staleDays} />
    </div>
    <div class="row">
      <span class="label">新規作成テンプレート</span>
      <textarea
        class="field mono"
        rows="3"
        bind:value={templatesText}
        placeholder={'メモ.md\n資料/'}
      ></textarea>
      <p class="hint">1行1項目。末尾が / ならフォルダを作成</p>
    </div>
    <div class="row">
      <span class="label">よく使う場所</span>
      <div class="pick">
        <textarea
          class="field mono"
          rows="3"
          bind:value={placesText}
          placeholder={'C:\\work\\一時置き\n\\\\server\\共有'}
        ></textarea>
        <button class="btn" onclick={addPlace}>追加…</button>
      </div>
      <p class="hint">
        1行1つ。共有フォルダのパスは貼り付けても構いません。
        作業ディレクトリとアーカイブは設定しなくても一覧に出ます
      </p>
    </div>
    <label class="auto">
      <input type="checkbox" bind:checked={auto} />
      Windows サインイン時に起動する（トレイに常駐）
    </label>

    <div class="buttons">
      <button class="btn" onclick={onclose}>キャンセル</button>
      <button class="btn primary" onclick={save} disabled={!workRoot || !archiveRoot}>保存</button>
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(35, 39, 45, 0.35);
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding-top: 8vh;
    z-index: 40;
  }
  .modal {
    width: 520px;
    max-height: 84vh;
    overflow-y: auto;
    background: var(--surface);
    border-radius: 12px;
    box-shadow: var(--shadow-lift);
    padding: 20px 22px 18px;
  }
  h2 {
    margin: 0 0 14px;
    font-size: 15px;
  }
  .row {
    margin-bottom: 12px;
  }
  .label {
    display: block;
    font-size: 11.5px;
    color: var(--ink-2);
    margin-bottom: 4px;
  }
  .pick {
    display: flex;
    gap: 6px;
  }
  .pick .field {
    flex: 1;
  }
  /* 複数行の欄に添える選択ボタンは、伸びずに上端へ揃える */
  .pick > .btn {
    align-self: flex-start;
  }
  .days {
    width: 90px;
  }
  .months {
    align-items: center;
    font-size: 12px;
    color: var(--ink-2);
  }
  .hint {
    margin: 4px 0 0;
    font-size: 11px;
    color: var(--ink-3);
  }
  .hk-row {
    display: flex;
    gap: 8px;
  }
  .hk {
    flex: 1;
    text-align: left;
    cursor: pointer;
  }
  .hk.recording {
    border-color: var(--manila);
    background: var(--manila-soft);
  }
  .hk-keys {
    font-family: var(--mono);
    font-weight: 600;
  }
  .hk-prompt {
    color: var(--ink-3);
  }
  textarea.field {
    resize: vertical;
  }
  .auto {
    display: flex;
    align-items: center;
    gap: 7px;
    font-size: 12.5px;
    margin: 14px 0 4px;
  }
  .buttons {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 14px;
  }
</style>
