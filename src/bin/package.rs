use actionlint_extension_api::package::Capabilities;
use actionlint_extension_api::package::Engine;
use actionlint_extension_api::package::Identity;
use actionlint_extension_api::package::Package;
use std::error::Error;
use std::path::Path;
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let [input, output] = args.as_slice() else {
        return Err("usage: package <component.wasm> <output-directory>".into());
    };
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
            version: "0.2.8",
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
