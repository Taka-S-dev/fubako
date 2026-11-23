<#
.SYNOPSIS
Fubako の作業を、アプリを開かずに探して読む。新しい作業を作ることもできる。

.DESCRIPTION
各作業フォルダの .todo.json を読んで、一覧や1件の詳細を出す。今ある .todo.json は
書き換えない（画面で開いている内容とぶつからないよう、書き換えは Fubako だけが行う）。

作業は id で指す。id は .todo.json に入っていて、フォルダの名前を変えても、アーカイブへ
移しても変わらない。ほかの道具が作業を覚えておくときは、パスではなく id を使う。

-Json を付けると、ほかのプログラム向けに JSON で出す。

このスクリプトは Fubako が起動のたびに %APPDATA%\Fubako\tools\ に置く。Fubako の版と一緒に
更新されるので、読み書きする .todo.json の形とずれない。

.EXAMPLE
fubako-tasks.ps1
作業中の作業を、期限切れ・今日まで・近いうち・放置・保留に分けて出す。

.EXAMPLE
fubako-tasks.ps1 -Id 1a2b3c4d
id（先頭の8文字以上でよい）で1件を探し、詳細を出す。アーカイブとディープアーカイブも探す。

.EXAMPLE
fubako-tasks.ps1 -Name 見積 -Archived -Json
名前に「見積」を含む作業を、アーカイブも含めて JSON で出す。

.EXAMPLE
fubako-tasks.ps1 -Create -Name 障害調査 -Due 2026-12-01 -Tags 問い合わせ,至急
作業ディレクトリに 20261123_障害調査 のような新しい作業を作り、その id を出す。

.NOTES
終了コード: 0 = 成功、1 = 失敗（設定が読めない、作れない等）、2 = -Id で見つからない
#>
[CmdletBinding(DefaultParameterSetName = 'List')]
param(
    # id の全体か、先頭の8文字以上
    [Parameter(ParameterSetName = 'List')]
    [string]$Id,

    # 一覧では名前の一部（大文字小文字を区別しない）。-Create では作る作業の名前
    [Parameter(ParameterSetName = 'List')]
    [Parameter(ParameterSetName = 'Create', Mandatory = $true)]
    [string]$Name,

    # 完了してアーカイブにある作業も含める
    [Parameter(ParameterSetName = 'List')]
    [switch]$Archived,

    # ディープアーカイブの作業も含める（量が多いので普段は読まない）
    [Parameter(ParameterSetName = 'List')]
    [switch]$Deep,

    # 「近いうち」とみなす日数
    [Parameter(ParameterSetName = 'List')]
    [int]$SoonDays = 3,

    [Parameter(ParameterSetName = 'Create', Mandatory = $true)]
    [switch]$Create,

    # 期限（YYYY-MM-DD）
    [Parameter(ParameterSetName = 'Create')]
    [string]$Due,

    # タグ。複数はカンマで区切る（-Tags 問い合わせ,至急）。ほかのプログラムから -File で
    # 呼ばれても1つの引数で渡せるよう、配列ではなく文字列で受ける
    [Parameter(ParameterSetName = 'Create')]
    [string]$Tags = '',

    [Parameter(ParameterSetName = 'Create')]
    [ValidateSet('backlog', 'doing')]
    [string]$Status = 'backlog',

    [switch]$Json,

    # Fubako の設定ファイル。普段は指定しない
    [string]$Config = (Join-Path $env:APPDATA 'Fubako\config.json'),

    # 使い方を出す。--help、-h、help でも出る
    [switch]$Help,

    # 名前の付いていない引数を受け止める。--help のような書き方にも使い方で応えるため
    [Parameter(Position = 0, ValueFromRemainingArguments = $true)]
    [string[]]$Rest
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8

# このスクリプトが読める .todo.json の版
$MetaVersion = 1
$MetaFile = '.todo.json'
$Utf8NoBom = New-Object Text.UTF8Encoding($false)

function Stop-WithError([string]$message, [int]$code = 1) {
    [Console]::Error.WriteLine($message)
    exit $code
}

function Get-Prop($obj, [string]$name) {
    if ($null -eq $obj) { return $null }
    $p = $obj.PSObject.Properties[$name]
    if ($p) { return $p.Value }
    return $null
}

function Read-FubakoConfig {
    if (-not (Test-Path -LiteralPath $Config)) {
        Stop-WithError "Fubako の設定が見つかりません（$Config）。Fubako を一度起動して、作業ディレクトリを設定してください"
    }
    try {
        return Get-Content -LiteralPath $Config -Raw -Encoding UTF8 | ConvertFrom-Json
    } catch {
        Stop-WithError "Fubako の設定を読めません（$Config）: $($_.Exception.Message)"
    }
}

function Get-FullPathOrNull([string]$path) {
    if (-not $path) { return $null }
    $resolved = Resolve-Path -LiteralPath $path -ErrorAction SilentlyContinue
    if ($resolved) { return $resolved.Path.TrimEnd('\') }
    return $null
}

function Get-SubDirs([string]$root) {
    if (-not $root -or -not (Test-Path -LiteralPath $root -PathType Container)) { return @() }
    # 名前が . で始まるフォルダは作業として扱わない（Fubako と同じ）
    @(Get-ChildItem -LiteralPath $root -Directory -ErrorAction SilentlyContinue |
        Where-Object { -not $_.Name.StartsWith('.') })
}

function Test-YearDir($dir) { $dir.Name -match '^\d{4}$' }

# Fubako と同じ考え方で、最後に手を付けた時刻を探す。
# 2階層まで降り、.todo.json は数えない（アプリ自身の書き込みで更新されるため）。1000 件で打ち切る
function Get-LastActivity([string]$dir) {
    $script:budget = 1000
    $newest = Get-NewestTime $dir 2
    if ($null -eq $newest) { $newest = (Get-Item -LiteralPath $dir).LastWriteTime }
    return $newest
}

function Get-NewestTime([string]$dir, [int]$depth) {
    $newest = $null
    foreach ($e in @(Get-ChildItem -LiteralPath $dir -Force -ErrorAction SilentlyContinue)) {
        if ($script:budget -le 0) { break }
        $script:budget--
        if ($e.Name -eq $MetaFile) { continue }
        if ($null -eq $newest -or $e.LastWriteTime -gt $newest) { $newest = $e.LastWriteTime }
        if ($e.PSIsContainer -and $depth -gt 0) {
            $sub = Get-NewestTime $e.FullName ($depth - 1)
            if ($null -ne $sub -and $sub -gt $newest) { $newest = $sub }
        }
    }
    return $newest
}

function Split-FolderName([string]$folder) {
    if ($folder -match '^(\d{8})_(.+)$') { return @($Matches[1], $Matches[2]) }
    if ($folder -match '^(\d{8})_?$') { return @($Matches[1], $Matches[1]) }
    return @($null, $folder)
}

function Read-Task($dir, [string]$location, [int]$staleDays) {
    $metaPath = Join-Path $dir.FullName $MetaFile
    $meta = $null
    $readable = $true
    if (Test-Path -LiteralPath $metaPath) {
        try {
            $meta = Get-Content -LiteralPath $metaPath -Raw -Encoding UTF8 | ConvertFrom-Json
        } catch {
            $readable = $false
        }
    }
    $parts = Split-FolderName $dir.Name
    $archived = $location -ne 'work'

    # 完了かどうかはフォルダの位置で決まる（Fubako と同じ）
    $status = Get-Prop $meta 'status'
    if ($archived) { $status = 'done' }
    elseif ($status -eq 'done' -or -not $status) {
        $status = if ($status -eq 'done') { 'doing' } else { 'backlog' }
    }

    $id = Get-Prop $meta 'id'
    if ($id -is [string] -and -not $id.Trim()) { $id = $null }
    $checklist = @(Get-Prop $meta 'checklist' | Where-Object { $_ })
    $onHold = Get-Prop $meta 'on_hold_since'
    $due = Get-Prop $meta 'due'
    $daysLeft = $null
    if ($due) {
        $parsed = [datetime]::MinValue
        if ([datetime]::TryParseExact($due, 'yyyy-MM-dd', [Globalization.CultureInfo]::InvariantCulture, 'None', [ref]$parsed)) {
            $daysLeft = ($parsed.Date - (Get-Date).Date).Days
        }
    }
    $last = Get-LastActivity $dir.FullName
    $idle = ((Get-Date).Date - $last.Date).Days

    $version = Get-Prop $meta 'v'
    [pscustomobject][ordered]@{
        id              = $id
        name            = $parts[1]
        folder          = $dir.Name
        path            = $dir.FullName
        location        = $location
        status          = $status
        on_hold_since   = $onHold
        due             = $due
        days_until_due  = $daysLeft
        tags            = @(Get-Prop $meta 'tags' | Where-Object { $_ })
        memo            = [string](Get-Prop $meta 'memo')
        checklist_done  = @($checklist | Where-Object { $_.done }).Count
        checklist_total = $checklist.Count
        checklist_open  = @($checklist | Where-Object { -not $_.done } | ForEach-Object { $_.text })
        links           = @(Get-Prop $meta 'links' | Where-Object { $_ } | ForEach-Object {
                [pscustomobject][ordered]@{ url = $_.url; label = [string]$_.label }
            })
        created_at      = Get-Prop $meta 'created_at'
        completed_at    = Get-Prop $meta 'completed_at'
        last_activity   = $last.ToString('yyyy-MM-ddTHH:mm:sszzz')
        idle_days       = $idle
        stale           = (-not $archived) -and (-not $onHold) -and ($idle -gt $staleDays)
        # 読めない記録や、このスクリプトより新しい版の記録は、中身を当てにしない
        readable        = $readable
        newer_format    = ($null -ne $version) -and ([int]$version -gt $MetaVersion)
    }
}

function Get-Tasks($cfg, [bool]$withArchive, [bool]$withDeep) {
    $work = Get-FullPathOrNull (Get-Prop $cfg 'work_root')
    $archive = Get-FullPathOrNull (Get-Prop $cfg 'archive_root')
    $deep = Get-FullPathOrNull (Get-Prop $cfg 'deep_archive_root')
    $staleDays = Get-Prop $cfg 'stale_days'
    if (-not $staleDays) { $staleDays = 14 }
    if (-not $work) {
        Stop-WithError "Fubako の作業ディレクトリが見つかりません（$(Get-Prop $cfg 'work_root')）"
    }
    $tasks = New-Object System.Collections.Generic.List[object]
    foreach ($d in Get-SubDirs $work) {
        # アーカイブが作業ディレクトリの直下にある構成でも二重に数えない
        if ($d.FullName -eq $archive -or $d.FullName -eq $deep) { continue }
        $tasks.Add((Read-Task $d 'work' $staleDays))
    }
    $cold = @()
    if ($withArchive -and $archive) { $cold += , @($archive, 'archive') }
    if ($withDeep -and $deep) { $cold += , @($deep, 'deep_archive') }
    foreach ($pair in $cold) {
        # アーカイブは年ごとのフォルダの中と、直下に置かれたものの両方を見る
        foreach ($d in Get-SubDirs $pair[0]) {
            if ($d.FullName -eq $deep) { continue }
            if (Test-YearDir $d) {
                foreach ($sub in Get-SubDirs $d.FullName) {
                    if ($sub.FullName -ne $deep) { $tasks.Add((Read-Task $sub $pair[1] $staleDays)) }
                }
            } else {
                $tasks.Add((Read-Task $d $pair[1] $staleDays))
            }
        }
    }
    return , $tasks.ToArray()
}

$statusLabel = @{ backlog = '未着手'; doing = '作業中'; done = '完了' }

function Format-Line($t) {
    $parts = @("- $($t.name)", "[$($statusLabel[$t.status])]")
    if ($t.on_hold_since) { $parts += '[保留]' }
    if ($t.due) {
        $rest = ''
        if ($null -ne $t.days_until_due) {
            if ($t.days_until_due -lt 0) { $rest = "（$(-$t.days_until_due) 日過ぎ）" }
            elseif ($t.days_until_due -eq 0) { $rest = '（今日）' }
            else { $rest = "（あと $($t.days_until_due) 日）" }
        }
        $parts += "📅 $($t.due)$rest"
    }
    if ($t.checklist_total -gt 0) { $parts += "やること $($t.checklist_done)/$($t.checklist_total)" }
    if ($t.location -ne 'work') { $parts += "（$(if ($t.location -eq 'archive') { 'アーカイブ' } else { 'ディープアーカイブ' })）" }
    if (-not $t.readable) { $parts += '（.todo.json が読めません）' }
    $parts += "id:$(if ($t.id) { $t.id.Substring(0, [Math]::Min(8, $t.id.Length)) } else { 'なし' })"
    return ($parts -join ' ')
}

function Write-Detail($t) {
    Write-Output "# $($t.name)"
    Write-Output "- id: $(if ($t.id) { $t.id } else { 'まだありません（Fubako が一覧を読むときに振ります）' })"
    Write-Output "- 状態: $($statusLabel[$t.status])$(if ($t.on_hold_since) { "（保留: $($t.on_hold_since.Substring(0, 10)) から）" })"
    Write-Output "- 場所: $($t.path)"
    if ($t.due) { Write-Output "- 期限: $($t.due)" }
    if ($t.tags.Count) { Write-Output "- タグ: $($t.tags -join ', ')" }
    if ($t.completed_at) { Write-Output "- 完了: $($t.completed_at.Substring(0, 10))" }
    Write-Output "- 最後に手を付けた日: $($t.last_activity.Substring(0, 10))（$($t.idle_days) 日前）"
    if (-not $t.readable) { Write-Output '- 注意: .todo.json が読めません（書き方が崩れています）' }
    if ($t.newer_format) { Write-Output '- 注意: このスクリプトより新しい形の記録です。中身が正しく読めていない場合があります' }
    if ($t.memo) { Write-Output ''; Write-Output '## 補足'; Write-Output $t.memo }
    if ($t.checklist_total -gt 0) {
        Write-Output ''
        Write-Output "## やること（$($t.checklist_done)/$($t.checklist_total) 済み）"
        $t.checklist_open | ForEach-Object { Write-Output "- [ ] $_" }
    }
    if ($t.links.Count) {
        Write-Output ''
        Write-Output '## 参照'
        $t.links | ForEach-Object { Write-Output "- $(if ($_.label) { "$($_.label): " })$($_.url)" }
    }
}

function Write-Summary($tasks) {
    $active = @($tasks | Where-Object { $_.location -eq 'work' })
    $dated = @($active | Where-Object { $null -ne $_.days_until_due } | Sort-Object days_until_due, name)
    $groups = [ordered]@{
        '期限切れ'                 = @($dated | Where-Object { $_.days_until_due -lt 0 })
        '今日まで'                 = @($dated | Where-Object { $_.days_until_due -eq 0 })
        "$SoonDays 日以内"         = @($dated | Where-Object { $_.days_until_due -gt 0 -and $_.days_until_due -le $SoonDays })
        '放置'                     = @($active | Where-Object { $_.stale } | Sort-Object idle_days -Descending)
        '保留'                     = @($active | Where-Object { $_.on_hold_since } | Sort-Object name)
    }
    Write-Output "# Fubako（作業中 $($active.Count) 件）"
    foreach ($k in $groups.Keys) {
        Write-Output ''
        Write-Output "## $k（$($groups[$k].Count)）"
        foreach ($t in $groups[$k]) {
            $line = Format-Line $t
            if ($k -eq '放置') { $line += "（$($t.idle_days) 日動きなし）" }
            Write-Output $line
        }
    }
    $others = @($active | Where-Object {
            $null -eq $_.days_until_due -or $_.days_until_due -gt $SoonDays
        } | Where-Object { -not $_.stale -and -not $_.on_hold_since } | Sort-Object name)
    Write-Output ''
    Write-Output "## そのほか（$($others.Count)）"
    $others | ForEach-Object { Write-Output (Format-Line $_) }
    $cold = @($tasks | Where-Object { $_.location -ne 'work' })
    if ($cold.Count) {
        Write-Output ''
        Write-Output "## 完了（$($cold.Count)）"
        $cold | Sort-Object completed_at -Descending | ForEach-Object { Write-Output (Format-Line $_) }
    }
}

function Write-JsonOut($value) {
    # 1件でも配列のまま出す（PowerShell 5.1 は1件の配列を中身だけにしてしまう）
    ConvertTo-Json -InputObject $value -Depth 6
}

function ConvertTo-SafeName([string]$raw) {
    # Fubako が作業名に使えない文字を置き換えるのと同じ規則
    ($raw.Trim() -replace '[\\/:*?"<>|]', '_')
}

function New-FubakoTask($cfg) {
    $work = Get-FullPathOrNull (Get-Prop $cfg 'work_root')
    if (-not $work) { Stop-WithError "Fubako の作業ディレクトリが見つかりません（$(Get-Prop $cfg 'work_root')）" }
    $safe = ConvertTo-SafeName $Name
    if (-not $safe) { Stop-WithError '作業名を指定してください' }
    if ($Due) {
        $parsed = [datetime]::MinValue
        if (-not [datetime]::TryParseExact($Due, 'yyyy-MM-dd', [Globalization.CultureInfo]::InvariantCulture, 'None', [ref]$parsed)) {
            Stop-WithError "期限は YYYY-MM-DD で指定してください（$Due）"
        }
    }
    $folder = "$((Get-Date).ToString('yyyyMMdd'))_$safe"
    $dest = Join-Path $work $folder
    if (Test-Path -LiteralPath $dest) { Stop-WithError "$folder は既にあります" }

    $id = [guid]::NewGuid().ToString()
    $meta = [ordered]@{ v = $MetaVersion; id = $id; status = $Status; created_at = (Get-Date).ToString('yyyy-MM-ddTHH:mm:ss.fffzzz') }
    if ($Due) { $meta.due = $Due }
    $cleanTags = @($Tags -split '[,、]' | ForEach-Object { $_.Trim() } | Where-Object { $_ } | Select-Object -Unique)
    if ($cleanTags.Count) { $meta.tags = $cleanTags }

    # . で始まる名前で中身を作り切ってから、作業の名前に変える。Fubako は . で始まるフォルダを
    # 読まないので、.todo.json の無いフォルダを先に見つけて別の id を振ってしまうことがない
    $staging = Join-Path $work ".fubako-new-$($id.Substring(0, 8))"
    try {
        New-Item -ItemType Directory -Path $staging | Out-Null
        [IO.File]::WriteAllText((Join-Path $staging $MetaFile), (ConvertTo-Json -InputObject $meta -Depth 4), $Utf8NoBom)
        Rename-Item -LiteralPath $staging -NewName $folder
    } catch {
        if (Test-Path -LiteralPath $staging) { Remove-Item -LiteralPath $staging -Recurse -Force -ErrorAction SilentlyContinue }
        Stop-WithError "作業を作れませんでした: $($_.Exception.Message)"
    }
    $result = [pscustomobject][ordered]@{ id = $id; name = $safe; folder = $folder; path = $dest }
    if ($Json) { Write-JsonOut $result }
    else {
        Write-Output "作りました: $folder"
        Write-Output "- id: $id"
        Write-Output "- 場所: $dest"
    }
}

if ($Help -or @($Rest | Where-Object { $_ -in @('--help', '-help', '-h', '/?', 'help') }).Count) {
    Get-Help $PSCommandPath -Detailed | Out-String -Width 120
    exit 0
}
if ($Rest) { Stop-WithError "知らない引数です: $($Rest -join ' ')。使い方は --help で出ます" }

$cfg = Read-FubakoConfig

if ($PSCmdlet.ParameterSetName -eq 'Create') {
    New-FubakoTask $cfg
    exit 0
}

if ($Id) {
    $needle = $Id.Trim().ToLowerInvariant()
    if ($needle.Length -lt 8) { Stop-WithError 'id は先頭の8文字以上を指定してください' }
    # id で探すときは、どこへ移っていても見つけられるよう全部を読む
    $all = Get-Tasks $cfg $true $true
    $found = @($all | Where-Object { $_.id -and $_.id.ToLowerInvariant().StartsWith($needle) })
    if ($Json) { Write-JsonOut $found }
    elseif ($found.Count -eq 1) { Write-Detail $found[0] }
    elseif ($found.Count -gt 1) {
        Write-Output "id が $Id で始まる作業が $($found.Count) 件あります。もっと長く指定してください"
        $found | ForEach-Object { Write-Output (Format-Line $_) }
    }
    if ($found.Count -eq 0) {
        if (-not $Json) { [Console]::Error.WriteLine("id が $Id の作業は見つかりません") }
        exit 2
    }
    exit 0
}

$tasks = Get-Tasks $cfg $Archived.IsPresent $Deep.IsPresent
if ($Name) {
    $tasks = @($tasks | Where-Object { $_.name.IndexOf($Name, [StringComparison]::OrdinalIgnoreCase) -ge 0 })
}
if ($Json) { Write-JsonOut @($tasks) }
elseif ($Name) {
    Write-Output "# 「$Name」を含む作業（$($tasks.Count)）"
    $tasks | Sort-Object location, name | ForEach-Object { Write-Output (Format-Line $_) }
} else {
    Write-Summary $tasks
}
exit 0
