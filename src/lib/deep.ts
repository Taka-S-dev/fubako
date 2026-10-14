import type { SearchTerms } from './search';
import type { Task } from './types';

/**
 * ディープアーカイブの検索結果を、ボードとリストへ渡すときの形。
 * 管理の外にあるので、選べる操作は開くか作業へ戻すかだけ
 */
export interface DeepView {
  /** 検索語に当たったもの */
  tasks: Task[];
  loading: boolean;
  /** 読む件数の上限で打ち切った。見つからないのが「無い」のか分からなくなるので告げる */
  capped: boolean;
  terms: SearchTerms;
  /** 右のペインに出しているもの */
  selectedPath: string | null;
  /** 押したら右のペインに読むだけの詳細を出す */
  onselect: (t: Task) => void;
  onopen: (t: Task) => void;
  onrestore: (t: Task) => void;
  onclose: () => void;
}
