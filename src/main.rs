mod cfg;
mod project;

use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
    },
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::{env, fs, io};

use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Terminal,
};

use crate::cfg::{global_todo_path, load_config, load_todos, parse_config, save_to_file, update_config_line};
use crossterm::event::KeyModifiers;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq)]
enum Focus {
    Left,
    Right,
}

#[derive(Clone, PartialEq)]
enum Mode {
    Normal,
    InputTodo,
    EditNotes,
    RemoveDialogue,
    SuggestProject,
}

#[derive(Serialize, Deserialize, Clone)]
struct Todo {
    id: u64,
    text: String,
    done: bool,
    notes: String,
}

struct App {
    focus: Focus,
    mode: Mode,
    todos: Vec<Todo>,
    selected: usize,
    input: String,

    notes_scroll: u16,
    edit_scroll: u16,
    todo_scroll: u16,
    todo_view_height: usize,

    cfg: HashMap<String, String>,
    cfg_dir: PathBuf,
    hints: bool,

    todo_path: PathBuf,
    suggested_project: Option<(String, PathBuf)>,
}

fn main() -> Result<(), io::Error> {
    let args: Vec<String> = env::args().collect();
    let cwd = env::current_dir()?;

    let proj = ProjectDirs::from("space", "akaruineko", "tolight")
        .expect("cannot determine dirs");
    let cfg_dir = proj.config_dir().to_path_buf();
    let cfg_path = cfg_dir.join("config.cfg");

    let (todo_path, suggested_project, mode) = match args.len() {
        1 => {
            // tolight — auto-detect
            match project::detect_project(&cwd) {
                Some((_, root)) if root.join(".tolight").join("todos.json").exists() => {
                    (root.join(".tolight").join("todos.json"), None, Mode::Normal)
                }
                Some((name, root)) => {
                    // git repo found — suggest creating project todo
                    (global_todo_path(), Some((name, root)), Mode::SuggestProject)
                }
                None => {
                    (global_todo_path(), None, Mode::Normal)
                }
            }
        }
        2 => {
            // tolight "project-name" — look up by name
            let registry = project::load_registry();
            let name = &args[1];
            match registry.get(name) {
                Some(path_str) => {
                    let root = PathBuf::from(path_str);
                    let path = root.join(".tolight").join("todos.json");
                    (path, None, Mode::Normal)
                }
                None => {
                    eprintln!("Project '{}' not found.", name);
                    eprintln!("Run 'tolight new \"{}\"' from the project directory to create one.", name);
                    return Ok(());
                }
            }
        }
        3 if args[1] == "new" => {
            // tolight new "project-name" — create and register
            let name = &args[2];
            let root = project::find_project_root(&cwd);
            let tolight_dir = root.join(".tolight");
            fs::create_dir_all(&tolight_dir)?;
            let path = tolight_dir.join("todos.json");
            if !path.exists() {
                fs::write(&path, "[]")?;
            }
            project::register_project(name, &root);
            (path, None, Mode::Normal)
        }
        _ => {
            eprintln!("Usage: tolight [project-name | new <project-name>]");
            return Ok(());
        }
    };

    // Ensure global ~/.tolight/ exists for the registry
    if let Some(parent) = global_todo_path().parent() {
        fs::create_dir_all(parent).ok();
    }

    enable_raw_mode()?;

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App {
        focus: Focus::Right,
        mode,
        todos: load_todos(&todo_path),
        selected: 0,
        input: String::new(),
        edit_scroll: 0,
        notes_scroll: 0,
        todo_scroll: 0,
        todo_view_height: 0usize,

        cfg_dir: cfg_dir.clone(),
        cfg: load_config(cfg_path.to_str().unwrap()),
        hints: true,

        todo_path,
        suggested_project,
    };

    loop {
        let show_help = app.cfg.get("show_hints").map(|v| v == "true").unwrap_or(true);

        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(3),
                    Constraint::Length(3),
                ])
                .split(f.area());

            let show_bottom = show_help
                || matches!(app.mode, Mode::RemoveDialogue | Mode::SuggestProject);

            let main = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(50),
                    Constraint::Percentage(50),
                ])
                .split(if show_bottom { chunks[0] } else { f.area() });

            let todo_view_height = main[1].height.saturating_sub(2) as usize;
            app.todo_view_height = todo_view_height;

            let normal_mode_text = match app.focus {
                Focus::Left => {
                    "e: edit notes | space: toggle | ↑↓: scroll | h: toggle hints | q: quit"
                }
                Focus::Right => {
                    "i: add todo | space: toggle | ↑↓: move | h: toggle hints | q: quit"
                }
            };

            let help_text = match app.mode {
                Mode::RemoveDialogue => "y: confirm delete | n: cancel",
                _ if show_help && app.mode != Mode::SuggestProject => normal_mode_text,
                Mode::InputTodo => "typing todo... enter: save | esc: cancel",
                Mode::EditNotes => "editing notes... enter: save | esc: cancel",
                _ => "",
            };

            let left_text = match app.mode {
                Mode::EditNotes | Mode::InputTodo => &app.input,
                _ => app
                    .todos
                    .get(app.selected)
                    .map(|t| t.notes.as_str())
                    .unwrap_or(""),
            };
            let scroll = match app.mode {
                Mode::EditNotes => app.edit_scroll,
                _ => app.notes_scroll,
            };

            let left = Paragraph::new(left_text).scroll((scroll, 0)).block(
                Block::default()
                    .title(match app.mode {
                        Mode::EditNotes => "edit notes (ESC/ENTER)",
                        Mode::InputTodo => "new todo (ENTER)",
                        _ => "notes",
                    })
                    .borders(Borders::ALL)
                    .style(if app.focus == Focus::Left {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default()
                    }),
            );

            let help = if matches!(app.mode, Mode::SuggestProject) {
                let msg = format!(
                    "Create project '{}'? y: yes | n: no (using global)",
                    app.suggested_project.as_ref().map(|(n, _)| n.as_str()).unwrap_or("")
                );
                Paragraph::new(msg).block(
                    Block::default().borders(Borders::ALL).title("dialogue")
                )
            } else {
                Paragraph::new(help_text).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(if !matches!(app.mode, Mode::RemoveDialogue) {"help"} else {"dialogue"})
                )
            };

            f.render_widget(left, main[0]);

            let visible_todos = app
                .todos
                .iter()
                .skip(app.todo_scroll as usize)
                .take(todo_view_height)
                .enumerate()
                .map(|(i, t)| {
                    let real_i = i + app.todo_scroll as usize;

                    let status = if t.done { "✔" } else { "✗" };

                    let selector = if real_i == app.selected && app.focus == Focus::Right {
                        "▶ "
                    } else {
                        "  "
                    };

                    ListItem::new(format!("{}{} {}", selector, status, t.text))
                })
                .collect::<Vec<_>>();

            let list = List::new(visible_todos).block(
                Block::default()
                    .title("todo")
                    .borders(Borders::ALL)
                    .style(if app.focus == Focus::Right {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default()
                    }),
            );

            f.render_widget(list, main[1]);

            if show_bottom {
                f.render_widget(help, chunks[1]);
            }
        })?;

        if event::poll(std::time::Duration::from_millis(16))?
            && let Event::Key(key) = event::read()? {

                // SuggestProject mode — handled before everything else
                if app.mode == Mode::SuggestProject {
                    match key.code {
                        KeyCode::Char('y') => {
                            if let Some((name, root)) = app.suggested_project.take() {
                                let tolight_dir = root.join(".tolight");
                                fs::create_dir_all(&tolight_dir)?;
                                let path = tolight_dir.join("todos.json");
                                project::register_project(&name, &root);
                                app.todo_path = path;
                                app.todos.clear();
                            }
                            app.mode = Mode::Normal;
                        }
                        KeyCode::Char('n') => {
                            app.suggested_project = None;
                            app.mode = Mode::Normal;
                        }
                        _ => {}
                    }
                    continue;
                }

                match key.code {
                    // exit mode
                    KeyCode::Esc => {
                        app.mode = Mode::Normal;
                        app.input.clear();
                    }

                    // switch focus
                    KeyCode::Tab if app.mode == Mode::Normal => {
                        app.focus = match app.focus {
                            Focus::Left => Focus::Right,
                            Focus::Right => Focus::Left,
                        };
                    }

                    KeyCode::Down => {
                        match app.mode {
                            Mode::Normal => {
                                if app.focus == Focus::Left {
                                    app.notes_scroll = app.notes_scroll.saturating_add(1);
                                } else {
                                    if app.selected + 1 < app.todos.len() {
                                        app.selected += 1;
                                        if app.selected >= app.todo_scroll as usize + app.todo_view_height {
                                            app.todo_scroll += 1;
                                        }
                                        app.notes_scroll = 0;
                                    }
                                }
                            }
                            Mode::EditNotes => {
                                app.edit_scroll = app.edit_scroll.saturating_add(1);
                            }
                            _ => {}
                        }
                    }

                    KeyCode::Up => {
                        match app.mode {
                            Mode::Normal => {
                                if app.focus == Focus::Left {
                                    app.notes_scroll = app.notes_scroll.saturating_sub(1);
                                } else {
                                    if app.selected > 0 {
                                        app.selected -= 1;
                                        if app.selected < app.todo_scroll as usize {
                                            app.todo_scroll = app.todo_scroll.saturating_sub(1);
                                        }
                                        app.notes_scroll = 0;
                                    }
                                }
                            }
                            Mode::EditNotes => {
                                app.edit_scroll = app.edit_scroll.saturating_sub(1);
                            }
                            _ => {}
                        }
                    }

                    KeyCode::Char(' ') => {
                        if matches!(app.mode, Mode::InputTodo | Mode::EditNotes) {
                            app.input.push(' ');
                        }
                        if app.mode == Mode::Normal {
                            if let Some(t) = app.todos.get_mut(app.selected) {
                                t.done = !t.done;
                            }
                            save_to_file(&app.todos, &app.todo_path)?;
                        }
                    }

                    // some poor fix for some terminal lol
                    KeyCode::Char('j') => {
                        if key.modifiers.contains(KeyModifiers::CONTROL) {
                            app.input.push('\n');
                            continue
                        }
                        if matches!(app.mode, Mode::InputTodo | Mode::EditNotes) {
                            app.input.push('j');
                        }
                    }

                    KeyCode::Char('h') => {
                        if matches!(app.mode, Mode::InputTodo | Mode::EditNotes) {
                            app.input.push('h');
                        } else {
                            app.hints = !app.hints;
                            match app.hints {
                                true => {
                                    app.cfg = parse_config(&update_config_line(app.cfg_dir.join("config.cfg").to_str().unwrap(), "show_hints", "true"));
                                }
                                false => {
                                    app.cfg = parse_config(&update_config_line(app.cfg_dir.join("config.cfg").to_str().unwrap(), "show_hints", "false"));
                                }
                            }
                        }
                    }

                    // enter
                    KeyCode::Enter => {
                        if key.modifiers.contains(KeyModifiers::CONTROL) {
                            // ctrl+enter - newline
                            app.input.push('\n');
                        } else {match app.mode {
                            Mode::InputTodo => {
                                    let text = app.input.drain(..).collect::<String>().trim().to_string();

                                    if !text.is_empty() {
                                        app.todos.push(Todo {
                                            id: (app.todos.len() + 1) as u64,
                                            text,
                                            done: false,
                                            notes: String::new(),
                                        });
                                        save_to_file(&app.todos, &app.todo_path)?;
                                    }
                                    app.mode = Mode::Normal;
                            }
                            Mode::EditNotes => {
                                    if let Some(t) = app.todos.get_mut(app.selected) {
                                        t.notes = app.input.drain(..).collect();
                                    }
                                    save_to_file(&app.todos, &app.todo_path)?;
                                app.mode = Mode::Normal;
                            }
                            _ => {}
                        }}
                    },

                    // input mode
                    KeyCode::Char('i') => {
                        if matches!(app.mode, Mode::InputTodo | Mode::EditNotes) {
                            app.input.push('i');
                        }
                        if app.mode == Mode::Normal && app.focus == Focus::Right {
                            app.mode = Mode::InputTodo;
                            app.input.clear();
                        }
                    }

                    KeyCode::Char('r') => {
                        if matches!(app.mode, Mode::InputTodo | Mode::EditNotes) {
                            app.input.push('r');
                        }
                        if app.mode == Mode::Normal && app.focus == Focus::Right {
                            app.mode = Mode::RemoveDialogue;
                            app.input.clear();
                        }
                    }

                    KeyCode::Char('y') => {
                        if matches!(app.mode, Mode::InputTodo | Mode::EditNotes) {
                            app.input.push('y');
                        }
                        if app.mode == Mode::RemoveDialogue {
                            let id = app.todos[app.selected].id;
                            app.todos.retain(|t| t.id != id);
                            save_to_file(&app.todos, &app.todo_path)?;
                            app.mode = Mode::Normal;
                        }
                    }
                    KeyCode::Char('n') => {
                        if matches!(app.mode, Mode::InputTodo | Mode::EditNotes) {
                            app.input.push('n');
                        }
                        if app.mode == Mode::RemoveDialogue {
                            app.mode = Mode::Normal;
                        }
                    }

                    // edit notes
                    KeyCode::Char('e') => {
                        if matches!(app.mode, Mode::InputTodo | Mode::EditNotes) {
                            app.input.push('e');
                        }
                        if app.mode == Mode::Normal && app.focus == Focus::Left
                            && let Some(t) = app.todos.get(app.selected) {
                                app.input = t.notes.clone();
                                app.mode = Mode::EditNotes;
                        }
                    }

                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        save_to_file(&app.todos, &app.todo_path)?;
                        break;
                    }

                    // input
                    KeyCode::Char(c) => {
                        if matches!(app.mode, Mode::InputTodo | Mode::EditNotes | Mode::RemoveDialogue) {
                            app.input.push(c);
                        }
                        if c == 'q' {
                            break;
                        }
                    }

                    KeyCode::Backspace => {
                        if matches!(app.mode, Mode::InputTodo | Mode::EditNotes) {
                            app.input.pop();
                        }
                    }

                    _ => {}
                }

        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}