use super::*;

/// Computes derived policy state from context and policy. Pure computation.
pub(super) fn derive<'a>(ctx: &'a EnsureSyntaxContext<'a>, policy: &TieredSyntaxPolicy) -> EnsureBase<'a> {
	let bytes = ctx.content.len_bytes();
	let bytes_u32 = (bytes.min(u32::MAX as usize)) as u32;
	let tier = policy.tier_for_bytes(bytes);
	let cfg = policy.cfg(tier);
	let opts_key = OptKey { injections: cfg.injections };
	let viewport = ctx.viewport.as_ref().map(|raw| {
		let start = raw.start.min(bytes_u32);
		let mut end = raw.end.min(bytes_u32);
		if end < start {
			end = start;
		}
		let capped_end = start.saturating_add(cfg.viewport_visible_span_cap);
		end = end.min(capped_end);
		start..end
	});
	let work_disabled = matches!(ctx.hotness, SyntaxHotness::Cold) && !cfg.parse_when_hidden;
	EnsureBase {
		doc_id: ctx.doc_id,
		doc_version: ctx.doc_version,
		language_id: ctx.language_id,
		content: ctx.content,
		hotness: ctx.hotness,
		loader: ctx.loader,
		bytes,
		bytes_u32,
		tier,
		cfg,
		opts_key,
		viewport,
		work_disabled,
	}
}
