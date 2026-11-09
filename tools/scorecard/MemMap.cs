// What a Windows process's private memory is made of, read from outside (tools/scorecard/
// memmap.ps1): every committed region of its address space by kind — private (heap, stacks,
// a driver's buffers), mapped, image (a module's pages, private where they were written) —
// and the largest private allocations, which on the NT heap are the big buffers (one
// allocation of half a megabyte or more is a region of its own).
using System;
using System.Collections.Generic;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text;

public static class MemMap {
  [StructLayout(LayoutKind.Sequential)]
  struct MBI { public ulong BaseAddress, AllocationBase; public uint AllocationProtect; public uint pad1; public ulong RegionSize; public uint State, Protect, Type, pad2; }
  [DllImport("kernel32.dll")] static extern IntPtr OpenProcess(uint a, bool i, int pid);
  [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr h);
  [DllImport("kernel32.dll")] static extern UIntPtr VirtualQueryEx(IntPtr p, ulong a, out MBI m, UIntPtr n);
  [DllImport("psapi.dll", CharSet = CharSet.Unicode)] static extern uint GetMappedFileNameW(IntPtr p, ulong a, StringBuilder s, uint n);

  const uint MEM_COMMIT = 0x1000, MEM_PRIVATE = 0x20000, MEM_MAPPED = 0x40000, MEM_IMAGE = 0x1000000;

  public static string Report(int pid, int top) {
    IntPtr p = OpenProcess(0x0400 | 0x0010, false, pid);
    if (p == IntPtr.Zero) return "cannot open process";
    var sb = new StringBuilder();
    ulong addr = 0; MBI m;
    ulong priv = 0, mapped = 0, image = 0, imageWritten = 0;
    var allocs = new Dictionary<ulong, ulong>();   // allocation base -> committed private bytes
    var perModule = new Dictionary<string, ulong>(); // image pages that are copy-on-write/written
    while (VirtualQueryEx(p, addr, out m, (UIntPtr)Marshal.SizeOf(typeof(MBI))) != UIntPtr.Zero) {
      if (m.State == MEM_COMMIT) {
        if (m.Type == MEM_PRIVATE) {
          priv += m.RegionSize;
          ulong v; allocs.TryGetValue(m.AllocationBase, out v); allocs[m.AllocationBase] = v + m.RegionSize;
        } else if (m.Type == MEM_MAPPED) mapped += m.RegionSize;
        else if (m.Type == MEM_IMAGE) {
          image += m.RegionSize;
          // Writable image pages (.data, relocated) are private to the process.
          if ((m.Protect & 0x04) != 0 || (m.Protect & 0x40) != 0) {
            var name = new StringBuilder(512); GetMappedFileNameW(p, m.BaseAddress, name, 512);
            var n = System.IO.Path.GetFileName(name.ToString());
            ulong v; perModule.TryGetValue(n, out v); perModule[n] = v + m.RegionSize;
            imageWritten += m.RegionSize;
          }
        }
      }
      ulong next = m.BaseAddress + m.RegionSize;
      if (next <= addr) break;
      addr = next;
    }
    sb.AppendLine(string.Format("committed private {0:F1} MB, mapped {1:F1} MB, image {2:F1} MB (writable image {3:F1} MB)", priv / 1048576.0, mapped / 1048576.0, image / 1048576.0, imageWritten / 1048576.0));
    var sizes = allocs.Values.OrderByDescending(x => x).ToList();
    ulong big = sizes.Where(x => x >= 512 * 1024).Aggregate(0UL, (a, b) => a + b);
    sb.AppendLine(string.Format("private allocations: {0}, of 512 KB or more: {1} holding {2:F1} MB", sizes.Count, sizes.Count(x => x >= 512 * 1024), big / 1048576.0));
    sb.AppendLine("largest private allocations (MB):");
    foreach (var kv in allocs.OrderByDescending(kv => kv.Value).Take(top))
      sb.AppendLine(string.Format("  {0,8:F2}  at 0x{1:x}", kv.Value / 1048576.0, kv.Key));
    sb.AppendLine("written image pages by module (MB):");
    foreach (var kv in perModule.OrderByDescending(kv => kv.Value).Take(10))
      sb.AppendLine(string.Format("  {0,8:F2}  {1}", kv.Value / 1048576.0, kv.Key));
    // A histogram of private allocation sizes.
    sb.AppendLine("private bytes by allocation size:");
    ulong[] edges = { 0, 64UL << 10, 512UL << 10, 4UL << 20, 32UL << 20, ulong.MaxValue };
    for (int k = 0; k + 1 < edges.Length; k++) {
      ulong lo = edges[k], hi = edges[k + 1];
      var s = sizes.Where(x => x >= lo && x < hi).ToList();
      sb.AppendLine(string.Format("  [{0,8} KB, {1,8}): {2,5} allocations, {3,7:F1} MB", lo / 1024, hi == ulong.MaxValue ? "inf" : (hi / 1024).ToString() + " KB", s.Count, s.Aggregate(0UL, (a, b) => a + b) / 1048576.0));
    }
    CloseHandle(p);
    return sb.ToString();
  }
}
