pub use keyby_derive::KeyBy;

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use std::collections::{BTreeMap, HashMap};

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

    // Test custom field name
    #[derive(Debug, Clone, Deserialize, KeyBy)]
    #[key_by(field = "id")]
    pub struct Param {
        #[serde(skip_deserializing)]
        pub id: String,
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

    // Test default field name ("name") when #[key_by] is bare
    #[derive(Debug, Clone, Deserialize, KeyBy)]
    #[key_by]
    pub struct Role {
        #[serde(skip_deserializing)]
        pub name: String,
        pub permissions: Vec<String>,
    }

    #[derive(Debug, Deserialize)]
    pub struct RoleConfig {
        #[serde(default, deserialize_with = "Role::deserialize_key_by_map")]
        pub roles: HashMap<String, Role>,
    }

    #[test]
    fn default_field_name() {
        let yaml = r#"
roles:
  admin:
    permissions: ["read", "write", "delete"]
  viewer:
    permissions: ["read"]
"#;
        let config: RoleConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.roles.len(), 2);
        assert_eq!(config.roles["admin"].name, "admin");
        assert_eq!(config.roles["viewer"].name, "viewer");
        assert_eq!(config.roles["admin"].permissions, vec!["read", "write", "delete"]);
    }

    // Test BTreeMap support
    #[derive(Debug, Clone, Deserialize, KeyBy)]
    #[key_by(field = "name", map = "BTreeMap")]
    pub struct Step {
        #[serde(skip_deserializing)]
        pub name: String,
        pub command: String,
    }

    #[derive(Debug, Deserialize)]
    pub struct Pipeline {
        #[serde(default, deserialize_with = "Step::deserialize_key_by_map")]
        pub steps: BTreeMap<String, Step>,
    }

    #[test]
    fn btreemap_support() {
        let yaml = r#"
steps:
  checkout:
    command: "git checkout main"
  build:
    command: "cargo build"
  deploy:
    command: "cargo install"
"#;
        let config: Pipeline = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.steps.len(), 3);

        // BTreeMap iterates in sorted order
        let keys: Vec<&String> = config.steps.keys().collect();
        assert_eq!(keys, vec!["build", "checkout", "deploy"]);

        assert_eq!(config.steps["build"].name, "build");
        assert_eq!(config.steps["build"].command, "cargo build");
    }

    // Test #[key_by] with only map specified (field defaults to "name")
    #[derive(Debug, Clone, Deserialize, KeyBy)]
    #[key_by(map = "BTreeMap")]
    pub struct Service {
        #[serde(skip_deserializing)]
        pub name: String,
        pub port: u16,
    }

    #[derive(Debug, Deserialize)]
    pub struct Cluster {
        #[serde(default, deserialize_with = "Service::deserialize_key_by_map")]
        pub services: BTreeMap<String, Service>,
    }

    #[test]
    fn btreemap_with_default_field() {
        let yaml = r#"
services:
  api:
    port: 8080
  web:
    port: 3000
"#;
        let config: Cluster = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.services.len(), 2);
        assert_eq!(config.services["api"].name, "api");
        assert_eq!(config.services["api"].port, 8080);
    }

    #[test]
    fn serde_default_on_other_fields() {
        #[derive(Debug, Clone, Deserialize, KeyBy)]
        #[key_by(field = "name")]
        pub struct Item {
            #[serde(skip_deserializing)]
            pub name: String,
            #[serde(default)]
            pub enabled: bool,
            #[serde(default = "default_priority")]
            pub priority: u32,
        }

        fn default_priority() -> u32 {
            10
        }

        #[derive(Debug, Deserialize)]
        pub struct Items {
            #[serde(default, deserialize_with = "Item::deserialize_key_by_map")]
            pub items: HashMap<String, Item>,
        }

        let yaml = r#"
items:
  foo:
    enabled: true
  bar: {}
"#;
        let config: Items = serde_yaml::from_str(yaml).unwrap();
        assert!(config.items["foo"].enabled);
        assert!(!config.items["bar"].enabled);
        assert_eq!(config.items["bar"].priority, 10);
    }
}
