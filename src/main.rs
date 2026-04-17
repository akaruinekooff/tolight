mod cfg;

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

use crate::cfg::{load_config, load_todos, parse_config, save_to_file, update_config_line};
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
}

#[derive(Serialize, Deserialize)]
#[derive(Clone)]
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
}

fn main() -> Result<(), io::Error> {
    enable_raw_mode()?;

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let proj = ProjectDirs::from("space", "akaruineko", "tolight")
        .expect("cannot determine dirs");
    let cfg = proj.config_dir().join("config.cfg");

    fs::create_dir_all(env::current_dir().expect("check perms in current dir").join(".tolight")).ok();

    let mut app = App {
        focus: Focus::Right,
        mode: Mode::Normal,
        todos: load_todos(),
        selected: 0,
        input: String::new(),
        edit_scroll: 0,
        notes_scroll: 0,
        todo_scroll: 0,
        todo_view_height: 0usize,

        cfg_dir: proj.config_dir().to_path_buf(),
        cfg: load_config(cfg.to_str().unwrap()),
        hints: true,
    };


    loop {
        let show_help = app.cfg.get("show_hints").map(|v| v == "true").unwrap_or(true);
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(3),   // main area
                    Constraint::Length(3)  // help bar
                ])
                .split(f.area());
            let main = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(50),
                    Constraint::Percentage(50),
                ])
                .split(if show_help || matches!(app.mode, Mode::RemoveDialogue) { chunks[0] } else { f.area() });

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
                _ if show_help => normal_mode_text,
                Mode::InputTodo => "typing todo... enter: save | esc: cancel",
                Mode::EditNotes => "editing notes... enter: save | esc: cancel",
                _ => "",
            };

            // left (notes or editor)
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


            let help = Paragraph::new(help_text).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(if !matches!(app.mode, Mode::RemoveDialogue) {"help"} else {"dialogue"})
            );


            f.render_widget(left, main[0]);

            // right (tod0 list)
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

            if show_help || matches!(app.mode, Mode::RemoveDialogue) {
                f.render_widget(help, chunks[1]);
            }
        })?;

        if event::poll(std::time::Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    // exit mode
                    KeyCode::Esc => {
                        app.mode = Mode::Normal;
                        app.input.clear();
                    }

                    // switch focus
                    KeyCode::Tab => {
                        if app.mode == Mode::Normal {
                            app.focus = match app.focus {
                                Focus::Left => Focus::Right,
                                Focus::Right => Focus::Left,
                            };
                        }
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
                                    app.cfg = parse_config(&*update_config_line(app.cfg_dir.join("config.cfg").to_str().unwrap(), "show_hints", "true"));
                                }
                                false => {
                                    app.cfg = parse_config(&*update_config_line(app.cfg_dir.join("config.cfg").to_str().unwrap(), "show_hints", "false"));
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
                                    }
                                    app.mode = Mode::Normal;
                            }
                            Mode::EditNotes => {
                                    if let Some(t) = app.todos.get_mut(app.selected) {
                                        t.notes = app.input.drain(..).collect();
                                    }
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
                        if app.mode == Mode::Normal && app.focus == Focus::Left {
                            if let Some(t) = app.todos.get(app.selected) {
                                app.input = t.notes.clone();
                                app.mode = Mode::EditNotes;
                            }
                        }
                    }

                    // input
                    KeyCode::Char(c) => {
                        if matches!(app.mode, Mode::InputTodo | Mode::EditNotes | Mode::RemoveDialogue) {
                            app.input.push(c);
                        }
                        if c == KeyCode::Char('q').as_char().unwrap() {
                            save_to_file(app.todos)?;
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
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}