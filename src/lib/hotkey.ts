/**
 * 押されたキーを Tauri のアクセラレータ表記へ変換する。
 * 設定ファイルに保存する形式は従来どおりなので、既存の設定はそのまま動く。
 */

/** 修飾キー単体では確定させない。押している途中の状態にすぎない */
const MODIFIER_CODES = [
  'ControlLeft',
  'ControlRight',
  'AltLeft',
  'AltRight',
  'ShiftLeft',
  'ShiftRight',
  'MetaLeft',
  'MetaRight',
];

/** 画面に出すときだけ短く読みやすい形に置き換える */
const DISPLAY_NAMES: Record<string, string> = {
  Ctrl: 'Ctrl',
  Alt: 'Alt',
  Shift: 'Shift',
  Super: 'Win',
};

export function isModifier(code: string): boolean {
  return MODIFIER_CODES.includes(code);
}

/**
 * 記録したキーからアクセラレータを組み立てる。確定できないときは null。
 * 修飾キーを必須にしているのは、単独キーを登録すると他のアプリで
 * その文字が打てなくなるため。
 */
export function accelFromEvent(e: KeyboardEvent): string | null {
  if (isModifier(e.code)) return null;
  const parts: string[] = [];
  if (e.ctrlKey) parts.push('Ctrl');
  if (e.altKey) parts.push('Alt');
  if (e.shiftKey) parts.push('Shift');
  if (e.metaKey) parts.push('Super');
  if (!parts.length) return null;
  if (!isBindableCode(e.code)) return null;
  parts.push(e.code);
  return parts.join('+');
}

/** Tauri が解釈できるキーだけを受け付ける */
function isBindableCode(code: string): boolean {
  return (
    /^Key[A-Z]$/.test(code) ||
    /^Digit[0-9]$/.test(code) ||
    /^F([1-9]|1[0-9]|2[0-4])$/.test(code) ||
    ['Space', 'Enter', 'Tab', 'Backspace', 'Delete', 'Insert', 'Home', 'End'].includes(code) ||
    ['PageUp', 'PageDown', 'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight'].includes(code)
  );
}

/** `Ctrl+Alt+KeyE` を `Ctrl + Alt + E` にする */
export function formatAccel(accel: string): string {
  if (!accel) return '';
  return accel
    .split('+')
    .map((part) => DISPLAY_NAMES[part] ?? part.replace(/^Key/, '').replace(/^Digit/, ''))
    .join(' + ');
}
