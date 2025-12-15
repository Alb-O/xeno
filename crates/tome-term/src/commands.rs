use tome_core::ext;
use tome_core::range::Direction as MoveDir;
use tome_core::{Command, Selection, Transaction, WordType, movement};
use tome_core::keymap::ObjectSelection;

use crate::editor::Editor;

const SCROLL_HALF_PAGE: usize = 10;
const SCROLL_FULL_PAGE: usize = 20;

fn apply_case_conversion<F>(editor: &mut Editor, char_mapper: F)
where
    F: Fn(char) -> Box<dyn Iterator<Item = char>>,
{
    let primary = editor.selection.primary();
    let from = primary.from();
    let to = primary.to();
    if from < to {
        editor.save_undo_state();
        let text: String = editor
            .doc
            .slice(from..to)
            .chars()
            .flat_map(|c| char_mapper(c))
            .collect();
        let new_len = text.chars().count();
        let tx = Transaction::delete(editor.doc.slice(..), &editor.selection);
        editor.selection = tx.map_selection(&editor.selection);
        tx.apply(&mut editor.doc);
        let tx = Transaction::insert(editor.doc.slice(..), &editor.selection, text);
        tx.apply(&mut editor.doc);
        let head = editor.selection.primary().head + new_len;
        editor.selection = Selection::point(head);
        editor.modified = true;
    }
}

pub fn execute_command_line(editor: &mut Editor, input: &str) -> bool {
    let input = input.trim();
    let mut parts = input.split_whitespace();
    let cmd_name = match parts.next() {
        Some(name) => name,
        None => return false,
    };
    let _args: Vec<&str> = parts.collect();

    let cmd = match ext::find_command(cmd_name) {
        Some(cmd) => cmd,
        None => {
            editor.message = Some(format!("Unknown command: {}", cmd_name));
            return false;
        }
    };

    match cmd.name {
        "help" => {
            let help_text: Vec<String> = ext::COMMANDS
                .iter()
                .map(|c| {
                    let aliases = if c.aliases.is_empty() {
                        String::new()
                    } else {
                        format!(" ({})", c.aliases.join(", "))
                    };
                    format!(":{}{} - {}", c.name, aliases, c.description)
                })
                .collect();
            editor.message = Some(help_text.join(" | "));
        }
        "quit" => return true,
        "quit!" => return true,
        "write" => {
            match editor.save() {
                Ok(()) => editor.message = Some("Written".into()),
                Err(e) => editor.message = Some(format!("Error saving: {}", e)),
            }
        }
        "wq" => {
            match editor.save() {
                Ok(()) => return true,
                Err(e) => editor.message = Some(format!("Error saving: {}", e)),
            }
        }
        _ => {
            editor.message = Some(format!("{} not implemented", cmd.name));
        }
    }
    false
}

pub fn execute_command(editor: &mut Editor, cmd: Command, count: u32, extend: bool) -> bool {
    let slice = editor.doc.slice(..);
    let count_usize = if count == 0 { 1 } else { count as usize };

    match cmd {
        Command::MoveLeft => {
            editor.selection.transform_mut(|r| {
                *r = movement::move_horizontally(slice, *r, MoveDir::Backward, count_usize, extend);
            });
        }
        Command::MoveRight => {
            editor.selection.transform_mut(|r| {
                *r = movement::move_horizontally(slice, *r, MoveDir::Forward, count_usize, extend);
            });
        }
        Command::MoveUp => {
            editor.move_visual_vertical(MoveDir::Backward, count_usize, extend);
        }
        Command::MoveDown => {
            editor.move_visual_vertical(MoveDir::Forward, count_usize, extend);
        }

        Command::MoveNextWordStart => {
            editor.selection.transform_mut(|r| {
                *r = movement::move_to_next_word_start(slice, *r, count_usize, WordType::Word, extend);
            });
        }
        Command::MovePrevWordStart => {
            editor.selection.transform_mut(|r| {
                *r = movement::move_to_prev_word_start(slice, *r, count_usize, WordType::Word, extend);
            });
        }
        Command::MoveNextWordEnd => {
            editor.selection.transform_mut(|r| {
                *r = movement::move_to_next_word_end(slice, *r, count_usize, WordType::Word, extend);
            });
        }
        Command::MoveNextWORDStart => {
            editor.selection.transform_mut(|r| {
                *r = movement::move_to_next_word_start(slice, *r, count_usize, WordType::WORD, extend);
            });
        }
        Command::MovePrevWORDStart => {
            editor.selection.transform_mut(|r| {
                *r = movement::move_to_prev_word_start(slice, *r, count_usize, WordType::WORD, extend);
            });
        }
        Command::MoveNextWORDEnd => {
            editor.selection.transform_mut(|r| {
                *r = movement::move_to_next_word_end(slice, *r, count_usize, WordType::WORD, extend);
            });
        }

        Command::MoveLineStart => {
            editor.selection.transform_mut(|r| {
                *r = movement::move_to_line_start(slice, *r, extend);
            });
        }
        Command::MoveLineEnd => {
            editor.selection.transform_mut(|r| {
                *r = movement::move_to_line_end(slice, *r, extend);
            });
        }
        Command::MoveFirstNonWhitespace => {
            editor.selection.transform_mut(|r| {
                *r = movement::move_to_first_nonwhitespace(slice, *r, extend);
            });
        }

        Command::MoveDocumentStart => {
            editor.selection.transform_mut(|r| {
                *r = movement::move_to_document_start(slice, *r, extend);
            });
        }
        Command::MoveDocumentEnd => {
            editor.selection.transform_mut(|r| {
                *r = movement::move_to_document_end(slice, *r, extend);
            });
        }

        Command::CollapseSelection => {
            editor.selection.transform_mut(|r| {
                r.anchor = r.head;
            });
        }
        Command::FlipSelection => {
            editor.selection.transform_mut(|r| {
                std::mem::swap(&mut r.anchor, &mut r.head);
            });
        }
        Command::EnsureForward => {
            editor.selection.transform_mut(|r| {
                if r.head < r.anchor {
                    std::mem::swap(&mut r.anchor, &mut r.head);
                }
            });
        }

        Command::SelectLine => {
            editor.selection.transform_mut(|r| {
                let line = slice.char_to_line(r.head);
                let start = slice.line_to_char(line);
                let end = if line + 1 < slice.len_lines() {
                    slice.line_to_char(line + 1)
                } else {
                    slice.len_chars()
                };
                r.anchor = start;
                r.head = end;
            });
        }
        Command::SelectAll => {
            editor.selection = Selection::single(0, editor.doc.len_chars());
        }

        Command::Delete { yank } => {
            if yank {
                editor.yank_selection();
            }
            if editor.selection.primary().is_empty() {
                let slice = editor.doc.slice(..);
                editor.selection.transform_mut(|r| {
                    *r = movement::move_horizontally(slice, *r, MoveDir::Forward, 1, true);
                });
            }
            if !editor.selection.primary().is_empty() {
                editor.save_undo_state();
                let tx = Transaction::delete(editor.doc.slice(..), &editor.selection);
                editor.selection = tx.map_selection(&editor.selection);
                tx.apply(&mut editor.doc);
                editor.modified = true;
            }
        }
        Command::DeleteBack => {
            let head = editor.selection.primary().head;
            if head > 0 {
                editor.save_undo_state();
                editor.selection = Selection::single(head - 1, head);
                let tx = Transaction::delete(editor.doc.slice(..), &editor.selection);
                editor.selection = tx.map_selection(&editor.selection);
                tx.apply(&mut editor.doc);
                editor.modified = true;
            }
        }
        Command::Change { yank } => {
            if yank {
                editor.yank_selection();
            }
            if !editor.selection.primary().is_empty() {
                editor.save_undo_state();
                let tx = Transaction::delete(editor.doc.slice(..), &editor.selection);
                editor.selection = tx.map_selection(&editor.selection);
                tx.apply(&mut editor.doc);
                editor.modified = true;
            }
        }
        Command::Yank => {
            editor.yank_selection();
        }
        Command::Paste { before } => {
            if before {
                editor.paste_before();
            } else {
                editor.paste_after();
            }
        }

        Command::InsertBefore => {}
        Command::InsertAfter => {
            editor.selection.transform_mut(|r| {
                *r = movement::move_horizontally(slice, *r, MoveDir::Forward, 1, false);
            });
        }
        Command::InsertLineStart => {
            editor.selection.transform_mut(|r| {
                *r = movement::move_to_first_nonwhitespace(slice, *r, false);
            });
        }
        Command::InsertLineEnd => {
            editor.selection.transform_mut(|r| {
                *r = movement::move_to_line_end(slice, *r, false);
            });
        }
        Command::OpenBelow => {
            editor.selection.transform_mut(|r| {
                *r = movement::move_to_line_end(slice, *r, false);
            });
            editor.insert_text("\n");
        }
        Command::OpenAbove => {
            editor.selection.transform_mut(|r| {
                *r = movement::move_to_line_start(slice, *r, false);
            });
            editor.insert_text("\n");
            editor.selection.transform_mut(|r| {
                *r = movement::move_vertically(
                    editor.doc.slice(..),
                    *r,
                    MoveDir::Backward,
                    1,
                    false,
                );
            });
        }

        Command::Escape => {
            editor.selection.transform_mut(|r| {
                r.anchor = r.head;
            });
        }

        Command::ScrollHalfPageUp => {
            editor.scroll_offset = editor.scroll_offset.saturating_sub(SCROLL_HALF_PAGE);
            editor.selection.transform_mut(|r| {
                *r = movement::move_vertically(slice, *r, MoveDir::Backward, SCROLL_HALF_PAGE, false);
            });
        }
        Command::ScrollHalfPageDown => {
            editor.scroll_offset = editor.scroll_offset.saturating_add(SCROLL_HALF_PAGE);
            editor.selection.transform_mut(|r| {
                *r = movement::move_vertically(slice, *r, MoveDir::Forward, SCROLL_HALF_PAGE, false);
            });
        }
        Command::ScrollPageUp => {
            editor.scroll_offset = editor.scroll_offset.saturating_sub(SCROLL_FULL_PAGE);
            editor.selection.transform_mut(|r| {
                *r = movement::move_vertically(slice, *r, MoveDir::Backward, SCROLL_FULL_PAGE, false);
            });
        }
        Command::ScrollPageDown => {
            editor.scroll_offset = editor.scroll_offset.saturating_add(SCROLL_FULL_PAGE);
            editor.selection.transform_mut(|r| {
                *r = movement::move_vertically(slice, *r, MoveDir::Forward, SCROLL_FULL_PAGE, false);
            });
        }

        Command::ToLowerCase => {
            apply_case_conversion(editor, |c| Box::new(c.to_lowercase()));
        }
        Command::ToUpperCase => {
            apply_case_conversion(editor, |c| Box::new(c.to_uppercase()));
        }

        Command::JoinLines => {
            let primary = editor.selection.primary();
            let line = editor.doc.char_to_line(primary.head);
            if line + 1 < editor.doc.len_lines() {
                editor.save_undo_state();
                let end_of_line = editor.doc.line_to_char(line + 1) - 1;
                editor.selection = Selection::single(end_of_line, end_of_line + 1);
                let tx = Transaction::delete(editor.doc.slice(..), &editor.selection);
                editor.selection = tx.map_selection(&editor.selection);
                tx.apply(&mut editor.doc);
                let tx = Transaction::insert(editor.doc.slice(..), &editor.selection, " ".to_string());
                tx.apply(&mut editor.doc);
                let head = editor.selection.primary().head + 1;
                editor.selection = Selection::point(head);
                editor.modified = true;
            }
        }

        Command::Indent => {
            editor.selection.transform_mut(|r| {
                *r = movement::move_to_line_start(slice, *r, false);
            });
            editor.insert_text("    ");
        }
        Command::Deindent => {
            let line = editor.doc.char_to_line(editor.selection.primary().head);
            let line_start = editor.doc.line_to_char(line);
            let line_text: String = editor.doc.line(line).chars().take(4).collect();
            let spaces = line_text.chars().take_while(|c| *c == ' ').count().min(4);
            if spaces > 0 {
                editor.save_undo_state();
                editor.selection = Selection::single(line_start, line_start + spaces);
                let tx = Transaction::delete(editor.doc.slice(..), &editor.selection);
                editor.selection = tx.map_selection(&editor.selection);
                tx.apply(&mut editor.doc);
                editor.modified = true;
            }
        }

        Command::Undo => {
            editor.undo();
        }
        Command::Redo => {
            editor.redo();
        }

        Command::SelectObject { trigger, selection } => {
            if let Some(ch) = trigger {
                match selection {
                    ObjectSelection::Inner => { editor.select_object_by_trigger(ch, true); }
                    ObjectSelection::Around => { editor.select_object_by_trigger(ch, false); }
                    ObjectSelection::ToStart => { editor.select_to_object_boundary(ch, true, extend); }
                    ObjectSelection::ToEnd => { editor.select_to_object_boundary(ch, false, extend); }
                }
            }
        }

        _ => {
            editor.message = Some(format!("{:?} not implemented", cmd));
        }
    }

    false
}
