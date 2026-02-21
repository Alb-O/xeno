use std::collections::{HashMap, HashSet};

use crate::core::{LinkedMetaOwned, RegistrySource};
use crate::defs::spec::MetaCommonSpec;

/// Builds `LinkedMetaOwned` from `MetaCommonSpec` with consistent defaults.
pub fn linked_meta_from_spec(common: &MetaCommonSpec) -> LinkedMetaOwned {
	LinkedMetaOwned {
		id: format!("xeno-registry::{}", common.name),
		name: common.name.clone(),
		keys: common.keys.clone(),
		description: common.description.clone(),
		priority: common.priority,
		source: RegistrySource::Crate(env!("CARGO_PKG_NAME")),
		mutates_buffer: false,
		short_desc: common.short_desc.clone().unwrap_or_else(|| common.description.clone()),
	}
}

/// Links definition specs with handler statics using a name-based bijection.
///
/// Panics if any spec entry has no matching handler, or vice versa.
pub fn link_by_name<M, H: 'static, Out>(
	metas: &[M],
	handlers: impl Iterator<Item = &'static H>,
	meta_name: impl Fn(&M) -> &str,
	handler_name: impl Fn(&H) -> &str,
	build: impl Fn(&M, &'static H) -> Out,
	what: &'static str,
) -> Vec<Out> {
	let mut handler_map: HashMap<&str, &H> = HashMap::new();
	let mut dup_handlers = Vec::new();

	for h in handlers {
		let name = handler_name(h);
		if handler_map.insert(name, h).is_some() {
			dup_handlers.push(name.to_string());
		}
	}

	let mut defs = Vec::with_capacity(metas.len());
	let mut used_handlers = HashSet::with_capacity(handler_map.len());
	let mut dup_metas = Vec::new();
	let mut seen_metas = HashSet::new();
	let mut missing_handlers = Vec::new();

	for meta in metas {
		let name = meta_name(meta);
		if !seen_metas.insert(name) {
			dup_metas.push(name.to_string());
			continue;
		}

		if let Some(handler) = handler_map.get(name) {
			used_handlers.insert(name);
			defs.push(build(meta, handler));
		} else {
			missing_handlers.push(name.to_string());
		}
	}

	let extra_handlers: Vec<String> = handler_map
		.keys()
		.filter(|&&name| !used_handlers.contains(name))
		.map(|&s| s.to_string())
		.collect();

	if !dup_handlers.is_empty() || !dup_metas.is_empty() || !missing_handlers.is_empty() || !extra_handlers.is_empty() {
		let mut report = format!("link_by_name({}) failed:\n", what);

		fn append_list(report: &mut String, title: &str, mut list: Vec<String>) {
			if !list.is_empty() {
				list.sort();
				list.dedup();
				report.push_str(&format!("  {} ({}):\n", title, list.len()));
				for item in list {
					report.push_str(&format!("    - {}\n", item));
				}
			}
		}

		append_list(&mut report, "duplicate handlers", dup_handlers);
		append_list(&mut report, "duplicate spec entries", dup_metas);
		append_list(&mut report, "spec entries missing handlers", missing_handlers);
		append_list(&mut report, "handlers missing spec entries", extra_handlers);

		panic!("{}", report);
	}

	defs
}

/// Builds a name-to-handler map from an iterator of statics.
pub fn build_name_map<H: 'static>(handlers: impl Iterator<Item = &'static H>, name: impl Fn(&'static H) -> &'static str) -> HashMap<&'static str, &'static H> {
	let mut out = HashMap::new();
	for h in handlers {
		let n = name(h);
		if out.insert(n, h).is_some() {
			panic!("duplicate static registration for '{}'", n);
		}
	}
	out
}
