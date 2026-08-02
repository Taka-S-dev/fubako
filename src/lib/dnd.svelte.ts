import type { Status, Task } from './types';

/** この距離を超えて動いたらドラッグとみなす。指やペンの微動でカードが飛ばないように */
const DRAG_THRESHOLD_PX = 6;

/**
 * カードのドラッグ&ドロップ。Windows の Tauri では HTML5 の DnD が動かないため、
 * ポインターイベントから自前で組み立てている。
 */
export class CardDrag {
  /** つかんでいるカード。閾値を超えるまでは dragging が false のまま */
  task = $state<Task | null>(null);
  dragging = $state(false);
  ghost = $state({ x: 0, y: 0 });
  over = $state<Status | null>(null);

  /** ドラッグ直後に飛んでくる click を無視するための印 */
  #justDragged = false;
  #startX = 0;
  #startY = 0;
  #onDrop: (task: Task, status: Status) => void;

  constructor(onDrop: (task: Task, status: Status) => void) {
    this.#onDrop = onDrop;
  }

  /** ドラッグ直後かどうか。カードの選択をトグルしてよいかの判定に使う */
  get suppressClick(): boolean {
    return this.#justDragged;
  }

  start = (e: PointerEvent, task: Task) => {
    if (e.button !== 0) return;
    this.task = task;
    this.dragging = false;
    this.#startX = e.clientX;
    this.#startY = e.clientY;
    window.addEventListener('pointermove', this.#move);
    window.addEventListener('pointerup', this.#end);
  };

  #move = (e: PointerEvent) => {
    if (!this.task) return;
    if (!this.dragging) {
      if (Math.hypot(e.clientX - this.#startX, e.clientY - this.#startY) < DRAG_THRESHOLD_PX) {
        return;
      }
      this.dragging = true;
      document.body.style.userSelect = 'none';
    }
    this.ghost = { x: e.clientX, y: e.clientY };
    const el = document.elementFromPoint(e.clientX, e.clientY);
    this.over = (el?.closest('[data-status]')?.getAttribute('data-status') as Status) ?? null;
  };

  #end = () => {
    window.removeEventListener('pointermove', this.#move);
    window.removeEventListener('pointerup', this.#end);
    document.body.style.userSelect = '';
    const task = this.task;
    const target = this.over;
    const moved = this.dragging;
    this.task = null;
    this.dragging = false;
    this.over = null;
    if (!moved) return;
    this.#justDragged = true;
    setTimeout(() => (this.#justDragged = false), 0);
    if (task && target) this.#onDrop(task, target);
  };
}
