# Calls ElevenLabs' API with the key kept in Windows Credential Manager (generic target
# "cantus-elevenlabs", user "api" — the owner's Starter plan, bought 2026-11-21 for the launch
# video's narration). The key is read at the moment of use, sent only in the xi-api-key header,
# and never printed or written anywhere.
#   eleven-api.ps1 GET  v1/user/subscription
#   eleven-api.ps1 GET  v1/voices
#   eleven-api.ps1 POST v1/text-to-speech/<voice_id>?output_format=wav_48000 -BodyFile line.json -OutFile take.wav
# A body is passed as a file (UTF-8 JSON), so the script's lines never sit on a command line.
param(
    [Parameter(Mandatory)][string]$Method,
    [Parameter(Mandatory)][string]$Path,
    [string]$BodyFile,
    [string]$OutFile
)
$ErrorActionPreference = 'Stop'
if (-not ('CantusCred' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class CantusCred {
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    struct CREDENTIAL {
        public int Flags; public int Type; public string TargetName; public string Comment;
        public long LastWritten; public int CredentialBlobSize; public IntPtr CredentialBlob;
        public int Persist; public int AttributeCount; public IntPtr Attributes;
        public string TargetAlias; public string UserName;
    }
    [DllImport("advapi32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    static extern bool CredReadW(string target, int type, int flags, out IntPtr cred);
    [DllImport("advapi32.dll")] static extern void CredFree(IntPtr cred);
    public static string Read(string target) {
        IntPtr p;
        if (!CredReadW(target, 1, 0, out p)) return null;
        try {
            var c = (CREDENTIAL)Marshal.PtrToStructure(p, typeof(CREDENTIAL));
            // cmdkey stores the blob as UTF-16; a key written as bytes reads as ASCII.
            var s = Marshal.PtrToStringUni(c.CredentialBlob, c.CredentialBlobSize / 2);
            if (s.Length > 0 && s[0] > 0x7f) {
                s = Marshal.PtrToStringAnsi(c.CredentialBlob, c.CredentialBlobSize);
            }
            return s;
        } finally { CredFree(p); }
    }
}
'@
}
$key = [CantusCred]::Read('cantus-elevenlabs')
if (-not $key) { throw 'No credential "cantus-elevenlabs" in Windows Credential Manager.' }
$headers = @{ 'xi-api-key' = $key; 'User-Agent' = 'cantus-video-tools' }
$uri = "https://api.elevenlabs.io/$($Path.TrimStart('/'))"
$req = @{ Method = $Method; Uri = $uri; Headers = $headers }
if ($BodyFile) {
    $req.Body = [System.IO.File]::ReadAllBytes((Resolve-Path $BodyFile))
    $req.ContentType = 'application/json; charset=utf-8'
}
if ($OutFile) {
    Invoke-WebRequest @req -OutFile $OutFile | Out-Null
    "saved $OutFile"
} else {
    $r = Invoke-WebRequest @req
    if ($r.Content -is [byte[]]) { [System.Text.Encoding]::UTF8.GetString($r.Content) } else { $r.Content }
}
