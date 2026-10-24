use ets_etscore::KeyPattern;

use crate::*;

// go test: internal/keybuilder/keybuilder_test.go:9 TestCyrb53
#[test]
fn test_cyrb53() {
    struct Case {
        input: &'static str,
        expected: &'static str,
    }
    let tests = [
        Case {
            input: "@effect/harness-effect-v4/ExpectedServiceIdentifier",
            expected: "fc438e0396fbb4f7",
        },
        Case { input: "@effect/harness-effect-v4/ErrorA", expected: "0e8acd2c08314dfd" },
    ];

    for tt in &tests {
        let got = cyrb53(tt.input);
        assert_eq!(
            got, tt.expected,
            "{}: Cyrb53({:?}) = {:?}, want {:?}",
            tt.input, tt.input, got, tt.expected
        );
    }
}

// go test: internal/keybuilder/keybuilder_test.go:36 TestCyrb53Length
#[test]
fn test_cyrb53_length() {
    let inputs = ["", "hello", "test/path/file", "@effect/myapp/services/AuthService"];
    for input in inputs {
        let got = cyrb53(input);
        assert_eq!(
            got.len(),
            16,
            "Cyrb53({input:?}) produced {got:?} with length {}, want 16",
            got.len()
        );
    }
}

/// Rust-only: Go's `etscore.KeyPattern{Target: .., Pattern: .., SkipLeadingPath: []string{..}}` literal.
fn kp(target: &str, pattern: &str, skip_leading_path: &[&str]) -> KeyPattern {
    KeyPattern {
        target: target.to_string(),
        pattern: pattern.to_string(),
        skip_leading_path: Some(skip_leading_path.iter().map(|s| (*s).to_string()).collect()),
    }
}

// go test: internal/keybuilder/keybuilder_test.go:47 TestCreateString
#[test]
fn test_create_string() {
    let default_service_pattern = [kp("service", "default", &["src/"])];
    let default_error_pattern = [kp("error", "default", &["src/"])];
    let default_hashed_service_pattern = [kp("service", "default-hashed", &["src/"])];
    let package_identifier_service_pattern = [kp("service", "package-identifier", &["src/"])];
    let identifier_error_pattern = [kp("error", "identifier", &["src/"])];
    let identifier_service_pattern = [kp("service", "identifier", &["src/"])];

    struct Case<'a> {
        name: &'static str,
        source_file_name: &'static str,
        package_name: &'static str,
        package_directory: &'static str,
        class_name: &'static str,
        target: &'static str,
        key_patterns: &'a [KeyPattern],
        expected: String,
    }
    let tests = [
        Case {
            name: "default pattern - file at package root, class different from file",
            source_file_name: "/.src/test.ts",
            package_name: "@effect/harness-effect-v4",
            package_directory: "/.src",
            class_name: "ExpectedServiceIdentifier",
            target: "service",
            key_patterns: &default_service_pattern,
            expected: "@effect/harness-effect-v4/test/ExpectedServiceIdentifier".to_string(),
        },
        Case {
            name: "default pattern - error target",
            source_file_name: "/.src/test.ts",
            package_name: "@effect/harness-effect-v4",
            package_directory: "/.src",
            class_name: "ErrorA",
            target: "error",
            key_patterns: &default_error_pattern,
            expected: "@effect/harness-effect-v4/test/ErrorA".to_string(),
        },
        Case {
            name: "package-identifier pattern - omits subdirectory",
            source_file_name: "/.src/test.ts",
            package_name: "@effect/harness-effect-v4",
            package_directory: "/.src",
            class_name: "ExpectedServiceIdentifier",
            target: "service",
            key_patterns: &package_identifier_service_pattern,
            expected: "@effect/harness-effect-v4/test/ExpectedServiceIdentifier".to_string(),
        },
        Case {
            name: "default-hashed pattern produces 16-char hex",
            source_file_name: "/.src/test.ts",
            package_name: "@effect/harness-effect-v4",
            package_directory: "/.src",
            class_name: "ExpectedServiceIdentifier",
            target: "service",
            key_patterns: &default_hashed_service_pattern,
            expected: cyrb53("@effect/harness-effect-v4/test/ExpectedServiceIdentifier"),
        },
        Case {
            name: "cyrb53 cross-check with known value",
            source_file_name: "/.src/test.ts",
            package_name: "@effect/harness-effect-v4",
            package_directory: "/.src",
            class_name: "ExpectedServiceIdentifier",
            target: "service",
            key_patterns: &[kp("service", "default", &["src/"])],
            expected: "@effect/harness-effect-v4/test/ExpectedServiceIdentifier".to_string(),
        },
        Case {
            name: "class name matches file name - omits class name",
            source_file_name: "/project/src/AuthService.ts",
            package_name: "@effect/myapp",
            package_directory: "/project",
            class_name: "AuthService",
            target: "service",
            key_patterns: &default_service_pattern,
            expected: "@effect/myapp/AuthService".to_string(),
        },
        Case {
            name: "class name matches file name case-insensitive - omits class name, uses file casing",
            source_file_name: "/project/src/authservice.ts",
            package_name: "@effect/myapp",
            package_directory: "/project",
            class_name: "AuthService",
            target: "service",
            key_patterns: &default_service_pattern,
            expected: "@effect/myapp/authservice".to_string(),
        },
        Case {
            name: "index file name is stripped",
            source_file_name: "/project/src/services/index.ts",
            package_name: "@effect/myapp",
            package_directory: "/project",
            class_name: "AuthService",
            target: "service",
            key_patterns: &default_service_pattern,
            expected: "@effect/myapp/services/AuthService".to_string(),
        },
        Case {
            name: "skipLeadingPath strips src/ prefix from subdirectory",
            source_file_name: "/project/src/services/AuthService.ts",
            package_name: "@effect/myapp",
            package_directory: "/project",
            class_name: "AuthService",
            target: "service",
            key_patterns: &default_service_pattern,
            expected: "@effect/myapp/services/AuthService".to_string(),
        },
        Case {
            name: "subdirectory preserved when not matching skipLeadingPath",
            source_file_name: "/project/lib/services/AuthService.ts",
            package_name: "@effect/myapp",
            package_directory: "/project",
            class_name: "MyService",
            target: "service",
            key_patterns: &default_service_pattern,
            expected: "@effect/myapp/lib/services/AuthService/MyService".to_string(),
        },
        Case {
            name: "package-identifier omits subdirectory",
            source_file_name: "/project/lib/services/AuthService.ts",
            package_name: "@effect/myapp",
            package_directory: "/project",
            class_name: "MyService",
            target: "service",
            key_patterns: &package_identifier_service_pattern,
            expected: "@effect/myapp/AuthService/MyService".to_string(),
        },
        Case {
            name: "identifier pattern - nested subdirectory, class different from file",
            source_file_name: "/project/src/errors/keybindings.ts",
            package_name: "@effect/myapp",
            package_directory: "/project",
            class_name: "KeybindingsConfigError",
            target: "error",
            key_patterns: &identifier_error_pattern,
            expected: "KeybindingsConfigError".to_string(),
        },
        Case {
            name: "identifier pattern - class name matches file name",
            source_file_name: "/project/src/errors/KeybindingsConfigError.ts",
            package_name: "@effect/myapp",
            package_directory: "/project",
            class_name: "KeybindingsConfigError",
            target: "error",
            key_patterns: &identifier_error_pattern,
            expected: "KeybindingsConfigError".to_string(),
        },
        Case {
            name: "no matching target - returns empty",
            source_file_name: "/project/src/file.ts",
            package_name: "@effect/myapp",
            package_directory: "/project",
            class_name: "Foo",
            target: "error",
            key_patterns: &default_service_pattern,
            expected: String::new(),
        },
        Case {
            name: "empty package name - default pattern omits package",
            source_file_name: "/project/src/services/AuthService.ts",
            package_name: "",
            package_directory: "/project",
            class_name: "MyService",
            target: "service",
            key_patterns: &default_service_pattern,
            expected: "services/AuthService/MyService".to_string(),
        },
        Case {
            name: "empty package name - package-identifier pattern omits package",
            source_file_name: "/project/src/services/AuthService.ts",
            package_name: "",
            package_directory: "/project",
            class_name: "MyService",
            target: "service",
            key_patterns: &package_identifier_service_pattern,
            expected: "AuthService/MyService".to_string(),
        },
        Case {
            name: "empty package name - identifier pattern omits package",
            source_file_name: "/project/src/services/AuthService.ts",
            package_name: "",
            package_directory: "/project",
            class_name: "MyService",
            target: "service",
            key_patterns: &identifier_service_pattern,
            expected: "MyService".to_string(),
        },
        Case {
            name: "empty package name - default-hashed pattern omits package",
            source_file_name: "/project/src/services/AuthService.ts",
            package_name: "",
            package_directory: "/project",
            class_name: "MyService",
            target: "service",
            key_patterns: &default_hashed_service_pattern,
            expected: cyrb53("services/AuthService/MyService"),
        },
        Case {
            name: "source file not under package directory - returns empty (skips)",
            source_file_name: "/other/file.ts",
            package_name: "@effect/myapp",
            package_directory: "/project",
            class_name: "Foo",
            target: "service",
            key_patterns: &default_service_pattern,
            expected: String::new(),
        },
    ];

    for tt in &tests {
        let got = create_string(
            tt.source_file_name,
            tt.package_name,
            tt.package_directory,
            tt.class_name,
            tt.target,
            tt.key_patterns,
        );
        assert_eq!(
            got,
            tt.expected,
            "{}: CreateString({:?}, {:?}, {:?}, {:?}, {:?}, ...) = {:?}, want {:?}",
            tt.name,
            tt.source_file_name,
            tt.package_name,
            tt.package_directory,
            tt.class_name,
            tt.target,
            got,
            tt.expected
        );
    }
}
