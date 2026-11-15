// Names the raw addresses of a heap census file (cantus-rt's census.rs) from outside, with
// dbghelp attached to the running process (memmap.ps1 -Census): the census writes addresses
// when its own lookup finds no symbols.
using System;
using System.Runtime.InteropServices;
using System.Text;
using System.Text.RegularExpressions;
using System.Collections.Generic;

public static class CensusNames {
  [DllImport("kernel32.dll")] static extern IntPtr OpenProcess(uint a, bool i, int pid);
  [DllImport("dbghelp.dll", CharSet = CharSet.Unicode, SetLastError = true)] static extern bool SymInitializeW(IntPtr p, string path, bool invade);
  [DllImport("dbghelp.dll")] static extern uint SymSetOptions(uint o);
  [DllImport("dbghelp.dll", SetLastError = true)] static extern bool SymFromAddrW(IntPtr p, ulong a, out ulong disp, IntPtr sym);
  [DllImport("dbghelp.dll")] static extern bool SymCleanup(IntPtr p);

  public static string Name(string text, int pid, string searchPath) {
    IntPtr p = OpenProcess(0x0400 | 0x0010, false, pid);
    SymSetOptions(0x2 | 0x4 | 0x10);
    if (!SymInitializeW(p, searchPath, true)) return "SymInitialize failed: " + Marshal.GetLastWin32Error() + "\n" + text;
    IntPtr buf = Marshal.AllocHGlobal(88 + 2 * 1024);
    var known = new Dictionary<string, string>();
    string named = Regex.Replace(text, "0x[0-9a-f]{8,16}", m => {
      string r;
      if (known.TryGetValue(m.Value, out r)) return r;
      for (int i = 0; i < 88; i++) Marshal.WriteByte(buf, i, 0);
      Marshal.WriteInt32(buf, 0, 88);          // SizeOfStruct
      Marshal.WriteInt32(buf, 80, 1000);       // MaxNameLen
      ulong d;
      r = m.Value;
      if (SymFromAddrW(p, Convert.ToUInt64(m.Value.Substring(2), 16), out d, buf)) {
        int len = Marshal.ReadInt32(buf, 76);
        r = Marshal.PtrToStringUni(IntPtr.Add(buf, 84), Math.Min(len, 1000));
      }
      known[m.Value] = r;
      return r;
    });
    Marshal.FreeHGlobal(buf);
    SymCleanup(p);
    return named;
  }
}
