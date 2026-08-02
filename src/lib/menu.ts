/** 区切り線。項目の並びの中に挟む */
export const SEPARATOR = 'separator' as const;

export interface MenuAction {
  label: string;
  action: () => void;
  /** 取り消しにくい操作。赤で描く */
  danger?: boolean;
}

export type MenuItem = MenuAction | typeof SEPARATOR;

export interface OpenMenu {
  x: number;
  y: number;
  items: MenuItem[];
}

export function isAction(item: MenuItem): item is MenuAction {
  return item !== SEPARATOR;
}

/**
 * 入力欄と選択中のテキストの上では OS 標準のメニューを残す。
 * コピー・貼り付けと変換候補は、独自メニューでは代われない。
 */
export function wantsNativeMenu(target: EventTarget | null): boolean {
  if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) return true;
  if (target instanceof HTMLElement && target.isContentEditable) return true;
  return !window.getSelection()?.isCollapsed;
}
