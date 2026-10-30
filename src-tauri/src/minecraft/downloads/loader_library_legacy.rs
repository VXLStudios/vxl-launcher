#[derive(Debug, Clone, Copy)]
pub enum LoaderMavenFlavor {
    Forge,
    NeoForge,
}

/// Maven `-universal` suffix for legacy loader client jars.
pub fn legacy_universal_jar_suffix(
    group: &str,
    artifact: &str,
    flavor: LoaderMavenFlavor,
) -> &'static str {
    match flavor {
        LoaderMavenFlavor::Forge if group == "net/minecraftforge" && artifact == "forge" => {
            "-universal"
        }
        LoaderMavenFlavor::NeoForge if group == "net/neoforged" && artifact == "neoforge" => {
            "-universal"
        }
        _ => "",
    }
}
