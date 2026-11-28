import { describe, expect, it } from 'vitest';
import { linkKind, linkLabel } from './links';

describe('linkKind', () => {
  it('http と https はブラウザで開くものとして見分ける', () => {
    expect(linkKind('https://example.com/t/1')).toBe('web');
    expect(linkKind('  HTTP://example.com ')).toBe('web');
  });

  it('末尾の名前に拡張子があればファイル、無ければフォルダ', () => {
    expect(linkKind('C:\\docs\\plan.xlsx')).toBe('file');
    expect(linkKind('\\\\server\\share\\資料')).toBe('folder');
    expect(linkKind('C:\\docs\\')).toBe('folder');
  });
});

describe('linkLabel', () => {
  it('表示名があればそれを出す', () => {
    expect(linkLabel({ url: 'https://example.com/t/1', label: 'チケット' })).toBe('チケット');
  });

  it('表示名が無ければ、URL は全体、パスは末尾の名前を出す', () => {
    expect(linkLabel({ url: 'https://example.com/t/1', label: '' })).toBe('https://example.com/t/1');
    expect(linkLabel({ url: '\\\\server\\share\\献立\\', label: '' })).toBe('献立');
  });
});
