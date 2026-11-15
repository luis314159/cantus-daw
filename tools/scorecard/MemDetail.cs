// A VMMap-style account of a Windows process's private bytes, read from outside
// (tools/scorecard/memmap.ps1 -Detail; round 40, ADR 0120). The commit charge a process's
// "private bytes" counts is three things, and every byte of it is put under an owner here:
//
// - committed MEM_PRIVATE regions: thread stacks (each thread's TEB names its stack's
//   allocation; the thread is named by its description and its start module), TEBs and the
//   PEB, heap segments (the NT heap's segment signature names the heap; every heap's
//   committed and allocated bytes come from RtlQueryProcessDebugInformation's heap summary),
//   the heaps' big blocks (one VirtualAlloc'd region each, chained to their heap), and the
//   rest — memory a driver or a library took with VirtualAlloc. For those the report names
//   the module whose data holds a pointer into the region, the usual owner;
// - writable image pages (copy-on-write sections, charged whether written or not), by module;
// - copy-on-write mapped views.
//
// The total is checked against GetProcessMemoryInfo's PrivateUsage.
using System;
using System.Collections.Generic;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text;

public static class MemDetail {
  [StructLayout(LayoutKind.Sequential)]
  struct MBI { public ulong BaseAddress, AllocationBase; public uint AllocationProtect; public uint pad1; public ulong RegionSize; public uint State, Protect, Type, pad2; }
  [StructLayout(LayoutKind.Sequential)]
  struct TBI { public int ExitStatus; public int pad; public ulong TebBaseAddress, Pid, Tid, Affinity; public int Priority, BasePriority; }
  [StructLayout(LayoutKind.Sequential)]
  struct PMC { public uint cb, PageFaultCount; public ulong PeakWorkingSetSize, WorkingSetSize, QuotaPeakPagedPoolUsage, QuotaPagedPoolUsage, QuotaPeakNonPagedPoolUsage, QuotaNonPagedPoolUsage, PagefileUsage, PeakPagefileUsage, PrivateUsage, PrivateWorkingSetSize, SharedCommitUsage; }
  [StructLayout(LayoutKind.Sequential)]
  struct MODINFO { public ulong lpBaseOfDll; public uint SizeOfImage; public uint pad; public ulong EntryPoint; }

  [DllImport("kernel32.dll")] static extern IntPtr OpenProcess(uint a, bool i, int pid);
  [DllImport("kernel32.dll")] static extern IntPtr OpenThread(uint a, bool i, int tid);
  [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr h);
  [DllImport("kernel32.dll")] static extern UIntPtr VirtualQueryEx(IntPtr p, ulong a, out MBI m, UIntPtr n);
  [DllImport("kernel32.dll")] static extern bool ReadProcessMemory(IntPtr p, ulong a, byte[] b, UIntPtr n, out UIntPtr r);
  [DllImport("kernel32.dll")] static extern int GetThreadDescription(IntPtr t, out IntPtr d);
  [DllImport("kernel32.dll")] static extern IntPtr LocalFree(IntPtr m);
  [DllImport("ntdll.dll")] static extern int NtQueryInformationThread(IntPtr t, int c, out TBI i, int n, IntPtr r);
  [DllImport("ntdll.dll")] static extern int NtQueryInformationThread(IntPtr t, int c, out ulong i, int n, IntPtr r);
  [DllImport("ntdll.dll")] static extern IntPtr RtlCreateQueryDebugBuffer(uint size, bool eventPair);
  [DllImport("ntdll.dll")] static extern int RtlQueryProcessDebugInformation(IntPtr pid, uint flags, IntPtr buffer);
  [DllImport("ntdll.dll")] static extern int RtlDestroyQueryDebugBuffer(IntPtr buffer);
  [DllImport("psapi.dll")] static extern bool GetProcessMemoryInfo(IntPtr p, out PMC c, uint cb);
  [DllImport("psapi.dll")] static extern bool EnumProcessModulesEx(IntPtr p, [Out] IntPtr[] m, uint cb, out uint needed, uint filter);
  [DllImport("psapi.dll", CharSet = CharSet.Unicode)] static extern uint GetModuleFileNameExW(IntPtr p, IntPtr m, StringBuilder s, uint n);
  [DllImport("psapi.dll")] static extern bool GetModuleInformation(IntPtr p, IntPtr m, out MODINFO i, uint cb);

  const uint MEM_COMMIT = 0x1000, MEM_PRIVATE = 0x20000, MEM_MAPPED = 0x40000, MEM_IMAGE = 0x1000000;

  static bool Charged(uint protect) {
    uint p = protect & 0xff;
    return p == 0x04 || p == 0x08 || p == 0x40 || p == 0x80;
  }
  static bool CopyOnWrite(uint protect) { uint p = protect & 0xff; return p == 0x08 || p == 0x80; }

  static ulong ReadU64(IntPtr p, ulong a) {
    var b = new byte[8]; UIntPtr r;
    if (!ReadProcessMemory(p, a, b, (UIntPtr)8, out r)) return 0;
    return BitConverter.ToUInt64(b, 0);
  }
  static uint ReadU32(IntPtr p, ulong a) {
    var b = new byte[4]; UIntPtr r;
    if (!ReadProcessMemory(p, a, b, (UIntPtr)4, out r)) return 0;
    return BitConverter.ToUInt32(b, 0);
  }

  class Region { public ulong Base, Size, AllocBase; public uint Protect, Type; }
  class Module { public ulong Base, End; public string Name; }
  class Owner { public string Kind, Name; }

  static string Mb(ulong b) { return string.Format("{0,8:F2}", b / 1048576.0); }

  // The heaps' summary from inside the process (a thread ntdll starts there for it).
  // RTL_DEBUG_INFORMATION.Heaps at 0x70; RTL_PROCESS_HEAPS { ULONG n; RTL_HEAP_INFORMATION[n] at 8 },
  // each 0x58 bytes: BaseAddress 0, Flags 8, BytesAllocated 0x10, BytesCommitted 0x18, NumberOfEntries 0x24.
  static List<ulong[]> HeapSummary(int pid, StringBuilder note) {
    var heaps = new List<ulong[]>();
    IntPtr buf = RtlCreateQueryDebugBuffer(0, false);
    if (buf == IntPtr.Zero) { note.AppendLine("  (no debug buffer)"); return heaps; }
    int st = RtlQueryProcessDebugInformation((IntPtr)pid, 0x04, buf);
    if (st != 0) { note.AppendLine(string.Format("  (heap summary failed: 0x{0:x})", st)); RtlDestroyQueryDebugBuffer(buf); return heaps; }
    IntPtr h = Marshal.ReadIntPtr(buf, 0x70);
    if (h != IntPtr.Zero) {
      int n = Marshal.ReadInt32(h, 0);
      // RTL_HEAP_INFORMATION is 0x58 bytes before Windows 10 and 0x60 (V2, a HeapTag at the
      // end) after: the stride whose every entry names a heap aligned to 64 KB is the one.
      int stride = 0x60;
      foreach (int s in new[] { 0x60, 0x58 }) {
        bool ok = true;
        for (int i = 0; i < n && i < 4096; i++) {
          ulong b = (ulong)Marshal.ReadInt64(IntPtr.Add(h, 8 + i * s), 0);
          if (b == 0 || (b & 0xffff) != 0) { ok = false; break; }
        }
        if (ok) { stride = s; break; }
      }
      for (int i = 0; i < n && i < 4096; i++) {
        IntPtr e = IntPtr.Add(h, 8 + i * stride);
        heaps.Add(new ulong[] {
          (ulong)Marshal.ReadInt64(e, 0), (ulong)(uint)Marshal.ReadInt32(e, 8),
          (ulong)Marshal.ReadInt64(e, 0x10), (ulong)Marshal.ReadInt64(e, 0x18), (ulong)(uint)Marshal.ReadInt32(e, 0x24) });
      }
    }
    RtlDestroyQueryDebugBuffer(buf);
    return heaps;
  }

  static List<Module> Modules(IntPtr p) {
    var mods = new List<Module>();
    var hs = new IntPtr[2048]; uint needed;
    if (!EnumProcessModulesEx(p, hs, (uint)(hs.Length * IntPtr.Size), out needed, 3)) return mods;
    int n = Math.Min(hs.Length, (int)(needed / IntPtr.Size));
    for (int i = 0; i < n; i++) {
      var sb = new StringBuilder(1024); GetModuleFileNameExW(p, hs[i], sb, 1024);
      MODINFO mi; GetModuleInformation(p, hs[i], out mi, (uint)Marshal.SizeOf(typeof(MODINFO)));
      mods.Add(new Module { Base = mi.lpBaseOfDll, End = mi.lpBaseOfDll + mi.SizeOfImage, Name = System.IO.Path.GetFileName(sb.ToString()) });
    }
    return mods;
  }

  static string ModuleAt(List<Module> mods, ulong a) {
    foreach (var m in mods) if (a >= m.Base && a < m.End) return m.Name;
    return "?";
  }

  public static string Report(int pid, int[] tids, int top) {
    IntPtr p = OpenProcess(0x0400 | 0x0010, false, pid);
    if (p == IntPtr.Zero) return "cannot open process";
    var sb = new StringBuilder();
    var regions = new List<Region>();
    ulong addr = 0; MBI m;
    while (VirtualQueryEx(p, addr, out m, (UIntPtr)Marshal.SizeOf(typeof(MBI))) != UIntPtr.Zero) {
      if (m.State == MEM_COMMIT)
        regions.Add(new Region { Base = m.BaseAddress, Size = m.RegionSize, AllocBase = m.AllocationBase, Protect = m.Protect, Type = m.Type });
      ulong next = m.BaseAddress + m.RegionSize;
      if (next <= addr) break;
      addr = next;
    }
    var mods = Modules(p);
    PMC pmc; pmc.cb = (uint)Marshal.SizeOf(typeof(PMC));
    GetProcessMemoryInfo(p, out pmc, pmc.cb);

    // Committed private bytes by allocation (an allocation is one VirtualAlloc reservation).
    var alloc = new Dictionary<ulong, ulong>();
    var allocProt = new Dictionary<ulong, uint>();
    foreach (var r in regions) if (r.Type == MEM_PRIVATE) {
      ulong v; alloc.TryGetValue(r.AllocBase, out v); alloc[r.AllocBase] = v + r.Size;
      uint q; allocProt.TryGetValue(r.AllocBase, out q); allocProt[r.AllocBase] = q | (r.Protect & 0x7ff);
    }
    var owner = new Dictionary<ulong, Owner>();
    Func<ulong, ulong> allocOf = a => {
      MBI mm; if (VirtualQueryEx(p, a, out mm, (UIntPtr)Marshal.SizeOf(typeof(MBI))) == UIntPtr.Zero) return 0;
      return mm.AllocationBase;
    };

    // Threads: each one's stack allocation and its TEB.
    var threadLines = new List<KeyValuePair<ulong, string>>();
    foreach (int tid in tids) {
      IntPtr t = OpenThread(0x0040 | 0x0800, false, tid);   // QUERY_INFORMATION | QUERY_LIMITED_INFORMATION
      if (t == IntPtr.Zero) continue;
      TBI tbi;
      string name = "";
      IntPtr d;
      if (GetThreadDescription(t, out d) >= 0 && d != IntPtr.Zero) { name = Marshal.PtrToStringUni(d) ?? ""; LocalFree(d); }
      ulong start;
      string startMod = NtQueryInformationThread(t, 9, out start, 8, IntPtr.Zero) == 0 ? ModuleAt(mods, start) : "?";
      if (NtQueryInformationThread(t, 0, out tbi, Marshal.SizeOf(typeof(TBI)), IntPtr.Zero) == 0) {
        ulong teb = tbi.TebBaseAddress;
        ulong stackBase = ReadU64(p, teb + 0x08), stackLimit = ReadU64(p, teb + 0x10), dealloc = ReadU64(p, teb + 0x1478);
        string label = string.Format("{0} [{1}] tid {2}", name == "" ? "(unnamed)" : name, startMod, tid);
        if (dealloc != 0) owner[dealloc] = new Owner { Kind = "stack", Name = label };
        ulong tebAlloc = allocOf(teb);
        if (tebAlloc != 0 && !owner.ContainsKey(tebAlloc)) owner[tebAlloc] = new Owner { Kind = "teb", Name = "TEBs" };
        ulong committed; alloc.TryGetValue(dealloc, out committed);
        MBI sm; ulong reserved = 0;
        // The reservation: from DeallocationStack to StackBase.
        if (VirtualQueryEx(p, dealloc, out sm, (UIntPtr)Marshal.SizeOf(typeof(MBI))) != UIntPtr.Zero) reserved = stackBase - dealloc;
        threadLines.Add(new KeyValuePair<ulong, string>(committed, string.Format("  {0}  (reserved {1:F2} MB, in use {2} KB)  {3}", Mb(committed), reserved / 1048576.0, (stackBase - stackLimit) / 1024, label)));
      }
      CloseHandle(t);
    }

    // Heaps: the summary, then each segment by its signature and owning heap.
    var note = new StringBuilder();
    var heaps = HeapSummary(pid, note);
    var heapBases = new HashSet<ulong>(heaps.Select(h => h[0]));
    var heapSeg = new Dictionary<ulong, ulong>();   // heap -> committed segment bytes
    var heapBig = new Dictionary<ulong, ulong>();   // heap -> committed big-block bytes
    var heapBigN = new Dictionary<ulong, int>();
    foreach (var kv in alloc) {
      if (owner.ContainsKey(kv.Key)) continue;
      ulong heap = 0;
      if (heapBases.Contains(kv.Key)) heap = kv.Key;
      else if (ReadU32(p, kv.Key + 0x10) == 0xffeeffee) heap = ReadU64(p, kv.Key + 0x28);
      if (heap != 0 && heapBases.Contains(heap)) {
        owner[kv.Key] = new Owner { Kind = "heap", Name = string.Format("0x{0:x}", heap) };
        ulong v; heapSeg.TryGetValue(heap, out v); heapSeg[heap] = v + kv.Value;
      }
    }
    // A heap's big blocks: the list in its header whose nodes lie in private allocations no
    // segment owns (VirtualAllocdBlocks), found by walking every LIST_ENTRY of the header.
    foreach (ulong hb in heapBases) {
      for (ulong o = 0x40; o < 0x300; o += 8) {
        ulong head = hb + o, f = ReadU64(p, head), prev = head;
        if (f == 0 || f == head) continue;
        var nodes = new List<ulong>(); bool ok = true; int steps = 0;
        while (f != head) {
          if (steps++ > 20000 || ReadU64(p, f + 8) != prev) { ok = false; break; }
          ulong ab = allocOf(f);
          if (ab == 0 || !alloc.ContainsKey(ab) || (owner.ContainsKey(ab) && owner[ab].Kind != "heapbig")) { ok = false; break; }
          nodes.Add(ab); prev = f; f = ReadU64(p, f);
        }
        if (!ok || nodes.Count == 0) continue;
        foreach (ulong b in nodes) {
          if (owner.ContainsKey(b)) continue;
          owner[b] = new Owner { Kind = "heapbig", Name = string.Format("0x{0:x}", hb) };
          ulong v; heapBig.TryGetValue(hb, out v); heapBig[hb] = v + alloc[b];
          int c; heapBigN.TryGetValue(hb, out c); heapBigN[hb] = c + 1;
        }
      }
    }

    // The rest: who points at it. Every charged image page (a module's data) and every heap
    // region is read and each pointer-sized value that falls inside an unowned allocation
    // names the module (or "a heap block") as a likely owner.
    var allocEnd = new Dictionary<ulong, ulong>();
    foreach (var r in regions) if (r.Type == MEM_PRIVATE) {
      ulong e; allocEnd.TryGetValue(r.AllocBase, out e); allocEnd[r.AllocBase] = Math.Max(e, r.Base + r.Size);
    }
    var unknown = alloc.Keys.Where(k => !owner.ContainsKey(k)).OrderBy(k => k).ToArray();
    var unknownEnd = unknown.Select(k => allocEnd[k]).ToArray();
    var hints = new Dictionary<ulong, Dictionary<string, int>>();
    var heapHints = new Dictionary<ulong, Dictionary<string, int>>();
    Action<ulong, ulong, string> scan = (b, size, src) => {
      const ulong chunk = 1 << 20;
      for (ulong a = b; a < b + size; a += chunk) {
        ulong n = Math.Min(chunk, b + size - a);
        var buf = new byte[n]; UIntPtr got;
        if (!ReadProcessMemory(p, a, buf, (UIntPtr)n, out got)) continue;
        for (int i = 0; i + 8 <= (int)got; i += 8) {
          ulong v = BitConverter.ToUInt64(buf, i);
          if (v < 0x10000 || v > 0x7fffffffffff) continue;
          if (heapBases.Contains(v) && src != "a heap block") {
            Dictionary<string, int> hh;
            if (!heapHints.TryGetValue(v, out hh)) { hh = new Dictionary<string, int>(); heapHints[v] = hh; }
            int cc; hh.TryGetValue(src, out cc); hh[src] = cc + 1;
          }
          int j = Array.BinarySearch(unknown, v);
          if (j < 0) j = ~j - 1;
          if (j < 0 || v >= unknownEnd[j]) continue;
          if (a + (ulong)i >= unknown[j] && a + (ulong)i < unknownEnd[j]) continue;   // itself
          Dictionary<string, int> h;
          if (!hints.TryGetValue(unknown[j], out h)) { h = new Dictionary<string, int>(); hints[unknown[j]] = h; }
          int c; h.TryGetValue(src, out c); h[src] = c + 1;
        }
      }
    };
    foreach (var r in regions) {
      if (r.Type == MEM_IMAGE && Charged(r.Protect) && !CopyOnWrite(r.Protect)) scan(r.Base, r.Size, ModuleAt(mods, r.Base));
      else if (r.Type == MEM_PRIVATE && owner.ContainsKey(r.AllocBase) && (owner[r.AllocBase].Kind == "heap" || owner[r.AllocBase].Kind == "heapbig") && (r.Protect & 0xff) == 0x04) scan(r.Base, r.Size, "a heap block");
    }

    // Image and mapped pages the commit charge counts.
    var imageBy = new Dictionary<string, ulong>(); var imageWritten = new Dictionary<string, ulong>();
    ulong image = 0, mappedCow = 0, mappedOther = 0;
    foreach (var r in regions) {
      if (r.Type == MEM_IMAGE && Charged(r.Protect)) {
        string n = ModuleAt(mods, r.Base);
        ulong v; imageBy.TryGetValue(n, out v); imageBy[n] = v + r.Size; image += r.Size;
        if (!CopyOnWrite(r.Protect)) { imageWritten.TryGetValue(n, out v); imageWritten[n] = v + r.Size; }
      } else if (r.Type == MEM_MAPPED) {
        if (CopyOnWrite(r.Protect)) mappedCow += r.Size; else mappedOther += r.Size;
      }
    }
    ulong priv = alloc.Values.Aggregate(0UL, (a, b) => a + b);
    // Page tables (charged to the process too): one 4 KB table per 2 MB of address space with
    // a committed page in it, one directory per 1 GB, an estimate.
    var pt = new HashSet<ulong>(); var pd = new HashSet<ulong>();
    foreach (var r in regions) for (ulong a = r.Base & ~0x1fffffUL; a < r.Base + r.Size; a += 0x200000) { pt.Add(a); pd.Add(a >> 30); }
    ulong tables = (ulong)(pt.Count + pd.Count) * 4096;
    Func<string, ulong> kind = k => owner.Where(kv => kv.Value.Kind == k && alloc.ContainsKey(kv.Key)).Aggregate(0UL, (a, kv) => a + alloc[kv.Key]);
    ulong stacks = kind("stack"), tebs = kind("teb"), segs = kind("heap"), bigs = kind("heapbig");
    ulong rest = unknown.Aggregate(0UL, (a, k) => a + alloc[k]);

    sb.AppendLine(string.Format("PrivateUsage {0} MB = private committed {1} + charged image {2} + copy-on-write views {3} + page tables ~{7}  (sum {4}; working set {5}, private working set {6})",
      Mb(pmc.PrivateUsage).Trim(), Mb(priv).Trim(), Mb(image).Trim(), Mb(mappedCow).Trim(), Mb(priv + image + mappedCow + tables).Trim(), Mb(pmc.WorkingSetSize).Trim(), Mb(pmc.PrivateWorkingSetSize).Trim(), Mb(tables).Trim()));
    sb.AppendLine(string.Format("  heaps {0} (segments {1}, big blocks {2}) | stacks {3} | TEBs/PEB {4} | other private {5} | image {6} | views {7}",
      Mb(segs + bigs).Trim(), Mb(segs).Trim(), Mb(bigs).Trim(), Mb(stacks).Trim(), Mb(tebs).Trim(), Mb(rest).Trim(), Mb(image).Trim(), Mb(mappedCow).Trim()));
    if (note.Length > 0) sb.Append(note);
    sb.AppendLine("heaps (committed in segments + big blocks; the heap's own count of allocated / committed):");
    foreach (var h in heaps.OrderByDescending(h => { ulong a, b; heapSeg.TryGetValue(h[0], out a); heapBig.TryGetValue(h[0], out b); return a + b; })) {
      ulong s, b; int n; heapSeg.TryGetValue(h[0], out s); heapBig.TryGetValue(h[0], out b); heapBigN.TryGetValue(h[0], out n);
      if (s + b < 64 * 1024 && h[3] < 64 * 1024) continue;
      // Who made the heap: the module whose data points at its base.
      Dictionary<string, int> hh; string made = "";
      if (heapHints.TryGetValue(h[0], out hh)) made = "; named by " + string.Join(", ", hh.OrderByDescending(x => x.Value).Take(4).Select(x => x.Key + " ×" + x.Value));
      sb.AppendLine(string.Format("  {0}  0x{1:x} flags 0x{2:x}: segments {3}, {4} big blocks {5}; allocated {6} of committed {7}{8}",
        Mb(s + b), h[0], h[1], Mb(s).Trim(), n, Mb(b).Trim(), Mb(h[2]).Trim(), Mb(h[3]).Trim(), made));
    }
    sb.AppendLine("the heaps' big blocks (MB, heap):");
    foreach (var kv in owner.Where(kv => kv.Value.Kind == "heapbig" && alloc.ContainsKey(kv.Key)).OrderByDescending(kv => alloc[kv.Key]))
      sb.AppendLine(string.Format("  {0}  0x{1:x} of heap {2}", Mb(alloc[kv.Key]), kv.Key, kv.Value.Name));
    sb.AppendLine("thread stacks (committed):");
    foreach (var kv in threadLines.OrderByDescending(kv => kv.Key)) sb.AppendLine(kv.Value);
    sb.AppendLine(string.Format("other private allocations, largest {0} (MB, protection, who points at it):", top));
    foreach (ulong k in unknown.OrderByDescending(k => alloc[k]).Take(top)) {
      Dictionary<string, int> h; string who = "";
      if (hints.TryGetValue(k, out h)) who = string.Join(", ", h.OrderByDescending(x => x.Value).Take(4).Select(x => x.Key + " ×" + x.Value));
      sb.AppendLine(string.Format("  {0}  0x{1:x} prot 0x{2:x}  {3}", Mb(alloc[k]), k, allocProt[k], who));
    }
    // Other private, summed by its most frequent pointer source.
    var byHint = new Dictionary<string, ulong>();
    foreach (ulong k in unknown) {
      Dictionary<string, int> h; string who = "(nobody found)";
      if (hints.TryGetValue(k, out h)) who = h.OrderByDescending(x => x.Value).First().Key;
      ulong v; byHint.TryGetValue(who, out v); byHint[who] = v + alloc[k];
    }
    sb.AppendLine("other private by the likeliest owner (MB):");
    foreach (var kv in byHint.OrderByDescending(kv => kv.Value).Take(top)) sb.AppendLine(string.Format("  {0}  {1}", Mb(kv.Value), kv.Key));
    sb.AppendLine("other private by allocation size:");
    ulong[] edges = { 0, 64UL << 10, 512UL << 10, 4UL << 20, 32UL << 20, ulong.MaxValue };
    for (int e = 0; e + 1 < edges.Length; e++) {
      var s = unknown.Select(k => alloc[k]).Where(x => x >= edges[e] && x < edges[e + 1]).ToList();
      sb.AppendLine(string.Format("  [{0,6} KB, {1,8}): {2,5} allocations, {3} MB", edges[e] / 1024, edges[e + 1] == ulong.MaxValue ? "inf" : (edges[e + 1] / 1024) + " KB", s.Count, Mb(s.Aggregate(0UL, (a, b) => a + b))));
    }
    sb.AppendLine("charged image pages by module (MB: charged, of which written):");
    foreach (var kv in imageBy.OrderByDescending(kv => kv.Value).Take(top)) {
      ulong w; imageWritten.TryGetValue(kv.Key, out w);
      sb.AppendLine(string.Format("  {0} {1}  {2}", Mb(kv.Value), Mb(w), kv.Key));
    }
    sb.AppendLine(string.Format("modules loaded: {0}", mods.Count));
    CloseHandle(p);
    return sb.ToString();
  }
}
