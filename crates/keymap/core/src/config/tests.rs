use super::*;
const CONFIG: &str = r#"
        Create = { keys = ["c"], description = "Create a new item" }
        Delete = { keys = ["d", "d e", "@digit"], description = "Delete an item" }
    "#;

// #[derive(KeyMap)]
#[derive(Debug, Deserialize, PartialEq, Eq, Hash)]
enum Action {
	// #[key("n")]
	Create,
	// #[key("u")]
	Update,
	Delete,
}

impl KeyMapConfig<Action> for Action {
	fn keymap_config() -> Config<Action> {
		Config::new(vec![
			(Action::Create, Item::new(vec!["n".into()], "".into())),
			(Action::Update, Item::new(vec!["u".into()], "".into())),
			(Action::Delete, Item::new(vec![], "".into())),
		])
	}

	fn keymap_item(&self) -> Item {
		match self {
			Action::Create => Item::new(vec!["n".into()], "".into()),
			Action::Update => Item::new(vec!["u".into()], "".into()),
			Action::Delete => Item::new(vec![], "".into()),
		}
	}
}

#[test]
fn test_deserialize_string_keys() {
	let config: Config<String> = toml::from_str(CONFIG).unwrap();

	// Reverse lookup by key string "c"
	let (action, item) = config.get_item_by_key_str("c").unwrap();
	assert_eq!(action, "Create");
	assert_eq!(item.description, "Create a new item");

	// Reverse lookup by parsed sequence ["d", "e"]
	let (action, item) = config
		.get_item_by_keymaps(&parse_seq("d e").unwrap())
		.unwrap();
	assert_eq!(action, "Delete");
	assert_eq!(item.description, "Delete an item");

	// Test special @digit group: any digit character should map to Delete
	let (action, _) = config.get_item_by_key_str("1").unwrap();
	assert_eq!(action, "Delete");
}

#[test]
fn test_deserialize_enum_keys() {
	let config: Config<Action> = toml::from_str(CONFIG).unwrap();

	// Reverse lookup by key "c"
	let (action, _) = config.get_item_by_key_str("c").unwrap();
	assert_eq!(*action, Action::Create);

	// No "u" in user config, so should return None
	assert!(config.get_item_by_key_str("u").is_none());

	// "d" maps to Delete
	let (action, _) = config.get_item_by_key_str("d").unwrap();
	assert_eq!(*action, Action::Delete);

	// Test @digit group on enums
	let (action, _) = config.get_item_by_key_str("1").unwrap();
	assert_eq!(*action, Action::Delete);
}

#[test]
fn test_deserialize_with_override() {
	let config: DerivedConfig<Action> = toml::from_str(CONFIG).unwrap();

	// "c" was provided by user config
	let (action, _) = config.get_item_by_key_str("c").unwrap();
	assert_eq!(*action, Action::Create);

	// "u" falls back to default from KeyMapConfig
	let (action, _) = config.get_item_by_key_str("u").unwrap();
	assert_eq!(*action, Action::Update);

	// "d" was provided by user config
	let (action, _) = config.get_item_by_key_str("d").unwrap();
	assert_eq!(*action, Action::Delete);
}
