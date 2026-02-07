use std::path::PathBuf;

use xeno_primitives::BoxFutureLocal;
use xeno_registry::FileOpsAccess;
use xeno_registry::commands::CommandError;

use crate::capabilities::provider::EditorCaps;

impl FileOpsAccess for EditorCaps<'_> {
	fn is_modified(&self) -> bool {
		self.ed.buffer().modified()
	}

	fn save(&mut self) -> BoxFutureLocal<'_, Result<(), CommandError>> {
		Box::pin(async move { self.ed.save().await })
	}

	fn save_as(&mut self, path: PathBuf) -> BoxFutureLocal<'_, Result<(), CommandError>> {
		Box::pin(async move { self.ed.save_as(path).await })
	}
}
