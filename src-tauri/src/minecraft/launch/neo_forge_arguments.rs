use crate::minecraft::dto::neo_forge_meta::NeoForgeVersion;
use crate::minecraft::launch::loader_launch_arguments::{
    game_arguments_from, jvm_arguments_from, LoaderJvmGameArguments,
};
use std::path::PathBuf;

pub struct NeoForgeArguments;

impl NeoForgeArguments {
    pub fn get_game_arguments(forge_version: &NeoForgeVersion) -> Vec<String> {
        let loader_args = forge_version.arguments.as_ref().map(|args| LoaderJvmGameArguments {
            game: args.game.clone(),
            jvm: args.jvm.clone(),
        });
        game_arguments_from(loader_args.as_ref())
    }

    pub fn get_jvm_arguments(
        forge_version: &NeoForgeVersion,
        library_directory: &PathBuf,
        version_name: &str,
    ) -> Vec<String> {
        let loader_args = forge_version.arguments.as_ref().map(|args| LoaderJvmGameArguments {
            game: args.game.clone(),
            jvm: args.jvm.clone(),
        });
        jvm_arguments_from(loader_args.as_ref(), library_directory, version_name)
    }
}
