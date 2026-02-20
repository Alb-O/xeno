use std::path::Path;
use std::sync::Arc;

pub use super::queries;
use crate::core::index::Snapshot;
use crate::core::{DenseId, LanguageId, RegistryIndex, RegistryRef, RuntimeRegistry};
use crate::languages::LanguageEntry;

pub type LanguageRef = RegistryRef<LanguageEntry, LanguageId>;

pub struct LanguagesRegistry {
	pub(super) inner: RuntimeRegistry<LanguageEntry, LanguageId>,
}

impl LanguagesRegistry {
	pub fn new(builtins: RegistryIndex<LanguageEntry, LanguageId>) -> Self {
		Self {
			inner: RuntimeRegistry::new("languages", builtins),
		}
	}

	pub fn get(&self, name: &str) -> Option<LanguageRef> {
		self.inner.get(name)
	}

	pub fn get_by_id(&self, id: LanguageId) -> Option<LanguageRef> {
		self.inner.get_by_id(id)
	}

	pub fn language_for_extension(&self, ext: &str) -> Option<LanguageRef> {
		let snap = self.inner.snapshot();
		let sym = snap.interner.get(ext)?;
		best_match(&snap, |entry| entry.extensions.contains(&sym))
	}

	pub fn language_for_filename(&self, filename: &str) -> Option<LanguageRef> {
		let snap = self.inner.snapshot();
		let sym = snap.interner.get(filename)?;
		best_match(&snap, |entry| entry.filenames.contains(&sym))
	}

	pub fn language_for_shebang(&self, interpreter: &str) -> Option<LanguageRef> {
		let snap = self.inner.snapshot();
		let sym = snap.interner.get(interpreter)?;
		best_match(&snap, |entry| entry.shebangs.contains(&sym))
	}

	pub fn globs(&self) -> Vec<(String, LanguageId)> {
		let snap = self.inner.snapshot();
		let mut out = Vec::new();
		for (idx, entry) in snap.table.iter().enumerate() {
			let id = LanguageId::from_u32(idx as u32);
			for &sym in entry.globs.iter() {
				out.push((snap.interner.resolve(sym).to_string(), id));
			}
		}
		out
	}

	pub fn resolve_path(&self, path: &Path) -> Option<LanguageRef> {
		let filename = path.file_name().and_then(|n| n.to_str());
		if let Some(name) = filename
			&& let Some(r) = self.language_for_filename(name)
		{
			return Some(r);
		}

		if let Some(ext) = path.extension().and_then(|e| e.to_str())
			&& let Some(r) = self.language_for_extension(ext)
		{
			return Some(r);
		}

		let snap = self.inner.snapshot();
		let path_str = path.to_string_lossy();

		best_match(&snap, |entry| {
			entry.globs.iter().any(|&pattern_sym| {
				let pattern = snap.interner.resolve(pattern_sym);
				glob_matches(pattern, &path_str, filename)
			})
		})
	}

	pub fn all(&self) -> Vec<LanguageRef> {
		self.inner.snapshot_guard().iter_refs().collect()
	}

	pub fn snapshot_guard(&self) -> crate::core::index::SnapshotGuard<LanguageEntry, LanguageId> {
		self.inner.snapshot_guard()
	}

	pub fn len(&self) -> usize {
		self.inner.len()
	}

	pub fn len_u32(&self) -> u32 {
		self.inner.len() as u32
	}

	pub fn is_empty(&self) -> bool {
		self.inner.is_empty()
	}

	pub fn collisions(&self) -> &[crate::core::Collision] {
		self.inner.collisions()
	}
}

fn best_match(snap: &Arc<Snapshot<LanguageEntry, LanguageId>>, matches: impl Fn(&LanguageEntry) -> bool) -> Option<LanguageRef> {
	let mut winner: Option<(LanguageId, crate::core::Party)> = None;

	for (idx, entry) in snap.table.iter().enumerate() {
		if !matches(entry) {
			continue;
		}

		let id = LanguageId::from_u32(idx as u32);
		let party = snap.parties[idx];

		match winner {
			None => winner = Some((id, party)),
			Some((_, best_party)) => {
				if crate::core::index::precedence::party_wins(&party, &best_party) {
					winner = Some((id, party));
				}
			}
		}
	}

	winner.map(|(id, _)| RegistryRef { snap: snap.clone(), id })
}

fn glob_matches(pattern: &str, path: &str, filename: Option<&str>) -> bool {
	if !pattern.contains('/') {
		return filename.is_some_and(|f| glob_match_simple(pattern, f));
	}
	glob_match_simple(pattern, path)
}

fn glob_match_simple(pattern: &str, text: &str) -> bool {
	let mut p = pattern.chars().peekable();
	let mut t = text.chars().peekable();

	while let Some(pc) = p.next() {
		match pc {
			'*' => {
				if p.peek() == Some(&'*') {
					p.next();
					let remaining: String = p.collect();
					if remaining.is_empty() {
						return true;
					}
					let rest: String = t.collect();
					return (0..=rest.len()).any(|i| glob_match_simple(&remaining, &rest[i..]));
				}

				let remaining: String = p.collect();
				if remaining.is_empty() {
					return !t.any(|c| c == '/');
				}

				let rest: String = t.collect();
				for (i, c) in rest.char_indices() {
					if c == '/' {
						break;
					}
					if glob_match_simple(&remaining, &rest[i..]) {
						return true;
					}
				}
				return glob_match_simple(&remaining, "");
			}
			'?' if t.next().is_none() => return false,
			'?' => {}
			c if t.next() != Some(c) => return false,
			_ => {}
		}
	}

	t.next().is_none()
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::core::index::RegistryBuilder;
	use crate::core::{RegistryMetaStatic, RegistrySource};
	use crate::languages::types::LanguageDef;
	use crate::languages::{LanguageEntry, LanguageInput};

	static BUILTIN_LANG: LanguageDef = LanguageDef {
		meta: RegistryMetaStatic {
			id: "registry::languages::builtin",
			name: "builtin",
			keys: &[],
			description: "builtin language",
			priority: 0,
			source: RegistrySource::Builtin,
			required_caps: &[],
			flags: 0,
		},
		scope: None,
		grammar_name: None,
		injection_regex: None,
		auto_format: false,
		extensions: &["rs"],
		filenames: &[],
		globs: &[],
		shebangs: &[],
		comment_tokens: &[],
		block_comment: None,
		lsp_servers: &[],
		roots: &[],
	};

	static RUNTIME_LANG: LanguageDef = LanguageDef {
		meta: RegistryMetaStatic {
			id: "registry::languages::runtime",
			name: "runtime",
			keys: &[],
			description: "runtime language",
			priority: 0,
			source: RegistrySource::Runtime,
			required_caps: &[],
			flags: 0,
		},
		scope: None,
		grammar_name: None,
		injection_regex: None,
		auto_format: false,
		extensions: &["rs"],
		filenames: &[],
		globs: &[],
		shebangs: &[],
		comment_tokens: &[],
		block_comment: None,
		lsp_servers: &[],
		roots: &[],
	};

	#[test]
	fn extension_lookup_prefers_runtime_source_on_tie() {
		let mut builder: RegistryBuilder<LanguageInput, LanguageEntry, LanguageId> = RegistryBuilder::new("languages-test");
		builder.push(std::sync::Arc::new(LanguageInput::Static(BUILTIN_LANG.clone())));
		builder.push(std::sync::Arc::new(LanguageInput::Static(RUNTIME_LANG.clone())));

		let registry = LanguagesRegistry::new(builder.build());

		let resolved = registry.language_for_extension("rs").expect("extension should resolve");
		assert_eq!(resolved.id_str(), RUNTIME_LANG.meta.id);
	}
}
