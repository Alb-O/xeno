use std::sync::Arc;

use crate::core::{BuildEntry, RegistryMeta, RegistryMetaRef, StrListRef, Symbol};

#[derive(Clone)]
pub struct LspServerEntry {
	pub meta: RegistryMeta,
	pub command: Symbol,
	pub args: Arc<[Symbol]>,
	pub environment: Arc<[(Symbol, Symbol)]>,
	pub config_json: Option<Symbol>,
	pub source: Option<Symbol>,
	pub nix: Option<Symbol>,
}

crate::impl_registry_entry!(LspServerEntry);

#[derive(Clone)]
pub struct LspServerDef {
	pub meta: crate::core::RegistryMetaStatic,
	pub command: &'static str,
	pub args: &'static [&'static str],
	pub environment: &'static [(&'static str, &'static str)],
	pub config_json: Option<&'static str>,
	pub source: Option<&'static str>,
	pub nix: Option<&'static str>,
}

impl BuildEntry<LspServerEntry> for LspServerDef {
	fn meta_ref(&self) -> RegistryMetaRef<'_> {
		RegistryMetaRef {
			id: self.meta.id,
			name: self.meta.name,
			keys: StrListRef::Static(self.meta.keys),
			description: self.meta.description,
			priority: self.meta.priority,
			source: self.meta.source,
			mutates_buffer: self.meta.mutates_buffer,
			flags: self.meta.flags,
		}
	}

	fn short_desc_str(&self) -> &str {
		""
	}

	fn collect_payload_strings<'b>(&'b self, collector: &mut crate::core::index::StringCollector<'_, 'b>) {
		collector.push(self.command);
		collector.extend(self.args.iter().copied());
		for (k, v) in self.environment {
			collector.push(k);
			collector.push(v);
		}
		collector.opt(self.config_json);
		collector.opt(self.source);
		collector.opt(self.nix);
	}

	fn build(&self, ctx: &mut dyn crate::core::index::BuildCtx, key_pool: &mut Vec<Symbol>) -> LspServerEntry {
		use crate::core::index::BuildCtxExt;

		let meta = crate::core::index::meta_build::build_meta(ctx, key_pool, self.meta_ref(), []);

		LspServerEntry {
			meta,
			command: ctx.intern(self.command),
			args: ctx.intern_slice(self.args),
			environment: self.environment.iter().map(|(k, v)| (ctx.intern(k), ctx.intern(v))).collect::<Vec<_>>().into(),
			config_json: self.config_json.map(|s| ctx.intern(s)),
			source: self.source.map(|s| ctx.intern(s)),
			nix: self.nix.map(|s| ctx.intern(s)),
		}
	}
}

pub type LspServerInput = crate::core::def_input::DefInput<LspServerDef, crate::core::LinkedDef<LspServerPayload>>;

#[derive(Clone)]
pub struct LspServerPayload {
	pub command: String,
	pub args: Vec<String>,
	pub environment: std::collections::BTreeMap<String, String>,
	pub config_json: Option<String>,
	pub source: Option<String>,
	pub nix: Option<String>,
}

impl crate::core::LinkedPayload<LspServerEntry> for LspServerPayload {
	fn collect_payload_strings<'b>(&'b self, collector: &mut crate::core::index::StringCollector<'_, 'b>) {
		collector.push(&self.command);
		collector.extend(self.args.iter().map(|s| s.as_str()));
		for (k, v) in &self.environment {
			collector.push(k);
			collector.push(v);
		}
		collector.opt(self.config_json.as_deref());
		collector.opt(self.source.as_deref());
		collector.opt(self.nix.as_deref());
	}

	fn build_entry(&self, ctx: &mut dyn crate::core::index::BuildCtx, meta: RegistryMeta, _short_desc: Symbol) -> LspServerEntry {
		LspServerEntry {
			meta,
			command: ctx.intern(&self.command),
			args: self.args.iter().map(|s| ctx.intern(s)).collect::<Vec<_>>().into(),
			environment: self.environment.iter().map(|(k, v)| (ctx.intern(k), ctx.intern(v))).collect::<Vec<_>>().into(),
			config_json: self.config_json.as_ref().map(|s| ctx.intern(s)),
			source: self.source.as_ref().map(|s| ctx.intern(s)),
			nix: self.nix.as_ref().map(|s| ctx.intern(s)),
		}
	}
}

pub fn link_lsp_servers(spec: &super::spec::LspServersSpec) -> Vec<crate::core::LinkedDef<LspServerPayload>> {
	spec.servers
		.iter()
		.map(|s| crate::core::LinkedDef {
			meta: crate::defs::link::linked_meta_from_spec(&s.common),
			payload: LspServerPayload {
				command: s.command.clone(),
				args: s.args.clone(),
				environment: s.environment.clone(),
				config_json: s.config_json.clone(),
				source: s.source.clone(),
				nix: s.nix.clone(),
			},
		})
		.collect()
}
