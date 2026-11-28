import type { TaskLink } from './types';

/**
 * 押した先が何で開くかを、表示名の裏に隠れても分かるようにする。
 * 実物は見に行かない。参照の先は切断中の共有や不在のパスであり得るので、
 * 絵を選ぶためだけにディスクへ問い合わせると、繋がらない場所で待たされる。
 * 名前に拡張子があるかで見分けるため、ドットを含むフォルダは外すが、
 * 外れても絵がずれるだけで開く動作は変わらない
 */
export function linkKind(url: string): 'web' | 'file' | 'folder' {
  const u = url.trim();
  const lower = u.toLowerCase();
  if (lower.startsWith('http://') || lower.startsWith('https://')) return 'web';
  const name = u.replace(/[\\/]+$/, '').split(/[\\/]/).pop() ?? '';
  return /\.[^.]+$/.test(name) ? 'file' : 'folder';
}

/**
 * 表示名が無いときに出す文字。パスは頭から詰めると、ドライブと共通の親という
 * 一番どうでもいい部分だけが残り、何を指しているか読めなくなる。
 * 識別に効くのは末尾なので、そこだけを出す。全体はホバーの title で読める
 */
export function linkLabel(link: TaskLink): string {
  if (link.label) return link.label;
  const u = link.url.trim();
  if (linkKind(u) === 'web') return u;
  return u.replace(/[\\/]+$/, '').split(/[\\/]/).pop() || u;
}
