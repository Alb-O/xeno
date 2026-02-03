pub(super) use std::collections::HashMap;

pub(super) use bumpalo::Bump;
pub(super) use heed3::{Env, EnvOpenOptions, RoTxn};
pub(super) use rand::Rng;
pub(super) use tempfile::tempdir;

pub(super) use crate::helix_engine::bm25::{
	BM25, BM25Flatten, BM25Metadata, HBM25Config, HybridSearch, METADATA_KEY,
};
pub(super) use crate::helix_engine::storage_core::HelixGraphStorage;
pub(super) use crate::helix_engine::storage_core::version_info::VersionInfo;
pub(super) use crate::helix_engine::traversal_core::config::Config;
pub(super) use crate::helix_engine::vector_core::hnsw::HNSW;
pub(super) use crate::helix_engine::vector_core::vector::HVector;
pub(super) use crate::protocol::value::Value;
pub(super) use crate::utils::properties::ImmutablePropertiesMap;

pub(super) fn setup_test_env() -> (Env, tempfile::TempDir) {
	let temp_dir = tempdir().unwrap();
	let path = temp_dir.path();

	let env = unsafe {
		EnvOpenOptions::new()
			.map_size(4 * 1024 * 1024 * 1024) // 4GB
			.max_dbs(20)
			.open(path)
			.unwrap()
	};

	(env, temp_dir)
}

pub(super) fn setup_bm25_config() -> (HBM25Config, tempfile::TempDir) {
	let (env, temp_dir) = setup_test_env();
	let mut wtxn = env.write_txn().unwrap();
	let config = HBM25Config::new(&env, &mut wtxn).unwrap();
	wtxn.commit().unwrap();
	(config, temp_dir)
}

pub(super) fn setup_helix_storage() -> (HelixGraphStorage, tempfile::TempDir) {
	let temp_dir = tempdir().unwrap();
	let path = temp_dir.path().to_str().unwrap();
	let config = Config::default();
	let storage = HelixGraphStorage::new(path, config, VersionInfo::default()).unwrap();
	(storage, temp_dir)
}

pub(super) fn generate_random_vectors(n: usize, d: usize) -> Vec<Vec<f64>> {
	let mut rng = rand::rng();
	let mut vectors = Vec::with_capacity(n);

	for _ in 0..n {
		let mut vector = Vec::with_capacity(d);
		for _ in 0..d {
			vector.push(rng.random::<f64>());
		}
		vectors.push(vector);
	}

	vectors
}
