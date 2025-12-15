use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use tome_core::range::Direction as MoveDir;
use tome_core::{
    InputHandler, Key, KeyResult, Mode, Rope, Selection, Transaction,
    ext, movement,
};

use crate::commands;

/// A history entry for undo/redo.
#[derive(Clone)]
pub struct HistoryEntry {
    pub doc: Rope,
    pub selection: Selection,
}

#[derive(Default)]
pub struct Registers {
    pub yank: String,
}

pub struct Editor {
    pub doc: Rope,
    pub selection: Selection,
    pub input: InputHandler,
    pub path: PathBuf,
    pub modified: bool,
    pub scroll_offset: usize,
    pub message: Option<String>,
    pub registers: Registers,
    pub undo_stack: Vec<HistoryEntry>,
    pub redo_stack: Vec<HistoryEntry>,
}

impl Editor {
    pub fn new(path: PathBuf) -> io::Result<Self> {
        let content = if path.exists() {
            fs::read_to_string(&path)?
        } else {
            String::new()
        };

        Ok(Self::from_content(content, path))
    }

    pub fn from_content(content: String, path: PathBuf) -> Self {
        Self {
            doc: Rope::from(content.as_str()),
            selection: Selection::point(0),
            input: InputHandler::new(),
            path,
            modified: false,
            scroll_offset: 0,
            message: None,
            registers: Registers::default(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub fn mode(&self) -> Mode {
        self.input.mode()
    }

    pub fn mode_name(&self) -> &'static str {
        self.input.mode_name()
    }

    pub fn cursor_line(&self) -> usize {
        let head = self.selection.primary().head;
        self.doc
            .char_to_line(head.min(self.doc.len_chars().saturating_sub(1).max(0)))
    }

    pub fn cursor_col(&self) -> usize {
        let head = self.selection.primary().head;
        let line = self.cursor_line();
        let line_start = self.doc.line_to_char(line);
        head.saturating_sub(line_start)
    }

    pub fn save_undo_state(&mut self) {
        self.undo_stack.push(HistoryEntry {
            doc: self.doc.clone(),
            selection: self.selection.clone(),
        });
        self.redo_stack.clear();

        const MAX_UNDO: usize = 100;
        if self.undo_stack.len() > MAX_UNDO {
            self.undo_stack.remove(0);
        }
    }

    pub fn undo(&mut self) {
        if let Some(entry) = self.undo_stack.pop() {
            self.redo_stack.push(HistoryEntry {
                doc: self.doc.clone(),
                selection: self.selection.clone(),
            });

            self.doc = entry.doc;
            self.selection = entry.selection;
            self.message = Some("Undo".to_string());
        } else {
            self.message = Some("Nothing to undo".to_string());
        }
    }

    pub fn redo(&mut self) {
        if let Some(entry) = self.redo_stack.pop() {
            self.undo_stack.push(HistoryEntry {
                doc: self.doc.clone(),
                selection: self.selection.clone(),
            });

            self.doc = entry.doc;
            self.selection = entry.selection;
            self.message = Some("Redo".to_string());
        } else {
            self.message = Some("Nothing to redo".to_string());
        }
    }

    pub fn insert_text(&mut self, text: &str) {
        self.save_undo_state();
        let tx = Transaction::insert(self.doc.slice(..), &self.selection, text.to_string());
        tx.apply(&mut self.doc);
        let head = self.selection.primary().head + text.chars().count();
        self.selection = Selection::point(head);
        self.modified = true;
    }

    pub fn save(&mut self) -> io::Result<()> {
        let mut f = fs::File::create(&self.path)?;
        for chunk in self.doc.chunks() {
            f.write_all(chunk.as_bytes())?;
        }
        self.modified = false;
        self.message = Some(format!("Saved {}", self.path.display()));
        Ok(())
    }

    pub fn yank_selection(&mut self) {
        let primary = self.selection.primary();
        let from = primary.from();
        let to = primary.to();
        if from < to {
            self.registers.yank = self.doc.slice(from..to).to_string();
            self.message = Some(format!("Yanked {} chars", to - from));
        }
    }

    pub fn paste_after(&mut self) {
        if self.registers.yank.is_empty() {
            return;
        }
        let slice = self.doc.slice(..);
        self.selection.transform_mut(|r| {
            *r = movement::move_horizontally(slice, *r, MoveDir::Forward, 1, false);
        });
        self.insert_text(&self.registers.yank.clone());
    }

    pub fn paste_before(&mut self) {
        if self.registers.yank.is_empty() {
            return;
        }
        self.insert_text(&self.registers.yank.clone());
    }

    /// Select a text object using the extension registry.
    pub fn select_object_by_trigger(&mut self, trigger: char, inner: bool) -> bool {
        if let Some(obj_def) = ext::find_text_object(trigger) {
            let slice = self.doc.slice(..);
            self.selection.transform_mut(|r| {
                let handler = if inner { obj_def.inner } else { obj_def.around };
                if let Some(new_range) = handler(slice, r.head) {
                    *r = new_range;
                }
            });
            true
        } else {
            false
        }
    }

    /// Select to object boundary using the extension registry.
    pub fn select_to_object_boundary(&mut self, trigger: char, to_start: bool, extend: bool) -> bool {
        if let Some(obj_def) = ext::find_text_object(trigger) {
            let slice = self.doc.slice(..);
            self.selection.transform_mut(|r| {
                // Use 'around' to get the full object bounds
                if let Some(obj_range) = (obj_def.around)(slice, r.head) {
                    let new_head = if to_start { obj_range.from() } else { obj_range.to() };
                    if extend {
                        *r = tome_core::Range::new(r.anchor, new_head);
                    } else {
                        *r = tome_core::Range::new(r.head, new_head);
                    }
                }
            });
            true
        } else {
            false
        }
    }

    pub fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> bool {
        self.message = None;

        let key: Key = key.into();
        let result = self.input.handle_key(key);

        match result {
            KeyResult::Command(cmd, params) => {
                commands::execute_command(self, cmd, params.count, params.extend)
            }
            KeyResult::ModeChange(mode) => {
                if matches!(mode, Mode::Normal) {
                    self.message = None;
                }
                false
            }
            KeyResult::InsertChar(c) => {
                self.insert_text(&c.to_string());
                false
            }
            KeyResult::Pending(msg) => {
                self.message = Some(msg);
                false
            }
            KeyResult::ExecuteCommand(cmd) => commands::execute_command_line(self, &cmd),
            KeyResult::ExecuteSearch { pattern, reverse } => {
                self.message = Some(format!(
                    "Search '{}' {}",
                    pattern,
                    if reverse { "(reverse)" } else { "" }
                ));
                false
            }
            KeyResult::Consumed => false,
            KeyResult::Unhandled => false,
            KeyResult::Quit => true,
        }
    }
}
