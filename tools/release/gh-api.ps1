# Calls GitHub's REST API for cantus-daw with the token kept in Windows Credential Manager
# (generic target "cantus-daw-github"). The token is read at the moment of use, passed only
# in the request header, and never printed or written anywhere.
#   gh-api.ps1 GET  repos/luis314159/cantus-daw/actions/runs/123/jobs
#   gh-api.ps1 PATCH repos/luis314159/cantus-daw/actions/variables/X '{"name":"X","value":"false"}'
#   gh-api.ps1 GET  <path> -OutFile log.txt     (binary or redirected downloads, e.g. job logs)
param(
    [Parameter(Mandatory)][string]$Method,
    [Parameter(Mandatory)][string]$Path,
    [string]$Body,
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
            return Marshal.PtrToStringUni(c.CredentialBlob, c.CredentialBlobSize / 2);
        } finally { CredFree(p); }
    }
}
'@
}
$token = [CantusCred]::Read('cantus-daw-github')
if (-not $token) { throw 'No credential "cantus-daw-github" in Windows Credential Manager.' }
$headers = @{
    Authorization          = "Bearer $token"
    Accept                 = 'application/vnd.github+json'
    'X-GitHub-Api-Version' = '2022-11-28'
    'User-Agent'           = 'cantus-release-tools'
}
$uri = if ($Path -match '^https://') { $Path } else { "https://api.github.com/$($Path.TrimStart('/'))" }
$req = @{ Method = $Method; Uri = $uri; Headers = $headers }
if ($Body) { $req.Body = $Body; $req.ContentType = 'application/json' }
if ($OutFile) {
    Invoke-WebRequest @req -UseBasicParsing -OutFile $OutFile | Out-Null
    "saved $OutFile"
} else {
    $r = Invoke-WebRequest @req -UseBasicParsing
    if ($r.Content) { $r.Content } else { "HTTP $($r.StatusCode)" }
}
