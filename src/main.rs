/*
 * Copyright 2023 sukawasatoru
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use clap::{CommandFactory, Parser, ValueEnum};
use futures::StreamExt;
use indexmap::IndexMap;
use oss_info_maven::function::gradle::{
    parse_dependencies_string, parse_prettied_dependencies_string,
};
use oss_info_maven::model::SPDX;
use oss_info_maven::prelude::*;
use oss_info_maven::retrieve_maven_lib;
use std::io::BufReader;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::{info_span, Instrument};

/// Collect OSS information from server.
#[derive(Parser)]
struct Opt {
    /// Output format type.
    #[clap(long, default_value = "csv")]
    format: FormatType,

    /// Parse stdin as manually formatted Gradle output.
    #[clap(long)]
    skip_pretty: bool,

    /// Generate shell completions.
    #[arg(long, exclusive = true)]
    completion: Option<clap_complete::Shell>,
}

#[derive(Clone, ValueEnum)]
enum FormatType {
    Csv,
}

#[tokio::main]
async fn main() -> Fallible<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let opt = Opt::parse();

    if let Some(shell) = opt.completion {
        clap_complete::generate(
            shell,
            &mut Opt::command(),
            env!("CARGO_PKG_NAME"),
            &mut std::io::stdout(),
        );
        return Ok(());
    }

    info!("hello");

    let lines = if opt.skip_pretty {
        parse_prettied_dependencies_string(BufReader::new(std::io::stdin()))?
    } else {
        let mut reader = BufReader::new(std::io::stdin());
        parse_dependencies_string(&mut reader)?
    };

    let mut dep_map = lines.into_iter().fold(IndexMap::new(), |mut acc, data| {
        acc.insert(data, None);
        acc
    });

    let client = reqwest::Client::builder().build().expect("Client::new()");
    let semaphore = Arc::new(Semaphore::new(8));
    let mut futs = futures::stream::FuturesUnordered::new();
    for dep_name in dep_map.keys() {
        let client = client.clone();
        let semaphore = semaphore.clone();
        let dep_name = dep_name.to_string();
        let span = info_span!("retrieve_task", %dep_name);
        futs.push(tokio::task::spawn(
            async move {
                let _permit = semaphore.acquire().await.unwrap();
                let ret = retrieve_maven_lib(client, &dep_name).await;
                (dep_name, ret)
            }
            .instrument(span),
        ));
    }

    let mut has_error = false;
    while let Some(data) = futs.next().await {
        let (name, pom) = match data {
            Ok((name, Ok(pom))) => (name, pom),
            Ok((name, Err(e))) => {
                warn!(%name, ?e, "failed to request artifact info.");
                has_error = true;
                continue;
            }
            Err(e) => {
                error!(?e, "a request was aborted");
                bail!("a request was aborted");
            }
        };
        dep_map[&name] = Some(pom);
    }

    match opt.format {
        FormatType::Csv => {
            let mut writer = csv::WriterBuilder::new().from_writer(std::io::stdout());
            writer.write_record([
                "Dependency",
                "Version (Input)",
                "Version (Latest)",
                "Packaging",
                "Name",
                "Description",
                "Licenses",
            ])?;
            for (dep_name, pom) in dep_map {
                let dep_name_segments = dep_name.split(':').collect::<Vec<_>>();
                let input_version = match dep_name_segments.get(2) {
                    Some(data) => data.to_string(),
                    None => "".into(),
                };
                let pom = match pom {
                    Some(pom) => pom,
                    None => {
                        info!(%dep_name, "skip");
                        continue;
                    }
                };

                writer.write_record(&[
                    format!(
                        "{}:{}",
                        dep_name_segments
                            .first()
                            .expect("unexpected format: group id"),
                        dep_name_segments
                            .get(1)
                            .expect("unexpected format: artifact name"),
                    ),
                    input_version,
                    pom.version.unwrap_or_else(|| "".into()),
                    pom.packaging.unwrap_or_else(|| "".into()),
                    pom.name.unwrap_or_else(|| "".into()),
                    pom.description.unwrap_or_else(|| "".into()),
                    pom.licenses
                        .iter()
                        .map(SPDX::to_string)
                        .collect::<Vec<_>>()
                        .join("/"),
                ])?;
            }

            writer.flush()?;
        }
    }

    if has_error {
        bail!("finished but an error occurred in some requests");
    }
    info!("bye");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn struct_opt() {
        Opt::command().debug_assert();
    }

    #[test]
    #[ignore]
    fn struct_opt_help() {
        Opt::command().print_help().unwrap();
    }
}
