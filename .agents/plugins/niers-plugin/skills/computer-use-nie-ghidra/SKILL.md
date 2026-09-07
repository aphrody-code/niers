---
name: computer-use-nie-ghidra
description: Probe and operate the approved Computer Use boundary for the local nie.exe and Ghidra CodeBrowser session.
---

# Computer Use — nie.exe and Ghidra

Use `niers computer-use nie-exe` to verify the exact executable path and
`niers computer-use ghidra` to verify the local Ghidra MCP endpoint. These
commands are probes: they do not launch, click, patch, or write.

The output is JSON and follows `schemas/computer-use-probe.schema.json`.
Treat an HTTP 400/401/404/405 from the Ghidra endpoint as reachability only;
it is not proof that an MCP session was initialized. A successful runtime claim
requires a protocol handshake and an inspected CodeBrowser result.

For UI mutation, keep a human-deniable boundary through WinClean/Computer Use:
observe before the action, perform one bounded action, then observe again. Never
use arbitrary shell text as a UI action and never infer `nie.exe` state from a
window title alone.
