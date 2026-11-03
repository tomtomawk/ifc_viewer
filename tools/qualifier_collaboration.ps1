param(
    [string]$Executable = "$PSScriptRoot\..\target\release\bimedit.exe",
    [string]$DepotParent = "$env:TEMP\bimedit-qualification",
    [string]$CopieParent = "$env:TEMP\bimedit-postes"
)
$ErrorActionPreference = 'Stop'
function Invoke-Bim([string[]]$Arguments) {
    $journal = Join-Path $env:TEMP ([guid]::NewGuid().ToString())
    $quoted = $Arguments | ForEach-Object { '"' + $_.Replace('"', '\"') + '"' }
    $processus = Start-Process -FilePath $Executable -ArgumentList $quoted -WindowStyle Hidden -Wait -PassThru -RedirectStandardOutput "$journal.out" -RedirectStandardError "$journal.err"
    Get-Content -LiteralPath "$journal.out"
    Get-Content -LiteralPath "$journal.err"
    if ($processus.ExitCode -ne 0) { throw "BIMedit a échoué ($($processus.ExitCode)) : $Arguments" }
}
$id = [guid]::NewGuid().ToString()
$depot = Join-Path $DepotParent $id
$local = Join-Path $CopieParent $id
New-Item -ItemType Directory -Path $local -Force | Out-Null
$a = Join-Path $local 'a.json'
$b = Join-Path $local 'b.json'
Copy-Item -LiteralPath "$PSScriptRoot\..\samples\maison_native.modele.json" -Destination $a
Copy-Item -LiteralPath $a -Destination $b
foreach ($copie in @($a, $b)) {
    Invoke-Bim @('--collaboration', 'associer', $copie, $depot)
}
$commandeA = Join-Path $local 'commande-a.json'
$commandeB = Join-Path $local 'commande-b.json'
'{"action":"quadrillage","nom":"QUALIFICATION-A","debut":[0,0],"fin":[1000,0]}' | Set-Content -LiteralPath $commandeA -Encoding utf8NoBOM
'{"action":"quadrillage","nom":"QUALIFICATION-B","debut":[0,1000],"fin":[1000,1000]}' | Set-Content -LiteralPath $commandeB -Encoding utf8NoBOM
Invoke-Bim @('--editer', $a, $commandeA)
Invoke-Bim @('--editer', $b, $commandeB)
$processus = foreach ($copie in @($a, $b)) {
    Start-Process -FilePath $Executable -ArgumentList @('--collaboration', 'synchroniser', ('"' + $copie + '"')) -PassThru -WindowStyle Hidden
}
foreach ($p in $processus) { $p.WaitForExit() }
# Une acquisition refusée sous contention est réessayée par une nouvelle opération
# complète. On ne réutilise jamais un ancien handle après l'échec.
foreach ($copie in @($a, $b)) {
    Invoke-Bim @('--collaboration', 'synchroniser', $copie)
}
# Après la barrière de publication, chacun reçoit la dernière révision.
foreach ($copie in @($a, $b)) { Invoke-Bim @('--collaboration', 'synchroniser', $copie) }
$pa = Get-Content -LiteralPath $a -Raw | ConvertFrom-Json
$pb = Get-Content -LiteralPath $b -Raw | ConvertFrom-Json
foreach ($p in @($pa,$pb)) {
    if (@($p.quadrillages | Where-Object nom -In @('QUALIFICATION-A','QUALIFICATION-B')).Count -ne 2) { throw 'Convergence incorrecte' }
    if (@($p.quadrillages.id | Sort-Object -Unique).Count -ne @($p.quadrillages).Count) { throw 'Collision' }
}
Invoke-Bim @('--collaboration', 'instantane', $a)
Invoke-Bim @('--collaboration', 'auditer', $depot)
$backup = Join-Path $CopieParent "$id-sauvegarde"
Invoke-Bim @('--collaboration', 'sauvegarde', $depot, $backup)
Invoke-Bim @('--collaboration', 'auditer', $backup)
Write-Output "Essais conservés : $local"
Write-Output "Dépôt : $depot"
Write-Output "Sauvegarde : $backup"
Write-Output 'Deux processus sur un même poste ne qualifient pas deux clients SMB ni une panne NAS.'
