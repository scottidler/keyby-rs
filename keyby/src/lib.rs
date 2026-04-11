pub use keyby_derive::KeyBy;

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use std::collections::HashMap;

    #[derive(Debug, Clone, Deserialize, KeyBy)]
    #[key_by(field = "name")]
    pub struct Task {
        #[serde(skip_deserializing)]
        pub name: String,
        pub help: Option<String>,
        pub bash: String,
    }

    #[derive(Debug, Deserialize)]
    pub struct Config {
        #[serde(default, deserialize_with = "Task::deserialize_key_by_map")]
        pub tasks: HashMap<String, Task>,
    }

    #[test]
    fn basic_keyed_map() {
        let yaml = r#"
tasks:
  lint:
    help: "Run linting"
    bash: "cargo clippy"
  test:
    help: "Run tests"
    bash: "cargo test"
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.tasks.len(), 2);

        let lint = &config.tasks["lint"];
        assert_eq!(lint.name, "lint");
        assert_eq!(lint.help.as_deref(), Some("Run linting"));
        assert_eq!(lint.bash, "cargo clippy");

        let test = &config.tasks["test"];
        assert_eq!(test.name, "test");
        assert_eq!(test.bash, "cargo test");
    }

    #[test]
    fn empty_map() {
        let yaml = "tasks: {}";
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert!(config.tasks.is_empty());
    }

    #[test]
    fn single_entry() {
        let yaml = r#"
tasks:
  build:
    bash: "cargo build"
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.tasks.len(), 1);
        let build = &config.tasks["build"];
        assert_eq!(build.name, "build");
        assert_eq!(build.bash, "cargo build");
        assert!(build.help.is_none());
    }

    #[test]
    fn missing_optional_fields() {
        let yaml = r#"
tasks:
  deploy:
    bash: "cargo install --path ."
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        let deploy = &config.tasks["deploy"];
        assert_eq!(deploy.name, "deploy");
        assert!(deploy.help.is_none());
    }

    #[test]
    fn default_empty_when_missing() {
        let yaml = "{}";
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert!(config.tasks.is_empty());
    }

    #[test]
    fn key_injected_matches_map_key() {
        let yaml = r#"
tasks:
  my-task:
    bash: "echo hello"
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        for (key, task) in &config.tasks {
            assert_eq!(key, &task.name);
        }
    }

    #[test]
    fn works_with_json() {
        let json = r#"{"tasks": {"fmt": {"bash": "cargo fmt"}}}"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.tasks.len(), 1);
        let fmt = &config.tasks["fmt"];
        assert_eq!(fmt.name, "fmt");
        assert_eq!(fmt.bash, "cargo fmt");
    }

    #[derive(Debug, Clone, Deserialize, KeyBy)]
    #[key_by(field = "id")]
    pub struct Param {
        #[serde(skip_deserializing)]
        pub id: String,
        #[serde(default)]
        pub help: String,
        pub default: Option<String>,
    }

    #[derive(Debug, Deserialize)]
    pub struct ParamConfig {
        #[serde(default, deserialize_with = "Param::deserialize_key_by_map")]
        pub params: HashMap<String, Param>,
    }

    #[test]
    fn custom_field_name() {
        let yaml = r#"
params:
  --verbose:
    help: "Enable verbose output"
    default: "false"
  --output:
    help: "Output path"
"#;
        let config: ParamConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.params.len(), 2);

        let verbose = &config.params["--verbose"];
        assert_eq!(verbose.id, "--verbose");
        assert_eq!(verbose.help, "Enable verbose output");
        assert_eq!(verbose.default.as_deref(), Some("false"));

        let output = &config.params["--output"];
        assert_eq!(output.id, "--output");
        assert_eq!(output.help, "Output path");
        assert!(output.default.is_none());
    }
}
