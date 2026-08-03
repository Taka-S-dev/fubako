export function fmtDate(iso: string | null | undefined): string {
  if (!iso) return '—';
  const d = new Date(iso);
  if (isNaN(d.getTime())) return '—';
  return `${d.getFullYear()}/${String(d.getMonth() + 1).padStart(2, '0')}/${String(d.getDate()).padStart(2, '0')}`;
}

export function fmtDateTime(iso: string | null | undefined): string {
  if (!iso) return '—';
  const d = new Date(iso);
  if (isNaN(d.getTime())) return '—';
  return `${fmtDate(iso)} ${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`;
}

export function daysSince(iso: string | null | undefined): number | null {
  if (!iso) return null;
  const d = new Date(iso);
  if (isNaN(d.getTime())) return null;
  return Math.floor((Date.now() - d.getTime()) / 86_400_000);
}

/** 相対表示: 今日 / 昨日 / n日前 / n週間前 */
export function relativeDays(iso: string | null | undefined): string {
  const days = daysSince(iso);
  if (days === null) return '—';
  if (days <= 0) return '今日';
  if (days === 1) return '昨日';
  if (days < 14) return `${days}日前`;
  return `${Math.floor(days / 7)}週間前`;
}

export function fmtSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`;
}

/** "20260802" -> "2026.08.02" */
export function fmtPrefix(prefix: string | null): string | null {
  if (!prefix || prefix.length !== 8) return null;
  return `${prefix.slice(0, 4)}.${prefix.slice(4, 6)}.${prefix.slice(6, 8)}`;
}

export function isOverdue(due: string | null): boolean {
  if (!due) return false;
  const d = new Date(due + 'T23:59:59');
  return !isNaN(d.getTime()) && d.getTime() < Date.now();
}

/**
 * 工数表現を分に正規化する（Jira/GitLab型の単位つき自由入力）。
 * 受理: "90" (=分) / "1.5h" / "90m" / "1h30m" / "1h30" / "45分" / "2時間" / "1時間30分"
 *
 * 分の単位は省略できる。読み取れない入力は数字だけを拾わずに null を返す
 * （"1h30" を 1 分と解釈するような、黙って桁の違う値になる読み方をしない）
 */
export function parseDuration(input: string): number | null {
  const s = input.trim().toLowerCase().replaceAll('時間', 'h').replaceAll('分', 'm');
  if (!s) return null;
  const m = s.match(/^(?:(\d+(?:\.\d+)?)\s*h)?\s*(?:(\d+(?:\.\d+)?)\s*m?)?$/);
  if (!m || !(m[1] || m[2])) return null;
  const min = Math.round(parseFloat(m[1] ?? '0') * 60 + parseFloat(m[2] ?? '0'));
  return min > 0 ? min : null;
}

/** 分を正規形 "45m" / "1h" / "1h30m" に（入力欄のエコー用・可逆） */
export function fmtDurationCompact(min: number): string {
  if (min < 60) return `${min}m`;
  const h = Math.floor(min / 60);
  const m = min % 60;
  return m ? `${h}h${m}m` : `${h}h`;
}

/** 分を "45分" / "2.5h" 表記に */
export function fmtMinutes(min: number): string {
  if (min < 60) return `${min}分`;
  const h = min / 60;
  return Number.isInteger(h) ? `${h}h` : `${h.toFixed(1)}h`;
}

export function todayPrefix(): string {
  const d = new Date();
  return `${d.getFullYear()}${String(d.getMonth() + 1).padStart(2, '0')}${String(d.getDate()).padStart(2, '0')}`;
}
