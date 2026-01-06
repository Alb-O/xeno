pub struct Config {
    pub name: String,
    pub value: i32,
    pub enabled: bool,
}

impl Config {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            value: 0,
            enabled: false,
        }
    }

    pub fn with_name(mut self, name: &str) -> Self {
        self.name = name.to_string();
        self
    }
}

fn test_completion() {
    let config = Config::new();
    // Cursor here for testing: config.
}
