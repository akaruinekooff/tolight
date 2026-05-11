# tolight

A terminal-based (TUI) todo list manager for developers. Todo files live alongside your project — one per project, under `.tolight/todos.json`.

![screenshot](/screenshot.png)

## Features

- **Project-aware** — automatically detects the project you're working in (walks up from cwd looking for `.tolight/todos.json` or a `.git` directory)
- **Per-project todos** — each project gets its own `.tolight/todos.json` checked into version control
- **Global fallback** — no project detected? Uses `~/.tolight/todos.json` for quick personal todos
- **Project registry** — named projects stored in `~/.tolight/projects.json` for CLI lookup
- **Dual-pane TUI** (powered by [ratatui](https://github.com/ratatui-org/ratatui)):
  - Left: view/edit notes attached to each todo
  - Right: scrollable todo list with completion status
- **Persistent config** — `~/.config/tolight/config.cfg`

## Usage

| Command | Description |
|---|---|
| `tolight` | Auto-detect project context and open todos |
| `tolight "project-name"` | Open a named project from the registry |
| `tolight new "project-name"` | Create a new per-project todo list in the current directory |

### Interactive keys

| Key | Action |
|---|---|
| `Tab` | Switch focus between notes pane and todo list |
| `↑`/`↓` | Scroll notes or navigate / reorder todos |
| `i` | Add a new todo |
| `space` | Toggle todo completion |
| `r` | Remove the selected todo (then `y` confirm / `n` cancel) |
| `e` | Edit notes for the selected todo (left pane focus) |
| `h` | Toggle hints bar |
| `Ctrl+C` / `q` | Quit |
| `Esc` | Cancel input mode |
| `Ctrl+J` | Insert newline in input fields |

## Installation

You can only build from source or use prebuilt releases:

```bash
git clone https://github.com/akaruineko/tolight.git
cd tolight
cargo install --path .
```

## Data locations

| What | Where |
|---|---|
| Global todos | `~/.tolight/todos.json` |
| Per-project todos | `<project>/.tolight/todos.json` |
| Project registry | `~/.tolight/projects.json` |
| Config | `~/.config/tolight/config.cfg` |

## License

MIT
