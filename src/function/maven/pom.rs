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

use crate::model::SPDX;
use crate::Fallible;
use serde::Deserialize;
use url::Url;

/// https://maven.apache.org/pom.html
pub fn parse_pom(xml: &str) -> Fallible<POM> {
    let parsed = quick_xml::de::from_str::<Project>(xml)?;

    Ok(parsed.into())
}

#[derive(Debug, Eq, PartialEq)]
pub struct POM {
    pub group_id: Option<String>,
    pub artifact_id: String,
    pub version: Option<String>,
    pub packaging: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub licenses: Vec<SPDX>,
}

impl From<Project> for POM {
    fn from(value: Project) -> Self {
        Self {
            group_id: value.group_id,
            artifact_id: value.artifact_id,
            version: value.version,
            packaging: value.packaging,
            name: value.name,
            description: value.description,
            licenses: value
                .licenses
                .map(|licenses| {
                    licenses
                        .field
                        .into_iter()
                        .map(|data| data.name.parse().expect("unexpected spdx"))
                        .collect()
                })
                .unwrap_or_else(|| vec![]),
        }
    }
}

/// https://maven.apache.org/pom.html
#[derive(Deserialize, PartialEq)]
struct Project {
    #[serde(rename = "groupId")]
    group_id: Option<String>,

    #[serde(rename = "artifactId")]
    artifact_id: String,

    version: Option<String>,
    packaging: Option<String>,
    name: Option<String>,
    description: Option<String>,
    licenses: Option<Licenses>,
}

#[derive(Deserialize, PartialEq)]
struct Licenses {
    #[serde(rename = "$value")]
    field: Vec<License>,
}

#[derive(Deserialize, PartialEq)]
struct License {
    name: String,
    url: Url,
    distribution: Option<String>,
}
