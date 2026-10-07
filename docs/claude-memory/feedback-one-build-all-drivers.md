---
name: feedback-one-build-all-drivers
description: "The owner expects ONE distributed Cantus build that works with every audio driver (WASAPI + ASIO), like modern DAWs — never feature-gated editions"
metadata:
  type: feedback
---

On 2026-10-06 the owner tried round 20's builds: the default one said "made without ASIO"
and the ASIO one lost ASIO4ALL ("disconnected", no sound). They said: shouldn't we be able
to use any driver after one build? That's how modern DAWs work — no new build.

**Why:** a cargo feature for ASIO was a developer convenience (the SDK is needed at build
time and the proxy blocked its download) that leaked into the product.

**How to apply:** every build handed to the owner or distributed includes ASIO and WASAPI;
xtask/CI fetch the ASIO SDK (pinned hash) themselves. Test real drivers on this machine
(ASIO4ALL v2, Realtek ASIO; never load the UMC driver while the interface is unplugged)
before calling audio-device work done — duplex (input + output) for a minute, not just
output. See [[round-19-recording]], [[round-20]].
