import { describe, expect, it } from 'vitest';
import {
  applyTagToQuery,
  collectTags,
  filterTasks,
  matchHint,
  parseTerms,
  tagSuggestions,
} from './search';
import type { Task } from './types';

/** 検索に関わる項目だけを指定して、残りは既定値で埋めたタスクを作る */
function task(over: Partial<Task>): Task {
  return {
    path: 'C:\\work\\20260802_調査',
    folder_name: '20260802_調査',
    name: '調査',
    date_prefix: '20260802',
    status: 'doing',
    archived: false,
    tags: [],
    due: null,
    memo: '',
    checklist: [],
    manual_progress: null,
    progress_mode: 'auto',
    progress: null,
    checklist_done: 0,
    checklist_total: 0,
    remaining_min: null,
    created_at: null,
    completed_at: null,
    last_activity: null,
    file_count: 0,
    file_names: [],
    stale: false,
    ...over,
  };
}

describe('parseTerms', () => {
  it('タグ限定と本文を分ける', () => {
    expect(parseTerms('#調査 ライブラリ')).toEqual({ tags: ['調査'], text: ['ライブラリ'] });
  });

  it('全角の ＃ も半角と同じくタグとして扱う', () => {
    // 日本語入力のまま打つと全角になるため
    expect(parseTerms('＃調査')).toEqual({ tags: ['調査'], text: [] });
  });

  it('空白の連続や前後の空白があっても語が増えない', () => {
    expect(parseTerms('  調査   ライブラリ ')).toEqual({ tags: [], text: ['調査', 'ライブラリ'] });
  });

  it('# だけの入力はまだタグ名が無いので条件にしない', () => {
    // 打ち始めた瞬間に候補が全部消えないようにするため
    expect(parseTerms('#')).toEqual({ tags: [], text: [] });
  });
});

describe('filterTasks', () => {
  const tasks = [
    task({ name: '障害調査', tags: ['調査', '緊急'], memo: 'ログを見る' }),
    task({ name: '記事執筆', tags: ['執筆'], file_names: ['資料\\下書き.md'] }),
    task({ name: '棚卸し', tags: [], checklist: [{ text: '在庫表を更新', done: false, estimate_min: null }] }),
  ];

  it('条件が無ければ全件そのまま返す', () => {
    expect(filterTasks(tasks, parseTerms(''))).toHaveLength(3);
  });

  it('タグ条件と本文の語は AND で効く', () => {
    expect(filterTasks(tasks, parseTerms('#調査 ログ')).map((t) => t.name)).toEqual(['障害調査']);
    // タグは合うが本文が合わない
    expect(filterTasks(tasks, parseTerms('#調査 在庫'))).toHaveLength(0);
  });

  it('サブフォルダの中のファイル名も本文として拾う', () => {
    expect(filterTasks(tasks, parseTerms('下書き')).map((t) => t.name)).toEqual(['記事執筆']);
  });

  it('やることの文言も本文として拾う', () => {
    expect(filterTasks(tasks, parseTerms('在庫表')).map((t) => t.name)).toEqual(['棚卸し']);
  });

  it('英字は大文字小文字を区別しない', () => {
    const t = [task({ name: 'API調査', file_names: ['README.md'] })];
    expect(filterTasks(t, parseTerms('readme'))).toHaveLength(1);
  });
});

describe('matchHint', () => {
  it('カードに出ている名前で当たったなら何も言わない', () => {
    const t = task({ name: '障害調査' });
    expect(matchHint(t, parseTerms('障害'))).toBeNull();
  });

  it('ファイル名で当たったらそのファイル名を返す', () => {
    const t = task({ name: '記事執筆', file_names: ['資料\\下書き.md'] });
    expect(matchHint(t, parseTerms('下書き'))).toBe('資料\\下書き.md');
  });

  it('やること・補足で当たったらその場所を返す', () => {
    const withChecklist = task({
      checklist: [{ text: '在庫表を更新', done: false, estimate_min: null }],
    });
    expect(matchHint(withChecklist, parseTerms('在庫表'))).toBe('やること');
    expect(matchHint(task({ memo: '担当は経理' }), parseTerms('経理'))).toBe('補足');
  });

  it('語が複数の場所に散っているときは何も言わない', () => {
    // 「ここが一致した」と1箇所を示すと嘘になる
    const t = task({ name: '調査', memo: '担当は経理', file_names: ['下書き.md'] });
    expect(matchHint(t, parseTerms('経理 下書き'))).toBeNull();
  });

  it('タグだけの絞り込みでは出さない', () => {
    expect(matchHint(task({ tags: ['調査'] }), parseTerms('#調査'))).toBeNull();
  });
});

describe('tagSuggestions', () => {
  const all = ['調査', '執筆', '緊急'];

  it('タグを打ち始めていないときは何も出さない', () => {
    expect(tagSuggestions('調査', all)).toEqual([]);
  });

  it('# だけでも既存タグを一覧できる', () => {
    expect(tagSuggestions('#', all)).toEqual(all);
  });

  it('前方一致を部分一致より先に並べる', () => {
    expect(tagSuggestions('#急', ['緊急', '急ぎ'])).toEqual(['急ぎ', '緊急']);
  });

  it('既に指定済みのタグは候補から外す', () => {
    expect(tagSuggestions('#調査 #', all)).toEqual(['執筆', '緊急']);
  });
});

describe('applyTagToQuery', () => {
  it('入力中の語だけを差し替え、続けて打てるよう末尾に空白を足す', () => {
    expect(applyTagToQuery('#調', '調査')).toBe('#調査 ');
    expect(applyTagToQuery('ログ #執', '執筆')).toBe('ログ #執筆 ');
  });
});

describe('collectTags', () => {
  it('使用頻度の高い順に並べ、同数なら五十音順にする', () => {
    const tasks = [
      task({ tags: ['執筆', '調査'] }),
      task({ tags: ['調査'] }),
      task({ tags: ['緊急'] }),
    ];
    expect(collectTags(tasks)).toEqual(['調査', '緊急', '執筆']);
  });
});
