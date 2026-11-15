# AI Tools

AI Tools is an optional module that lets AI assistants (Claude, Codex,
Cursor, VS Code and other MCP clients) search your clipboard history and
snippets, copy to the clipboard and save snippets, with your approval. It is
not installed by default.

Install it from **Settings → Modules**, confirm its permissions, then open
**Connect AI tool** and add the shown configuration to your AI tool. The tool
starts `copypaste module connect copypaste.ai-tools mcp`, which carries MCP
over the module's `mcp` endpoint. The first connection of a tool asks you in
CopyPaste: **Allow once**, **Always allow**, or **Deny**. New connections can
only read; **Allow changes** in **Connected tools** adds clipboard writes,
pins and new snippets. **Revoke** works from any of your devices.

- **What tools can see** is set by synced preferences: other devices,
  excluded devices and apps, content kinds and the history window (pinned
  items are always visible). CopyPaste itself never shares deleted, secret,
  credential, transient or hidden items.
- **Limits:** 30 calls per 10 s and 600 per hour per tool, replies of at most
  64 KB, and 2 MB of reads per connection before CopyPaste asks again.
- **Activity** keeps ids and counts of recent calls on this device only
  (500 entries, 30 days), never content or queries.

The MCP server runs on macOS, Windows and Linux. On Android the module only
manages connections; your phone's history reaches AI tools through a
computer.

Clipboard content returned to a tool is marked as untrusted data. These
limits apply to tools that use the CopyPaste connection; they do not protect
against other programs running under your account.
