use std::path::Path;

use rustc_hash::FxHashMap;

use crate::core::{DenseId, LanguageId, RegistryIndex, RegistryRef, RuntimeRegistry, Symbol};
use crate::languages::LanguageEntry;

pub type LanguageRef = RegistryRef<LanguageEntry, LanguageId>;

pub struct LanguagesRegistry {
	pub(super) inner: RuntimeRegistry<LanguageEntry, LanguageId>,
	pub(super) by_extension: FxHashMap<Symbol, LanguageId>,
	pub(super) by_filename: FxHashMap<Symbol, LanguageId>,
	pub(super) by_shebang: FxHashMap<Symbol, LanguageId>,
	pub(super) globs: Vec<(Symbol, LanguageId)>,
}

impl LanguagesRegistry {
	pub fn new(builtins: RegistryIndex<LanguageEntry, LanguageId>) -> Self {
		let mut by_extension = FxHashMap::default();
		let mut by_filename = FxHashMap::default();
		let mut by_shebang = FxHashMap::default();
		let mut globs = Vec::new();

		for (i, entry) in builtins.table.iter().enumerate() {
			let id = LanguageId::from_u32(i as u32);

			for &ext in entry.extensions.iter() {
				by_extension.entry(ext).or_insert(id);
			}
			for &name in entry.filenames.iter() {
				by_filename.entry(name).or_insert(id);
			}
			for &shebang in entry.shebangs.iter() {
				by_shebang.entry(shebang).or_insert(id);
			}
			for &glob in entry.globs.iter() {
				globs.push((glob, id));
			}
		}

		Self {
			inner: RuntimeRegistry::new("languages", builtins),
			by_extension,
			by_filename,
			by_shebang,
			globs,
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
		let id = self.by_extension.get(&sym)?;
		Some(RegistryRef { snap, id: *id })
	}

	pub fn language_for_filename(&self, filename: &str) -> Option<LanguageRef> {
		let snap = self.inner.snapshot();
		let sym = snap.interner.get(filename)?;
		let id = self.by_filename.get(&sym)?;
		Some(RegistryRef { snap, id: *id })
	}

	pub fn language_for_shebang(&self, interpreter: &str) -> Option<LanguageRef> {
		let snap = self.inner.snapshot();
		let sym = snap.interner.get(interpreter)?;
		let id = self.by_shebang.get(&sym)?;
		Some(RegistryRef { snap, id: *id })
	}

	pub fn globs(&self) -> Vec<(String, LanguageId)> {
		let snap = self.inner.snapshot();
		self.globs
			.iter()
			.map(|(sym, id)| (snap.interner.resolve(*sym).to_string(), *id))
			.collect()
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
		for (pattern_sym, id) in &self.globs {
			let pattern = snap.interner.resolve(*pattern_sym);
			if glob_matches(pattern, &path_str, filename) {
				return Some(RegistryRef {
					snap: snap.clone(),
					id: *id,
				});
			}
		}

		None
	}

	pub fn all(&self) -> Vec<LanguageRef> {
		self.inner.all()
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
