# Validates MusicXML files against the MusicXML 4.0 XSD with .NET's own validator.
#
#   pwsh tools/score/validate_musicxml.ps1 <xsd-dir> <file.musicxml>...
#
# <xsd-dir> holds musicxml.xsd, xlink.xsd and xml.xsd from github.com/w3c/musicxml
# (schema/); the imports are pointed at those local copies. Exits 1 on any error.
param([string]$XsdDir, [Parameter(ValueFromRemainingArguments = $true)][string[]]$Files)

$ErrorActionPreference = 'Stop'
$schemas = New-Object System.Xml.Schema.XmlSchemaSet
$xml = [System.Xml.Schema.XmlSchema]::Read([System.Xml.XmlReader]::Create((Join-Path $XsdDir 'xml.xsd')), $null)
$xlink = [System.Xml.Schema.XmlSchema]::Read([System.Xml.XmlReader]::Create((Join-Path $XsdDir 'xlink.xsd')), $null)
[void]$schemas.Add($xml)
[void]$schemas.Add($xlink)
$text = Get-Content -Raw (Join-Path $XsdDir 'musicxml.xsd')
$text = $text -replace 'schemaLocation="http://www.musicxml.org/xsd/xml.xsd"', '' -replace 'schemaLocation="http://www.musicxml.org/xsd/xlink.xsd"', ''
$main = [System.Xml.Schema.XmlSchema]::Read([System.Xml.XmlReader]::Create((New-Object System.IO.StringReader $text)), $null)
[void]$schemas.Add($main)
$schemas.Compile()

$bad = 0
foreach ($f in $Files) {
    $settings = New-Object System.Xml.XmlReaderSettings
    $settings.DtdProcessing = [System.Xml.DtdProcessing]::Ignore
    $settings.ValidationType = [System.Xml.ValidationType]::Schema
    $settings.Schemas = $schemas
    $errors = New-Object System.Collections.Generic.List[string]
    $settings.add_ValidationEventHandler({ param($s, $e) $errors.Add("$($e.Severity) line $($e.Exception.LineNumber): $($e.Message)") })
    $reader = [System.Xml.XmlReader]::Create($f, $settings)
    while ($reader.Read()) { }
    $reader.Close()
    if ($errors.Count -eq 0) {
        Write-Output "valid: $f"
    } else {
        $bad++
        Write-Output "INVALID: $f"
        $errors | Select-Object -First 10 | ForEach-Object { Write-Output "  $_" }
    }
}
if ($bad -gt 0) { exit 1 }
