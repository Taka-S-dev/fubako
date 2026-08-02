import type { Task } from './types';

/** 検索語を「タグ限定」と「本文」に分けたもの */
export interface SearchTerms {
  tags: string[];
  text: string[];
}

/** 半角の # と全角の ＃ はどちらもタグ限定として扱う */
function tagPart(token: string): string | null {
  return token.startsWith('#') || token.startsWith('＃') ? token.slice(1) : null;
}

export function parseTerms(query: string): SearchTerms {
  const tags: string[] = [];
  const text: string[] = [];
  for (const token of query.trim().split(/\s+/).filter(Boolean)) {
    const tag = tagPart(token);
    if (tag === null) {
      text.push(token);
    } else if (tag) {
      tags.push(tag);
    }
  }
  return { tags, text };
}

export function hasTerms(terms: SearchTerms): boolean {
  return terms.tags.length > 0 || terms.text.length > 0;
}

/** タグ条件はすべて満たし、本文の語もすべて含むものだけを残す */
export function filterTasks(tasks: Task[], terms: SearchTerms): Task[] {
  if (!hasTerms(terms)) return tasks;
  const tagQueries = terms.tags.map((t) => t.toLowerCase());
  const words = terms.text.map((t) => t.toLowerCase());
  return tasks.filter((t) => {
    const taskTags = t.tags.map((tag) => tag.toLowerCase());
    if (!tagQueries.every((q) => taskTags.some((tag) => tag.includes(q)))) return false;
    if (!words.length) return true;
    const hay = [
      t.folder_name,
      t.name,
      t.memo,
      t.tags.join(' '),
      t.date_prefix ?? '',
      t.checklist.map((i) => i.text).join(' '),
      t.file_names.join(' '),
    ]
      .join(' ')
      .toLowerCase();
    return words.every((w) => hay.includes(w));
  });
}

export function filterLabel(terms: SearchTerms): string {
  const parts: string[] = [];
  if (terms.tags.length) parts.push(`タグ「${terms.tags.join('」「')}」`);
  if (terms.text.length) parts.push(`「${terms.text.join(' ')}」`);
  return `${parts.join(' かつ ')}で絞り込み中`;
}

/** 表記ゆれを防ぐため、既存タグを使用頻度順に並べる */
export function collectTags(tasks: Task[]): string[] {
  const counts = new Map<string, number>();
  for (const t of tasks) {
    for (const tag of t.tags) counts.set(tag, (counts.get(tag) ?? 0) + 1);
  }
  return [...counts.entries()]
    .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0], 'ja'))
    .map(([tag]) => tag);
}

/** 入力中の最後の語がタグなら、その途中の文字列を返す */
export function activeTagPartial(query: string): string | null {
  const tokens = query.split(/\s+/);
  return tagPart(tokens[tokens.length - 1] ?? '');
}

/** 入力中のタグに対する候補。既に指定済みのタグは出さない */
export function tagSuggestions(query: string, allTags: string[], limit = 8): string[] {
  const partial = activeTagPartial(query);
  if (partial === null) return [];
  const tokens = query.split(/\s+/);
  const used = tokens
    .slice(0, -1)
    .map(tagPart)
    .filter((t): t is string => t !== null)
    .map((t) => t.toLowerCase());
  const q = partial.toLowerCase();
  return allTags
    .filter((t) => !used.includes(t.toLowerCase()) && (!q || t.toLowerCase().includes(q)))
    .sort((a, b) => Number(b.toLowerCase().startsWith(q)) - Number(a.toLowerCase().startsWith(q)))
    .slice(0, limit);
}

/** 入力中の最後の語を、選んだタグで置き換える */
export function applyTagToQuery(query: string, tag: string): string {
  const parts = query.split(/\s+/);
  parts[parts.length - 1] = `#${tag}`;
  return `${parts.join(' ')} `;
}
