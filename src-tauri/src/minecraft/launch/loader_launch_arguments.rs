use log::info;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct LoaderJvmGameArguments {
    pub game: Vec<String>,
    pub jvm: Vec<String>,
}

pub fn game_arguments_from(args: Option<&LoaderJvmGameArguments>) -> Vec<String> {
    args.map(|a| a.game.clone()).unwrap_or_default()
}

pub fn jvm_arguments_from(
    args: Option<&LoaderJvmGameArguments>,
    library_directory: &PathBuf,
    version_name: &str,
) -> Vec<String> {
    let Some(arg_lists) = args else {
        return Vec::new();
    };

    let classpath_separator = if cfg!(windows) { ";" } else { ":" };
    let library_dir = library_directory.to_string_lossy().replace('\\', "/");

    info!("\n=== Loader JVM Arguments Debug ===");
    info!("Library Directory: {}", library_dir);
    info!("Classpath Separator: {}", classpath_separator);
    info!("Version Name: {}", version_name);

    arg_lists
        .jvm
        .iter()
        .map(|arg| {
            info!("\nOriginal argument: {}", arg);
            let replaced = arg
                .replace("${library_directory}", &library_dir)
                .replace("${classpath_separator}", classpath_separator)
                .replace("${version_name}", version_name);
            info!("Replaced argument: {}", replaced);
            replaced
        })
        .collect()
}
