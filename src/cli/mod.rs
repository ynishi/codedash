//! CLI presentation layer using senl SenlApp.

use std::sync::Arc;

use senl::SenlApp;

use crate::app::analyze::AnalyzePipeline;
use crate::infra::git::GitEnricher;
use crate::infra::lua::{modules::CODEDASH_FILES, rustlib};
use crate::infra::parser::registry::ParserRegistry;
use crate::Error;

const APP_LUA: &str = include_str!("../lua/app.lua");

/// Run the codedash CLI.
pub fn run() -> Result<i32, Error> {
    let repo_path = std::env::current_dir()?;
    let registry = ParserRegistry::new();
    let enricher = Box::new(GitEnricher::new());
    let pipeline = Arc::new(AnalyzePipeline::new(registry, enricher, repo_path));

    let exit_code = SenlApp::from_source("codedash", APP_LUA)
        .with_preload_dir("codedash", CODEDASH_FILES)
        .with_setup({
            let pipeline = Arc::clone(&pipeline);
            move |lua| {
                rustlib::inject_rustlib(lua, pipeline)
                    .map_err(|e| senl::SenlError::App(e.to_string()))
            }
        })
        .run()?;

    Ok(exit_code)
}
