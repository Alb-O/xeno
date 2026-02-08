use std::collections::{HashMap, HashSet};

/// Links KDL metadata with handler statics using a name-based bijection.
///
/// Panics if any KDL entry has no matching handler, or vice versa.
pub(crate) fn link_by_name<M, H: 'static, Out>(
	metas: &[M],
	handlers: impl Iterator<Item = &'static H>,
	meta_name: impl Fn(&M) -> &str,
	handler_name: impl Fn(&H) -> &str,
	build: impl Fn(&M, &'static H) -> Out,
	what: &'static str,
) -> Vec<Out> {
	let handler_map: HashMap<&str, &H> = handlers.map(|h| (handler_name(h), h)).collect();
	let mut defs = Vec::with_capacity(metas.len());
	let mut used_handlers = HashSet::with_capacity(handler_map.len());

	for meta in metas {
		let name = meta_name(meta);
		let handler = handler_map.get(name).unwrap_or_else(|| {
			panic!(
				"KDL {} '{}' has no matching {}_handler!() in Rust",
				what, name, what
			)
		});
		used_handlers.insert(name);
		defs.push(build(meta, handler));
	}

	for name in handler_map.keys() {
		if !used_handlers.contains(name) {
			panic!(
				"{}_handler!({}) has no matching entry in {}s.kdl",
				what, name, what
			);
		}
	}

	defs
}

/// Builds a name-to-handler map from an iterator of statics.
pub(crate) fn build_name_map<H: 'static>(
	handlers: impl Iterator<Item = &'static H>,
	name: impl Fn(&'static H) -> &'static str,
) -> HashMap<&'static str, &'static H> {
	handlers.map(|h| (name(h), h)).collect()
}
