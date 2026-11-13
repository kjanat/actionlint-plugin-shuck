use actionlint_extension_api::package::Capabilities;
use actionlint_extension_api::package::Engine;
use actionlint_extension_api::package::Identity;
use actionlint_extension_api::package::Package;
use std::error::Error;
use std::path::Path;

#[derive(serde::Deserialize)]
struct Lockfile {
    package: Vec<LockedPackage>,
}

#[derive(serde::Deserialize)]
struct LockedPackage {
    name: String,
    version: String,
}

fn engine_version(lockfile: &str) -> Result<String, Box<dyn Error>> {
    let lockfile: Lockfile = toml::from_str(lockfile)?;
    let version = |name: &str| -> Result<&str, Box<dyn Error>> {
        let mut packages = lockfile
            .package
            .iter()
            .filter(|package| package.name == name);
        let package = packages
            .next()
            .ok_or_else(|| format!("{name} is missing from Cargo.lock"))?;
        if packages.next().is_some() {
            return Err(format!("{name} has multiple versions in Cargo.lock").into());
        }
        Ok(&package.version)
    };
    let linter = version("shuck-linter")?;
    let parser = version("shuck-parser")?;
    if linter != parser {
        return Err(format!("Shuck versions differ: linter {linter}, parser {parser}").into());
    }
    Ok(linter.to_owned())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let [input, output] = args.as_slice() else {
        return Err("usage: package <component.wasm> <output-directory>".into());
    };
    let engine_version = engine_version(include_str!("../../Cargo.lock"))?;
    Package {
        identity: Identity {
            id: "shuck",
            name: "Shuck",
            description: env!("CARGO_PKG_DESCRIPTION"),
            license: "MIT",
            repository: env!("CARGO_PKG_REPOSITORY"),
            version: env!("CARGO_PKG_VERSION"),
        },
        engine: Engine {
            name: "shuck",
            version: &engine_version,
        },
        languages: &["shell"],
        capabilities: Capabilities {
            filesystem: false,
            network: false,
            fixes: true,
            source_annotations: false,
            source_files: false,
        },
        descriptor: &actionlint_plugin_shuck::descriptor(),
        license_text: include_str!("../../LICENSE"),
    }
    .write(Path::new(input), Path::new(output))
}

#[cfg(test)]
mod tests {
    use super::engine_version;

    #[test]
    fn reads_the_resolved_version() {
        let lockfile = "[[package]]\nname = 'shuck-linter'\nversion = '9.8.7'\n\n[[package]]\nname = 'shuck-parser'\nversion = '9.8.7'\n";
        assert_eq!(engine_version(lockfile).unwrap(), "9.8.7");
    }

    #[test]
    fn rejects_missing_mismatched_and_ambiguous_versions() {
        for packages in [
            vec![("shuck-linter", "1.0.0")],
            vec![("shuck-linter", "1.0.0"), ("shuck-parser", "2.0.0")],
            vec![
                ("shuck-linter", "1.0.0"),
                ("shuck-linter", "2.0.0"),
                ("shuck-parser", "1.0.0"),
            ],
        ] {
            let lockfile = packages
                .into_iter()
                .map(|(name, version)| {
                    format!("[[package]]\nname = '{name}'\nversion = '{version}'\n")
                })
                .collect::<String>();
            assert!(engine_version(&lockfile).is_err());
        }
    }
}
