# cadforge-cli

`cadforge-cli` is CadForge without its window. It reports on drawings, converts them, runs command scripts on them, and
serves the Model Context Protocol so AI agents (Claude and other MCP clients) can draw with CadForge. It ships next
to the desktop program in the Linux and Windows packages; build it with `cargo build --release -p cadforge-cli`.

```sh
cadforge-cli info plan.dwg                          # what the drawing holds, as JSON
cadforge-cli convert plan.dwg plan.dxf              # .dwg, .dxf, .json; or plot model space to .svg / .pdf
cadforge-cli run plan.dwg -s add-border.scr -o out.dwg
cadforge-cli run -c CIRCLE -c 0,0 -c 25 -o circle.dxf
cadforge-cli commands dim                           # the commands whose name or description mentions "dim"
```

A script is what you would type at the command line, one entry per line: a command name, then the answers to its
prompts (`10,20` a point, `@30,0` relative, `50<45` polar, a number or an option keyword). An empty line is Enter
and lines starting with `;` are comments, as in AutoCAD script (`.scr`) files. `run` stops with an error status when a
command reported an error, and says so when the last command is still waiting for input.

## MCP server

`cadforge-cli mcp` speaks MCP (JSON-RPC 2.0, one message per line) on standard input and output. Its tools:

| Tool | What it does |
|---|---|
| `new_drawing` | Start an empty drawing (metric, or imperial) |
| `open_drawing` | Open a .dwg, .dxf or .json drawing |
| `run_commands` | Type lines at the command line; returns the command-line text, errors and any prompt still waiting |
| `cancel` | Escape out of a waiting command |
| `drawing_info` | Object counts, layers, blocks, styles, layouts and extents |
| `list_objects` | Model-space objects as JSON, by type or layer |
| `add_objects` | Add objects given as JSON |
| `list_commands` | The command reference, optionally filtered |
| `save_drawing` | Save as .dwg, .dxf, .json, .svg or .pdf |

To use it from Claude Code: `claude mcp add cadforge -- /path/to/cadforge-cli mcp`. Other clients take the same
command in their server list, for example:

```json
{ "mcpServers": { "cadforge": { "command": "/path/to/cadforge-cli", "args": ["mcp"] } } }
```
