# fubako-tasks.ps1 のテスト。Windows PowerShell 5.1 で動かす:
#   powershell -NoProfile -ExecutionPolicy Bypass -File tools/fubako-tasks.test.ps1
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$script = Join-Path $PSScriptRoot 'fubako-tasks.ps1'
$tmp = Join-Path ([IO.Path]::GetTempPath()) ('fubako-tasks-test-' + [guid]::NewGuid().ToString('N'))
$utf8 = New-Object Text.UTF8Encoding($false)
$script:passed = 0

function Assert($condition, [string]$label) {
    if (-not $condition) { throw "FAIL: $label" }
    $script:passed++
    Write-Output "PASS: $label"
}

function Write-Meta([string]$dir, [string]$json) {
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
    [IO.File]::WriteAllText((Join-Path $dir '.todo.json'), $json, $utf8)
}

function Invoke-Tasks([string[]]$arguments) {
    # 5.1 は Stop のままだと、子の標準エラーを受けた時点で止まってしまう
    $ErrorActionPreference = 'Continue'
    $out = & powershell -NoProfile -ExecutionPolicy Bypass -File $script -Config $cfgPath @arguments 2>&1
    return [pscustomobject]@{ code = $LASTEXITCODE; text = ($out | Out-String) }
}

function Invoke-Json([string[]]$arguments) {
    $r = Invoke-Tasks ($arguments + '-Json')
    if ($r.code -ne 0) { throw "exit $($r.code): $($r.text)" }
    # 5.1 の ConvertFrom-Json は配列を1つのかたまりとして流すので、いったん受けてから配列にする
    $parsed = $r.text | ConvertFrom-Json
    return , @($parsed)
}

try {
    $work = Join-Path $tmp 'work'
    $archive = Join-Path $tmp 'archive'
    $deep = Join-Path $tmp 'deep'
    New-Item -ItemType Directory -Force -Path $work, $archive, $deep | Out-Null
    $cfgPath = Join-Path $tmp 'config.json'
    [IO.File]::WriteAllText($cfgPath, (@{ work_root = $work; archive_root = $archive; deep_archive_root = $deep; stale_days = 14 } | ConvertTo-Json), $utf8)

    $yesterday = (Get-Date).AddDays(-1).ToString('yyyy-MM-dd')
    $soon = (Get-Date).AddDays(2).ToString('yyyy-MM-dd')
    Write-Meta (Join-Path $work '20261101_障害調査') "{`"v`":1,`"id`":`"aaaaaaaa-0000-4000-8000-000000000001`",`"status`":`"doing`",`"due`":`"$yesterday`",`"memo`":`"ログを見る`",`"checklist`":[{`"text`":`"再現`",`"done`":true},{`"text`":`"報告`",`"done`":false}],`"mytool`":{`"x`":1}}"
    Write-Meta (Join-Path $work '20261102_見積もり') "{`"id`":`"bbbbbbbb-0000-4000-8000-000000000002`",`"status`":`"backlog`",`"due`":`"$soon`",`"on_hold_since`":`"2026-11-20T10:00:00.123456789+09:00`"}"
    Write-Meta (Join-Path $work '20261103_移動の途中') '{"id":"cccccccc-0000-4000-8000-000000000003","status":"done"}'
    Write-Meta (Join-Path $work '20261104_壊れた') '{ 途中'
    New-Item -ItemType Directory -Force -Path (Join-Path $work '.隠し') | Out-Null
    Write-Meta (Join-Path $archive '2026\20261010_終わった作業') '{"id":"dddddddd-0000-4000-8000-000000000004","status":"done","completed_at":"2026-10-20T12:00:00+09:00"}'
    Write-Meta (Join-Path $deep '2025\20250101_昔の作業') '{"id":"eeeeeeee-0000-4000-8000-000000000005","status":"done"}'
    $before = Get-FileHash (Join-Path $work '20261101_障害調査\.todo.json')

    # 一覧（作業ディレクトリだけ）
    $list = Invoke-Json @()
    Assert ($list.Count -eq 4) '一覧は作業ディレクトリの作業だけで、. で始まるフォルダは入らない'
    $byName = @{}; $list | ForEach-Object { $byName[$_.name] = $_ }
    Assert ($byName['障害調査'].status -eq 'doing' -and $byName['障害調査'].days_until_due -eq -1) '期限までの日数を出す'
    Assert ($byName['障害調査'].checklist_open.Count -eq 1 -and $byName['障害調査'].checklist_open[0] -eq '報告') '終わっていないやることを出す'
    Assert ($byName['移動の途中'].status -eq 'doing') '作業側にある done は作業中として扱う（Fubako と同じ）'
    Assert (-not $byName['壊れた'].readable) '読めない .todo.json は readable=false で一覧に出る'
    Assert ($byName['見積もり'].on_hold_since -and $byName['見積もり'].location -eq 'work') '保留の時刻をそのまま出す'

    $summary = (Invoke-Tasks @()).text
    Assert ($summary -match '## 期限切れ（1）' -and $summary -match '障害調査') 'まとめでは期限切れを分けて出す'
    Assert ($summary -match '## 3 日以内（1）' -and $summary -match '## 保留（1）') 'まとめでは近いうちと保留を分けて出す'

    # 読むだけで書き換えない
    Assert ((Get-FileHash (Join-Path $work '20261101_障害調査\.todo.json')).Hash -eq $before.Hash) '今ある .todo.json を書き換えない'

    # アーカイブを含める
    $withArchive = Invoke-Json @('-Archived')
    Assert ($withArchive.Count -eq 5 -and @($withArchive | Where-Object { $_.location -eq 'archive' }).Count -eq 1) '-Archived で年のフォルダの中の完了も読む'

    # id で探す（どこへ移っていても見つかる）
    $deepHit = Invoke-Tasks @('-Id', 'eeeeeeee')
    Assert ($deepHit.code -eq 0 -and $deepHit.text -match '昔の作業') 'id の先頭8文字でディープアーカイブの作業も見つける'
    $full = Invoke-Json @('-Id', 'DDDDDDDD-0000-4000-8000-000000000004')
    Assert ($full.Count -eq 1 -and $full[0].location -eq 'archive') 'id は大文字小文字を区別しない'
    Assert ((Invoke-Tasks @('-Id', 'ffffffff')).code -eq 2) '見つからない id は終了コード 2'
    Assert ((Invoke-Tasks @('-Id', 'aaaa')).code -eq 1) '8文字より短い id は断る'

    # 名前で探す
    $byPart = Invoke-Json @('-Name', '見積')
    Assert ($byPart.Count -eq 1 -and $byPart[0].id -like 'bbbbbbbb*') '名前の一部で探す'

    # 作る
    $created = Invoke-Json @('-Create', '-Name', '問い合わせ:対応', '-Due', '2026-12-01', '-Tags', 'a,a、b')
    $c = $created[0]
    Assert ($c.folder -match '^\d{8}_問い合わせ_対応$') '作業名に使えない文字は Fubako と同じく _ にする'
    $meta = Get-Content -LiteralPath (Join-Path $c.path '.todo.json') -Raw -Encoding UTF8 | ConvertFrom-Json
    Assert ($meta.v -eq 1 -and $meta.id -eq $c.id -and $meta.status -eq 'backlog' -and $meta.due -eq '2026-12-01') '作った .todo.json に版・id・期限が入る'
    Assert (@($meta.tags).Count -eq 2) 'タグの重なりを除く'
    $bytes = [IO.File]::ReadAllBytes((Join-Path $c.path '.todo.json'))
    Assert (-not ($bytes[0] -eq 0xEF -and $bytes[1] -eq 0xBB)) '.todo.json は BOM なしで書く'
    Assert (@(Get-ChildItem -LiteralPath $work -Directory -Force | Where-Object { $_.Name -like '.fubako-new-*' }).Count -eq 0) '作りかけのフォルダを残さない'
    Assert ((Invoke-Json @('-Id', $c.id)).Count -eq 1) '作った作業をその場で id で探せる'

    $again = Invoke-Tasks @('-Create', '-Name', '問い合わせ:対応')
    Assert ($again.code -eq 1 -and $again.text -match '既にあります') '同じ名前の作業がある日は作らない'
    Assert ((Invoke-Tasks @('-Create', '-Name', 'x', '-Due', '来週')).code -eq 1) '読めない期限では作らない'

    # 画面を動かすリンク（-WhatIf で組み立てだけを見る）
    $open = Invoke-Tasks @('-OpenInApp', 'aaaaaaaa', '-WhatIf')
    Assert ($open.code -eq 0 -and $open.text -match 'fubako://open\?id=aaaaaaaa-0000-4000-8000-000000000001') '-OpenInApp は id の先頭から作業を引き、全体の id でリンクを組む'
    Assert ((Invoke-Tasks @('-OpenInApp', 'ffffffff', '-WhatIf')).code -eq 2) '-OpenInApp で見つからない id は終了コード 2'
    $deepOpen = Invoke-Tasks @('-OpenInApp', 'eeeeeeee', '-WhatIf')
    Assert ($deepOpen.code -eq 1 -and $deepOpen.text -match 'ディープアーカイブ') 'ディープアーカイブの作業は画面で開けないと断る'
    $search = Invoke-Tasks @('-SearchInApp', '見積 #調査&x=1', '-WhatIf')
    Assert ($search.code -eq 0 -and $search.text -match 'fubako://search\?q=%E8%A6%8B%E7%A9%8D%20%23%E8%AA%BF%E6%9F%BB%26x%3D1') '-SearchInApp は検索語を符号化してリンクに入れる（& や = で別の指定を足せない）'
    Assert ((Invoke-Tasks @('-SearchInApp', ' ', '-WhatIf')).code -eq 1) '空の検索語は断る'
    Assert ((Invoke-Tasks @('-SearchInApp', ('あ' * 201), '-WhatIf')).code -eq 1) '長すぎる検索語は断る'

    # 使い方
    $usage = Invoke-Tasks @('--help')
    Assert ($usage.code -eq 0 -and $usage.text -match '-Create' -and $usage.text -match 'id') '--help で使い方を出す'
    Assert ((Invoke-Tasks @('-Help')).code -eq 0) '-Help でも使い方を出す'
    $unknown = Invoke-Tasks @('--list')
    Assert ($unknown.code -eq 1 -and $unknown.text -match '--help') '知らない引数は、使い方の出し方を添えて断る'

    # 設定が無い
    $cfgPath = Join-Path $tmp 'none.json'
    $missing = Invoke-Tasks @()
    Assert ($missing.code -eq 1 -and $missing.text -match '設定が見つかりません') '設定が無ければ理由を言って終わる'

    Write-Output "ok: $script:passed passed"
} finally {
    Remove-Item -LiteralPath $tmp -Recurse -Force -ErrorAction SilentlyContinue
}
