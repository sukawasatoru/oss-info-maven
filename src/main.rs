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

use futures::StreamExt;
use indexmap::IndexMap;
use oss_info_maven::prelude::*;
use oss_info_maven::retrieve_maven_lib;
use std::io::prelude::*;
use std::io::BufReader;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::{info_span, Instrument};

#[tokio::main]
async fn main() -> Fallible<()> {
    tracing_subscriber::fmt::init();

    info!("hello");

    let mut dep_map = read_deps()?
        .into_iter()
        .fold(IndexMap::new(), |mut acc, data| {
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

    dbg!(dep_map);

    if has_error {
        bail!("finished but an error occurred in some requests");
    }
    info!("bye");
    Ok(())
}

fn read_deps() -> Fallible<Vec<String>> {
    let mut reader = BufReader::new(std::io::stdin());

    let mut list = vec![];
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {
                let line = line.trim();
                if !line.is_empty() {
                    list.push(line.to_owned());
                }
            }
            Err(e) => {
                debug!(?e);
                bail!("failed to read lines: {}", e);
            }
        }
    }

    Ok(list)
}
