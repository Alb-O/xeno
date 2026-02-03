use super::helpers::*;

#[test]
fn test_search_single_term() {
	let (bm25, _temp_dir) = setup_bm25_config();
	let mut wtxn = bm25.graph_env.write_txn().unwrap();
	let arena = Bump::new();

	// model properties list stored in nodes
	let props1: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("Swift shadow leaps".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("Idle fox wolf rests".to_string()),
		),
	]);

	let props2: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("Rapid hare bounds".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("Quiet bear naps".to_string()),
		),
	]);

	let props3: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("Fleet deer fox sprints".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("Calm owl dozes".to_string()),
		),
	]);

	let nodes = [props1, props2, props3];

	for (i, props) in nodes.iter().enumerate() {
		let props_map = ImmutablePropertiesMap::new(
			props.len(),
			props
				.iter()
				.map(|(k, v)| (arena.alloc_str(k) as &str, v.clone())),
			&arena,
		);
		let data = props_map.flatten_bm25();
		bm25.insert_doc(&mut wtxn, i as u128, &data).unwrap();
	}
	wtxn.commit().unwrap();

	// search for "fox"
	let rtxn = bm25.graph_env.read_txn().unwrap();
	let arena = Bump::new();
	let results = bm25.search(&rtxn, "fox", 10, &arena).unwrap();

	tracing::debug!(results = ?results, "bm25 search results");

	// should return documents 1 and 3 (both contain "fox")
	assert_eq!(results.len(), 2);

	let doc_ids: Vec<u128> = results.iter().map(|(id, _)| *id).collect();
	assert!(doc_ids.contains(&0u128));
	assert!(doc_ids.contains(&2u128));

	// scores should be positive
	for (_, score) in &results {
		assert!(*score != 0.0);
	}
}

#[test]
fn test_search_multiple_terms() {
	let (bm25, _temp_dir) = setup_bm25_config();
	let mut wtxn = bm25.graph_env.write_txn().unwrap();
	let arena = Bump::new();

	let props1: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("machine learning algorithms for data science".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("deep learning".to_string()),
		),
	]);

	let props2: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("deep learning neural networks".to_string()),
		),
		("label2".to_string(), Value::I64(6969)),
	]);

	let props3: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("data analysis and machine learning".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("natural language processing".to_string()),
		),
	]);

	let nodes = [props1, props2, props3];

	for (i, props) in nodes.iter().enumerate() {
		let props_map = ImmutablePropertiesMap::new(
			props.len(),
			props
				.iter()
				.map(|(k, v)| (arena.alloc_str(k) as &str, v.clone())),
			&arena,
		);
		let data = props_map.flatten_bm25();
		bm25.insert_doc(&mut wtxn, i as u128, &data).unwrap();
	}
	wtxn.commit().unwrap();

	let rtxn = bm25.graph_env.read_txn().unwrap();
	let arena = Bump::new();
	let results = bm25.search(&rtxn, "machine learning", 10, &arena).unwrap();

	tracing::debug!(results = ?results, "bm25 search results");

	// documents 1 and 3 should score highest (contain both terms)
	assert!(results.len() >= 2);

	let doc_ids: Vec<u128> = results.iter().map(|(id, _)| *id).collect();
	assert!(doc_ids.contains(&0u128));
	assert!(doc_ids.contains(&1u128));
	assert!(doc_ids.contains(&2u128));
}

#[test]
fn test_search_many_terms() {
	let (bm25, _temp_dir) = setup_bm25_config();
	let mut wtxn = bm25.graph_env.write_txn().unwrap();
	let arena = Bump::new();

	let props1: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("machine learning algorithms for data science".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("deep learning".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("neural networks optimization".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("data analysis techniques".to_string()),
		),
	]);

	let props2: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("deep learning neural networks".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("machine learning".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("computer vision models".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("reinforcement learning".to_string()),
		),
	]);

	let props3: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("data analysis and machine learning".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("natural language processing".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("sentiment analysis".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("text mining".to_string()),
		),
	]);

	let props4: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("machine learning for predictive analytics".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("deep learning frameworks".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("image recognition".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("data preprocessing".to_string()),
		),
	]);

	let props5: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("neural networks for data science".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("machine learning pipelines".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("feature engineering".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("model evaluation".to_string()),
		),
	]);

	let props6: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("deep learning for image processing".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("machine learning models".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("clustering algorithms".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("dimensionality reduction".to_string()),
		),
	]);

	let props7: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("natural language processing techniques".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("machine learning applications".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("text classification".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("data visualization".to_string()),
		),
	]);

	let props8: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("machine learning for time series".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("deep learning architectures".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("anomaly detection".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("predictive modeling".to_string()),
		),
	]);

	let props9: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("data science with machine learning".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("neural networks training".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("regression analysis".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("model optimization".to_string()),
		),
	]);

	let props10: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("deep learning for speech recognition".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("machine learning workflows".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("audio processing".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("data augmentation".to_string()),
		),
	]);

	let props11: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("machine learning for fraud detection".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("deep learning systems".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("pattern recognition".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("data cleaning".to_string()),
		),
	]);

	let props12: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("natural language processing models".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("machine learning algorithms".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("topic modeling".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("text analytics".to_string()),
		),
	]);

	let props13: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("machine learning for recommendation systems".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("deep learning techniques".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("collaborative filtering".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("user profiling".to_string()),
		),
	]);

	let props14: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("data science and neural networks".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("machine learning strategies".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("classification models".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("data exploration".to_string()),
		),
	]);

	let props15: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("deep learning for object detection".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("machine learning tools".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("image segmentation".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("feature extraction".to_string()),
		),
	]);

	let props16: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("machine learning for customer segmentation".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("deep learning applications".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("market analysis".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("data clustering".to_string()),
		),
	]);

	let props17: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("natural language processing for chatbots".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("machine learning frameworks".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("dialogue systems".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("text generation".to_string()),
		),
	]);

	let props18: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("machine learning for predictive maintenance".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("deep learning models".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("equipment monitoring".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("failure prediction".to_string()),
		),
	]);

	let props19: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("data science with deep learning".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("machine learning techniques".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("statistical modeling".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("data interpretation".to_string()),
		),
	]);

	let props20: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("deep learning for facial recognition".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("machine learning processes".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("biometric analysis".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("identity verification".to_string()),
		),
	]);

	let props21: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("machine learning for supply chain".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("deep learning optimization".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("inventory management".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("demand forecasting".to_string()),
		),
	]);

	let props22: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("natural language processing for sentiment".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("machine learning solutions".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("opinion mining".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("text processing".to_string()),
		),
	]);

	let props23: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("machine learning for risk assessment".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("deep learning algorithms".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("probability analysis".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("data modeling".to_string()),
		),
	]);

	let props24: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("data science for business intelligence".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("machine learning insights".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("decision support".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("data reporting".to_string()),
		),
	]);

	let props25: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("deep learning for autonomous vehicles".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("machine learning systems".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("path planning".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("sensor fusion".to_string()),
		),
	]);

	let props26: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("machine learning for healthcare".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("deep learning diagnostics".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("medical imaging".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("patient data analysis".to_string()),
		),
	]);

	let props27: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("natural language processing for translation".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("machine learning models".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("language models".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("text translation".to_string()),
		),
	]);

	let props28: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("machine learning for energy optimization".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("deep learning strategies".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("energy forecasting".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("resource allocation".to_string()),
		),
	]);

	let props29: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("data science for marketing".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("machine learning analytics".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("customer insights".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("campaign analysis".to_string()),
		),
	]);

	let props30: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("deep learning for video analysis".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("machine learning pipelines".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("motion detection".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("frame analysis".to_string()),
		),
	]);

	let props31: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("machine learning for cybersecurity".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("deep learning detection".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("threat analysis".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("network security".to_string()),
		),
	]);

	let props32: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("natural language processing for summarization".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("machine learning techniques".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("text summarization".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("content analysis".to_string()),
		),
	]);

	let props33: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("machine learning for logistics".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("deep learning optimization".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("route planning".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("supply chain analytics".to_string()),
		),
	]);

	let props34: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("data science for finance".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("machine learning predictions".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("market forecasting".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("risk modeling".to_string()),
		),
	]);

	let props35: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("deep learning for robotics".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("machine learning algorithms".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("motion planning".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("sensor processing".to_string()),
		),
	]);

	let props36: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("machine learning for agriculture".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("deep learning applications".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("crop monitoring".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("yield prediction".to_string()),
		),
	]);

	let props37: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("natural language processing for search".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("machine learning systems".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("query processing".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("search optimization".to_string()),
		),
	]);

	let props38: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("machine learning for retail".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("deep learning models".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("sales forecasting".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("inventory optimization".to_string()),
		),
	]);

	let props39: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("data science for education".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("machine learning tools".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("student performance".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("learning analytics".to_string()),
		),
	]);

	let props40: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("deep learning for gaming".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("machine learning strategies".to_string()),
		),
		("label3".to_string(), Value::String("game AI".to_string())),
		(
			"label4".to_string(),
			Value::String("player behavior".to_string()),
		),
	]);

	let props41: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("machine learning for transportation".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("deep learning frameworks".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("traffic prediction".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("route optimization".to_string()),
		),
	]);

	let props42: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("natural language processing for legal".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("machine learning applications".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("document analysis".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("contract review".to_string()),
		),
	]);

	let props43: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("machine learning for manufacturing".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("deep learning systems".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("quality control".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("process optimization".to_string()),
		),
	]);

	let props44: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("data science for e-commerce".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("machine learning insights".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("product recommendation".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("customer segmentation".to_string()),
		),
	]);

	let props45: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("deep learning for environmental monitoring".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("machine learning models".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("climate analysis".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("sensor data".to_string()),
		),
	]);

	let props46: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("machine learning for sports analytics".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("deep learning techniques".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("player performance".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("game strategy".to_string()),
		),
	]);

	let props47: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("natural language processing for news".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("machine learning algorithms".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("article classification".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("content summarization".to_string()),
		),
	]);

	let props48: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("machine learning for urban planning".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("deep learning applications".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("traffic modeling".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("resource planning".to_string()),
		),
	]);

	let props49: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("data science for telecommunications".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("machine learning systems".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("network optimization".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("customer analytics".to_string()),
		),
	]);

	let props50: HashMap<String, Value> = HashMap::from([
		(
			"label1".to_string(),
			Value::String("deep learning for astronomy".to_string()),
		),
		(
			"label2".to_string(),
			Value::String("machine learning frameworks".to_string()),
		),
		(
			"label3".to_string(),
			Value::String("star classification".to_string()),
		),
		(
			"label4".to_string(),
			Value::String("cosmic data analysis".to_string()),
		),
	]);

	let nodes = [
		props1, props2, props3, props4, props5, props6, props7, props8, props9, props10, props11,
		props12, props13, props14, props15, props16, props17, props18, props19, props20, props21,
		props22, props23, props24, props25, props26, props27, props28, props29, props30, props31,
		props32, props33, props34, props35, props36, props37, props38, props39, props40, props41,
		props42, props43, props44, props45, props46, props47, props48, props49, props50,
	];

	for (i, props) in nodes.iter().enumerate() {
		let props_map = ImmutablePropertiesMap::new(
			props.len(),
			props
				.iter()
				.map(|(k, v)| (arena.alloc_str(k) as &str, v.clone())),
			&arena,
		);
		let data = props_map.flatten_bm25();
		bm25.insert_doc(&mut wtxn, i as u128, &data).unwrap();
		tracing::trace!(data = ?data, "bm25 doc data");
	}
	wtxn.commit().unwrap();

	let rtxn = bm25.graph_env.read_txn().unwrap();
	let arena = Bump::new();
	let results = bm25.search(&rtxn, "science", 10, &arena).unwrap();

	tracing::debug!(results = ?results, "bm25 search results");

	assert!(results.len() >= 2);

	let doc_ids: Vec<u128> = results.iter().map(|(id, _)| *id).collect();
	assert!(doc_ids.contains(&38u128));
	assert!(doc_ids.contains(&43u128));
	assert!(doc_ids.contains(&28u128));
	assert!(doc_ids.contains(&33u128));
	assert!(doc_ids.contains(&48u128));
	assert!(doc_ids.contains(&18u128));
	assert!(doc_ids.contains(&8u128));
	assert!(doc_ids.contains(&13u128));
	assert!(doc_ids.contains(&23u128));
}

#[test]
fn test_search_with_limit() {
	let (bm25, _temp_dir) = setup_bm25_config();
	let mut wtxn = bm25.graph_env.write_txn().unwrap();

	// insert many documents containing the same term
	for i in 1..=10 {
		let doc = format!("document {i} contains test content");
		bm25.insert_doc(&mut wtxn, i as u128, &doc).unwrap();
	}
	wtxn.commit().unwrap();

	let rtxn = bm25.graph_env.read_txn().unwrap();
	let arena = Bump::new();
	let results = bm25.search(&rtxn, "test", 5, &arena).unwrap();

	// should respect the limit
	assert_eq!(results.len(), 5);

	// results should be sorted by score (descending)
	for i in 1..results.len() {
		assert!(results[i - 1].1 >= results[i].1);
	}
}

#[test]
fn test_search_no_results() {
	let (bm25, _temp_dir) = setup_bm25_config();
	let mut wtxn = bm25.graph_env.write_txn().unwrap();

	bm25.insert_doc(&mut wtxn, 1u128, "some document content")
		.unwrap();
	wtxn.commit().unwrap();

	let rtxn = bm25.graph_env.read_txn().unwrap();
	let arena = Bump::new();
	let results = bm25.search(&rtxn, "nonexistent", 10, &arena).unwrap();

	assert_eq!(results.len(), 0);
}

#[test]
fn test_edge_cases_empty_document() {
	let (bm25, _temp_dir) = setup_bm25_config();
	let mut wtxn = bm25.graph_env.write_txn().unwrap();

	// Insert empty document
	let result = bm25.insert_doc(&mut wtxn, 1u128, "");
	assert!(result.is_ok());

	// document length should be 0
	let doc_length = bm25.doc_lengths_db.get(&wtxn, &1u128).unwrap().unwrap();
	assert_eq!(doc_length, 0);

	wtxn.commit().unwrap();
}

#[test]
fn test_search_with_limit_zero() {
	let (bm25, _temp_dir) = setup_bm25_config();
	let mut wtxn = bm25.graph_env.write_txn().unwrap();

	bm25.insert_doc(&mut wtxn, 1u128, "test document content")
		.unwrap();
	wtxn.commit().unwrap();

	let rtxn = bm25.graph_env.read_txn().unwrap();
	let arena = Bump::new();
	let results = bm25.search(&rtxn, "test", 0, &arena).unwrap();

	// With limit 0, should return empty results
	assert_eq!(results.len(), 0);
}

#[test]
fn test_search_results_sorted_by_score() {
	let (bm25, _temp_dir) = setup_bm25_config();
	let mut wtxn = bm25.graph_env.write_txn().unwrap();

	// Insert documents with varying relevance to "machine learning"
	let docs = vec![
		(1u128, "machine learning machine learning machine learning"), // High relevance
		(2u128, "machine learning"),                                   // Medium relevance
		(3u128, "learning about machines"),                            // Lower relevance
		(4u128, "machine"),                                            // Lowest
	];

	for (doc_id, doc) in &docs {
		bm25.insert_doc(&mut wtxn, *doc_id, doc).unwrap();
	}
	wtxn.commit().unwrap();

	let rtxn = bm25.graph_env.read_txn().unwrap();
	let arena = Bump::new();
	let results = bm25.search(&rtxn, "machine learning", 10, &arena).unwrap();

	// Results should be sorted by score (descending)
	for i in 1..results.len() {
		assert!(
			results[i - 1].1 >= results[i].1,
			"Results not sorted: {} < {}",
			results[i - 1].1,
			results[i].1
		);
	}
}
