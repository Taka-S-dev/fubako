<script lang="ts">
  import { isAction, type MenuItem } from '$lib/menu';

  let {
    x,
    y,
    items,
    onclose,
  }: {
    x: number;
    y: number;
    items: MenuItem[];
    onclose: () => void;
  } = $props();

  /** 画面の端で見切れないよう、はみ出す分だけ内側へ寄せる */
  const MARGIN = 6;

  let el: HTMLDivElement | undefined = $state();
  let pos = $state({ x: 0, y: 0 });
  /** 大きさを測るまで位置が決まらないので、それまでは描かない */
  let placed = $state(false);
  let highlight = $state(-1);

  const actions = $derived(items.filter(isAction));

  $effect(() => {
    const node = el;
    // 開いたまま別の場所を右クリックすると同じ要素が使い回されるため、
    // x と y を読んで位置を計算し直す
    const [wantX, wantY] = [x, y];
    if (!node) return;
    const box = node.getBoundingClientRect();
    pos = {
      x: Math.max(MARGIN, Math.min(wantX, window.innerWidth - box.width - MARGIN)),
      y: Math.max(MARGIN, Math.min(wantY, window.innerHeight - box.height - MARGIN)),
    };
    placed = true;
    highlight = -1;
    node.focus();
  });

  function run(item: MenuItem) {
    if (!isAction(item)) return;
    onclose();
    item.action();
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.stopPropagation();
      onclose();
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      highlight = (highlight + 1) % actions.length;
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      highlight = (highlight - 1 + actions.length) % actions.length;
    } else if (e.key === 'Enter' && highlight >= 0) {
      e.preventDefault();
      run(actions[highlight]);
    }
  }
</script>

<!-- 外側のどこを触っても閉じる。スクロールでも位置がずれるので閉じる -->
<svelte:window onpointerdown={onclose} onwheel={onclose} onresize={onclose} onblur={onclose} />

<div
  bind:this={el}
  class="menu"
  role="menu"
  tabindex="-1"
  class:placed
  style="left: {pos.x}px; top: {pos.y}px"
  onkeydown={onKeydown}
  onpointerdown={(e) => e.stopPropagation()}
  oncontextmenu={(e) => e.preventDefault()}
>
  {#each items as item, i (i)}
    {#if isAction(item)}
      {@const index = actions.indexOf(item)}
      <button
        role="menuitem"
        class:danger={item.danger}
        class:active={index === highlight}
        onmouseenter={() => (highlight = index)}
        onclick={() => run(item)}
      >
        {item.label}
      </button>
    {:else}
      <hr />
    {/if}
  {/each}
</div>

<style>
  .menu {
    position: fixed;
    z-index: 80;
    min-width: 176px;
    padding: 4px;
    background: var(--surface);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius);
    box-shadow: var(--shadow-lift);
    visibility: hidden;
  }
  .menu.placed {
    visibility: visible;
  }
  .menu:focus {
    outline: none;
  }
  button {
    display: block;
    width: 100%;
    text-align: left;
    border: none;
    background: none;
    padding: 6px 10px;
    border-radius: 5px;
    font-size: 12.5px;
    color: var(--ink);
  }
  button.active {
    background: var(--manila-soft);
  }
  button.danger {
    color: var(--red);
  }
  button.danger.active {
    background: var(--red);
    color: #fff;
  }
  hr {
    margin: 4px 6px;
    border: none;
    border-top: 1px solid var(--line);
  }
</style>
