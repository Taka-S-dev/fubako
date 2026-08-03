import { describe, expect, it } from 'vitest';
import {
  fmtDurationCompact,
  fmtMinutes,
  fmtPrefix,
  fmtSize,
  isoDate,
  isoMonth,
  parseDuration,
  todayPrefix,
} from './format';

describe('parseDuration', () => {
  it('単位が無ければ分として読む', () => {
    expect(parseDuration('90')).toBe(90);
  });

  it('時間・分の単位を英字でも日本語でも受ける', () => {
    for (const input of ['1h30m', '1時間30分', '90m', '90分']) {
      expect(parseDuration(input), input).toBe(90);
    }
    expect(parseDuration('2時間')).toBe(120);
  });

  it('時間のあとの分は単位を省いても読める', () => {
    // 数字だけを拾って 1 分と解釈すると、桁の違う値が黙って入る
    expect(parseDuration('1h30')).toBe(90);
  });

  it('小数の時間を分へ丸める', () => {
    expect(parseDuration('1.5h')).toBe(90);
    expect(parseDuration('0.7h')).toBe(42);
  });

  it('空白や大文字の揺れを吸収する', () => {
    expect(parseDuration('  1H 30M  ')).toBe(90);
  });

  it('見積として意味の無い入力は受け付けない', () => {
    for (const input of ['', '  ', 'あとで', '0', '-30', '3x', '5個', 'h30']) {
      expect(parseDuration(input), input).toBeNull();
    }
  });
});

describe('fmtDurationCompact', () => {
  it('parseDuration で読み直すと同じ分数に戻る', () => {
    // 入力欄へ書き戻す表記なので、往復して値が変わらないこと
    for (const min of [1, 45, 60, 90, 125, 600]) {
      expect(parseDuration(fmtDurationCompact(min)), `${min}`).toBe(min);
    }
  });

  it('端数が無ければ分を省く', () => {
    expect(fmtDurationCompact(120)).toBe('2h');
    expect(fmtDurationCompact(45)).toBe('45m');
  });
});

describe('fmtMinutes', () => {
  it('1時間未満は分、それ以上は時間で表す', () => {
    expect(fmtMinutes(45)).toBe('45分');
    expect(fmtMinutes(120)).toBe('2h');
    expect(fmtMinutes(150)).toBe('2.5h');
  });
});

describe('fmtPrefix', () => {
  it('8桁の日付だけを区切って返す', () => {
    expect(fmtPrefix('20260802')).toBe('2026.08.02');
  });

  it('日付として扱えないものは null（フォルダ名に日付が無い場合）', () => {
    expect(fmtPrefix(null)).toBeNull();
    expect(fmtPrefix('2026')).toBeNull();
  });
});

describe('isoDate', () => {
  it('同じ日ならローカル時刻の何時であっても同じ日付を返す', () => {
    // UTC 基準（toISOString）で組み立てると、時差のぶんだけ日付が隣へずれる。
    // 東側・西側どちらの地域でも、この2つのどちらかが翌日/前日になる
    expect(isoDate(new Date(2026, 7, 2, 0, 0))).toBe('2026-08-02');
    expect(isoDate(new Date(2026, 7, 2, 23, 59))).toBe('2026-08-02');
  });

  it('月と日を2桁に揃える', () => {
    expect(isoDate(new Date(2026, 0, 5, 12, 0))).toBe('2026-01-05');
  });

  it('月だけを取り出せる', () => {
    expect(isoMonth(new Date(2026, 11, 31, 23, 0))).toBe('2026-12');
  });

  it('フォルダ名の日付も同じ基準で作る', () => {
    expect(todayPrefix()).toBe(isoDate(new Date()).replaceAll('-', ''));
  });
});

describe('fmtSize', () => {
  it('桁に応じて単位を切り替える', () => {
    expect(fmtSize(512)).toBe('512 B');
    expect(fmtSize(1024)).toBe('1.0 KB');
    expect(fmtSize(1024 * 1024 * 3)).toBe('3.0 MB');
  });
});
