// A sampling profiler for any Windows x64 process, from outside it (tools/scorecard/profile.ps1).
// No administrator rights, no ETW: every few milliseconds each thread that used CPU since the
// last look is suspended, its stack walked with dbghelp (StackWalk64, the program's PDB for
// names and lines) and resumed. Samples are therefore weighted by CPU, roughly.
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;

public static class Sampler {
  const uint THREAD_ALL = 0x0002 | 0x0008 | 0x0040 | 0x0800; // suspend/resume, get context, query, query limited
  const uint PROCESS_ALL = 0x0400 | 0x0010 | 0x00100000; // + synchronize // query information, vm read

  [DllImport("kernel32.dll")] static extern IntPtr OpenProcess(uint a, bool i, int pid);
  [DllImport("kernel32.dll")] static extern IntPtr OpenThread(uint a, bool i, int tid);
  [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr h);
  [DllImport("kernel32.dll")] static extern uint SuspendThread(IntPtr h);
  [DllImport("kernel32.dll")] static extern uint ResumeThread(IntPtr h);
  [DllImport("kernel32.dll")] static extern bool GetThreadContext(IntPtr h, IntPtr ctx);
  [DllImport("kernel32.dll")] static extern bool GetThreadTimes(IntPtr h, out long c, out long e, out long k, out long u);
  [DllImport("kernel32.dll")] static extern int GetThreadDescription(IntPtr h, out IntPtr desc);
  [DllImport("kernel32.dll")] static extern IntPtr LocalFree(IntPtr p);
  [DllImport("kernel32.dll")] static extern uint WaitForSingleObject(IntPtr h, uint ms);
  [DllImport("winmm.dll")] static extern uint timeBeginPeriod(uint ms);
  [DllImport("winmm.dll")] static extern uint timeEndPeriod(uint ms);

  delegate bool ReadMem(IntPtr p, ulong a, IntPtr buf, uint n, out uint read);
  delegate IntPtr FnTable(IntPtr p, ulong a);
  delegate ulong ModBase(IntPtr p, ulong a);

  [DllImport("dbghelp.dll", SetLastError = true)] static extern bool SymInitializeW(IntPtr p, [MarshalAs(UnmanagedType.LPWStr)] string path, bool invade);
  [DllImport("dbghelp.dll")] static extern uint SymSetOptions(uint o);
  [DllImport("dbghelp.dll")] static extern bool SymRefreshModuleList(IntPtr p);
  [DllImport("dbghelp.dll")] static extern IntPtr SymFunctionTableAccess64(IntPtr p, ulong a);
  [DllImport("dbghelp.dll")] static extern ulong SymGetModuleBase64(IntPtr p, ulong a);
  [DllImport("dbghelp.dll")] static extern bool StackWalk64(uint machine, IntPtr p, IntPtr t, IntPtr frame, IntPtr ctx, ReadMem r, FnTable f, ModBase m, IntPtr tr);
  [DllImport("dbghelp.dll", CharSet = CharSet.Unicode)] static extern bool SymFromAddrW(IntPtr p, ulong a, out ulong disp, IntPtr sym);

  static FnTable fnTable = (p, a) => SymFunctionTableAccess64(p, a);
  static ModBase modBase = (p, a) => SymGetModuleBase64(p, a);

  class Thr { public IntPtr H; public long Cpu; public string Name; public int Samples; }

  // Samples `pid` for `seconds`, every `periodMs`; writes one line per sample to `outPath`:
  // `every`: look at every thread each time, not only those that used CPU since the last look
  // (the report drops the samples caught waiting).
  // "<ms since start>\t<tid>\t<thread name>\t<frame0>;<frame1>;…" (innermost first).
  public static string Run(int pid, double seconds, int periodMs, string outPath, string symPath, bool every = false) {
    IntPtr proc = OpenProcess(PROCESS_ALL, false, pid);
    if (proc == IntPtr.Zero) return "cannot open process";
    SymSetOptions(0x00000002 | 0x00000004 | 0x00000010); // undname, deferred loads, load lines
    if (!SymInitializeW(proc, symPath, true)) return "SymInitialize failed " + Marshal.GetLastWin32Error();
    var threads = new Dictionary<int, Thr>();
    var names = new Dictionary<ulong, string>();
    IntPtr ctx = Marshal.AllocHGlobal(1232 + 16);
    IntPtr ctxA = new IntPtr((ctx.ToInt64() + 15) & ~15L);
    IntPtr frame = Marshal.AllocHGlobal(1024);
    IntPtr sym = Marshal.AllocHGlobal(88 + 2 * 512);
    timeBeginPeriod(1); // so the sleeps between looks are the period asked
    var sw = Stopwatch.StartNew();
    var lines = new List<string>();
    var cpuLines = new List<string>();
    int refresh = 0; long total = 0; long lastList = -1000;
    while (sw.Elapsed.TotalSeconds < seconds) {
      // The thread list is read again every 250 ms (reading it is slow); between, the
      // threads already open are looked at.
      if (sw.ElapsedMilliseconds - lastList >= 250 || threads.Count == 0) {
        lastList = sw.ElapsedMilliseconds;
        Process p;
        try { p = Process.GetProcessById(pid); } catch { break; }
        if (p.HasExited) break;
        if (refresh++ % 20 == 0) SymRefreshModuleList(proc);
        foreach (var kv0 in threads) {
          long c0, e0, k0, u0;
          if (GetThreadTimes(kv0.Value.H, out c0, out e0, out k0, out u0))
            cpuLines.Add("cpu\t" + sw.ElapsedMilliseconds + "\t" + kv0.Key + "\t" + kv0.Value.Name + "\t" + (k0 + u0));
        }
        foreach (ProcessThread pt in p.Threads) {
          if (threads.ContainsKey(pt.Id)) continue;
          IntPtr h = OpenThread(THREAD_ALL, false, pt.Id);
          if (h == IntPtr.Zero) continue;
          var nt = new Thr { H = h, Cpu = 0, Name = "" };
          IntPtr d0; if (GetThreadDescription(h, out d0) >= 0 && d0 != IntPtr.Zero) { nt.Name = Marshal.PtrToStringUni(d0) ?? ""; LocalFree(d0); }
          if (nt.Name == "") nt.Name = "?";
          threads[pt.Id] = nt;
        }
      }
      foreach (var kv in threads) {
        int tid = kv.Key; Thr t = kv.Value;
        long c, e, k, u;
        if (!GetThreadTimes(t.H, out c, out e, out k, out u)) continue;
        long cpu = k + u;
        if (cpu == t.Cpu && !every) continue;  // idle since the last look (unless every thread is asked)
        t.Cpu = cpu;
        if (t.Name == "") {
          IntPtr d; if (GetThreadDescription(t.H, out d) >= 0 && d != IntPtr.Zero) { t.Name = Marshal.PtrToStringUni(d) ?? ""; LocalFree(d); }
          if (t.Name == "") t.Name = "?";
        }
        if (SuspendThread(t.H) == 0xFFFFFFFF) continue;
        var stack = new StringBuilder();
        try {
          for (int i = 0; i < 1232; i += 8) Marshal.WriteInt64(ctxA, i, 0);
          Marshal.WriteInt32(ctxA, 0x30, 0x10000B); // CONTEXT_FULL (AMD64)
          if (!GetThreadContext(t.H, ctxA)) continue;
          ulong rip = (ulong)Marshal.ReadInt64(ctxA, 0xF8), rsp = (ulong)Marshal.ReadInt64(ctxA, 0x98), rbp = (ulong)Marshal.ReadInt64(ctxA, 0xA0);
          for (int i = 0; i < 1024; i += 8) Marshal.WriteInt64(frame, i, 0);
          Marshal.WriteInt64(frame, 0, (long)rip); Marshal.WriteInt32(frame, 12, 3);
          Marshal.WriteInt64(frame, 32, (long)rbp); Marshal.WriteInt32(frame, 44, 3);
          Marshal.WriteInt64(frame, 48, (long)rsp); Marshal.WriteInt32(frame, 60, 3);
          for (int depth = 0; depth < 96; depth++) {
            if (!StackWalk64(0x8664, proc, t.H, frame, ctxA, null, fnTable, modBase, IntPtr.Zero)) break;
            ulong pc = (ulong)Marshal.ReadInt64(frame, 0);
            if (pc == 0) break;
            stack.Append(pc.ToString("x")).Append(';');
          }
        } finally { ResumeThread(t.H); }
        t.Samples++; total++;
        lines.Add(((long)sw.Elapsed.TotalMilliseconds) + "\t" + tid + "\t" + t.Name + "\t" + stack.ToString());
      }
      if (WaitForSingleObject(proc, 0) == 0) break;
      Thread.Sleep(periodMs);
    }
    timeEndPeriod(1);
    // Symbolise after sampling (cheaper while it runs).
    SymRefreshModuleList(proc);
    var outLines = new List<string>(lines.Count);
    foreach (var l in lines) {
      var parts = l.Split('\t');
      var sb = new StringBuilder();
      foreach (var a in parts[3].Split(new[] { ';' }, StringSplitOptions.RemoveEmptyEntries)) {
        ulong addr = Convert.ToUInt64(a, 16);
        string n;
        if (!names.TryGetValue(addr, out n)) {
          for (int i = 0; i < 88; i += 8) Marshal.WriteInt64(sym, i, 0);
          Marshal.WriteInt32(sym, 0, 88);          // SizeOfStruct
          Marshal.WriteInt32(sym, 80, 512);        // MaxNameLen
          ulong disp;
          // The return address points after the call; look one byte back for the caller's line.
          if (SymFromAddrW(proc, addr - 1, out disp, sym)) {
            int len = Marshal.ReadInt32(sym, 76);
            n = Marshal.PtrToStringUni(new IntPtr(sym.ToInt64() + 84), Math.Min(len, 511));
          } else n = "0x" + a;
          names[addr] = n;
        }
        sb.Append(n.Replace(';', ',')).Append(';');
      }
      outLines.Add(parts[0] + "\t" + parts[1] + "\t" + parts[2] + "\t" + sb.ToString());
    }
    outLines.AddRange(cpuLines);
    File.WriteAllLines(outPath, outLines);
    foreach (var t in threads.Values) CloseHandle(t.H);
    return "samples " + total + ", threads " + threads.Count;
  }
}
