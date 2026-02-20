//! NUON → [`ThemesSpec`] compiler.

use std::collections::HashSet;

use crate::schema::themes::*;
use crate::build_support::compile::*;

pub fn build(ctx: &BuildCtx) {
	let root = ctx.asset("src/domains/themes/assets");
	ctx.rerun_tree(&root);

	let mut themes: Vec<ThemeSpec> = collect_files_sorted(&root, "nuon").iter().map(|path| read_nuon_spec(path)).collect();

	themes.sort_by(|a, b| a.common.name.cmp(&b.common.name));

	let mut seen = HashSet::new();
	for theme in &themes {
		if !seen.insert(&theme.common.name) {
			panic!("duplicate theme name: '{}'", theme.common.name);
		}
	}

	let spec = ThemesSpec { themes };
	let bin = postcard::to_stdvec(&spec).expect("failed to serialize themes spec");
	ctx.write_blob("themes.bin", &bin);
}
