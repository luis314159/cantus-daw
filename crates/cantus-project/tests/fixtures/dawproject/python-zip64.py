# (Run: python -I python-zip64.py python-zip64.zip — the fixture is committed; tests need no Python.)
# Writes a small ZIP64 archive with Python's own zipfile (an independent writer) for the
# DAWproject reader's test: every entry forced to ZIP64 records, one stored, one deflated.
import sys
import zipfile

# Every size, offset and count past these limits: ZIP64 records everywhere.
zipfile.ZIP64_LIMIT = 16
zipfile.ZIP_FILECOUNT_LIMIT = 1

out = sys.argv[1]
with zipfile.ZipFile(out, "w") as z:
    with z.open(zipfile.ZipInfo("project.xml"), "w", force_zip64=True) as f:
        f.write(b"<Project version=\"1.0\"/>")
    info = zipfile.ZipInfo("audio/take 1/vox.wav")
    info.compress_type = zipfile.ZIP_DEFLATED
    with z.open(info, "w", force_zip64=True) as f:
        f.write(bytes(range(256)) * 40)
