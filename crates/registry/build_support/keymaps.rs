//! NUON → [`KeymapPresetSpec`] compiler.

use crate::build_support::compile::*;
use crate::schema::keymaps::KeymapPresetSpec;

pub fn build(ctx: &BuildCtx) {
	let assets_dir = ctx.asset("src/keymaps/assets");
	ctx.rerun_tree(&assets_dir);

	for path in collect_files_sorted(&assets_dir, "nuon") {
		let spec: KeymapPresetSpec = read_nuon_spec(&path);
		let bin = postcard::to_stdvec(&spec).expect("failed to serialize keymap preset");
		let blob_name = format!("keymap_{}.bin", spec.name);
		ctx.write_blob(&blob_name, &bin);
	}
}
