# PT-P710BT Bluetooth-Diagnose
# Listet Bluetooth-COM-Ports und fragt den Druckerstatus direkt ab (ESC i S).
# Ausfuehren in PowerShell:  powershell -ExecutionPolicy Bypass -File .\PT-P710BT_BT-Test.ps1

Write-Host "`n=== 1) Bluetooth-Geraete PT-P710BT ===" -ForegroundColor Cyan
Get-PnpDevice -ErrorAction SilentlyContinue |
    Where-Object { $_.FriendlyName -like '*P710*' } |
    Format-Table Status, Class, FriendlyName, InstanceId -AutoSize

Write-Host "=== 2) COM-Ports ueber Bluetooth ===" -ForegroundColor Cyan
$ports = Get-CimInstance Win32_PnPEntity |
    Where-Object { $_.Name -match '\(COM\d+\)' -and $_.PNPDeviceID -like 'BTHENUM*' }
if (-not $ports) {
    Write-Host "KEIN Bluetooth-COM-Port vorhanden!" -ForegroundColor Red
    Write-Host "-> Systemsteuerung > Geraete und Drucker > PT-P710BT > Eigenschaften > Dienste > 'Serieller Anschluss (SPP)' anhaken"
    Write-Host "   oder: 'Weitere Bluetooth-Einstellungen' > COM-Anschluesse > Hinzufuegen > Ausgehend > PT-P710BT"
    return
}
$ports | ForEach-Object {
    $outgoing = $_.PNPDeviceID -notmatch '&0&000000000000_'
    [pscustomobject]@{ Name = $_.Name; Richtung = $(if ($outgoing) {'ausgehend'} else {'EINGEHEND (unbrauchbar)'}); ID = $_.PNPDeviceID }
} | Format-Table -AutoSize

Write-Host "=== 3) Statusabfrage am Drucker ===" -ForegroundColor Cyan
foreach ($p in $ports) {
    if ($p.PNPDeviceID -match '&0&000000000000_') { continue }   # eingehende Ports ueberspringen
    $com = [regex]::Match($p.Name, 'COM\d+').Value
    Write-Host "Teste $com ..." -NoNewline
    $sp = New-Object System.IO.Ports.SerialPort $com, 9600
    $sp.ReadTimeout = 8000; $sp.WriteTimeout = 8000
    try {
        $sp.Open()
        $cmd = [byte[]](@(0) * 100) + [byte[]](0x1B, 0x40) + [byte[]](0x1B, 0x69, 0x53)  # Invalidate, Init, Status
        $sp.Write($cmd, 0, $cmd.Length)
        $buf = New-Object byte[] 32; $n = 0
        $sw = [Diagnostics.Stopwatch]::StartNew()
        while ($n -lt 32 -and $sw.ElapsedMilliseconds -lt 8000) {
            if ($sp.BytesToRead -gt 0) { $n += $sp.Read($buf, $n, 32 - $n) } else { Start-Sleep -Milliseconds 50 }
        }
        if ($n -eq 32 -and $buf[0] -eq 0x80) {
            Write-Host " OK!" -ForegroundColor Green
            Write-Host ("  Band: {0} mm, Fehler1: 0x{1:X2}, Fehler2: 0x{2:X2}" -f $buf[10], $buf[8], $buf[9])
            Write-Host "  -> Diesen Port ($com) im Druckertreiber unter 'Anschluesse' auswaehlen."
        } else {
            Write-Host " keine/ungueltige Antwort ($n Bytes)" -ForegroundColor Yellow
        }
    } catch {
        Write-Host " Fehler: $($_.Exception.Message)" -ForegroundColor Red
    } finally { if ($sp.IsOpen) { $sp.Close() } }
}

Write-Host "`n=== 4) Treiber-Anschluss ===" -ForegroundColor Cyan
Get-Printer | Where-Object { $_.DriverName -like '*P710*' } | Format-Table Name, PortName, PrinterStatus -AutoSize
