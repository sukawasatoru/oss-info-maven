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

use crate::Fallible;
use serde::Deserialize;

/// https://maven.apache.org/ref/3.9.4/maven-repository-metadata/
pub fn parse_maven_metadata(maven_metadata: &str) -> Fallible<Dependency> {
    let parsed = quick_xml::de::from_str::<Metadata>(maven_metadata)?;

    Ok(parsed.into())
}

#[derive(Debug)]
pub struct Dependency {
    pub group_id: String,
    pub artifact_id: String,
    pub version: Option<String>,
    pub latest_version: Option<String>,
    pub release_version: Option<String>,
}

impl From<Metadata> for Dependency {
    fn from(value: Metadata) -> Self {
        Self {
            group_id: value.group_id,
            artifact_id: value.artifact_id,
            version: value.version,
            latest_version: value.versioning.latest,
            release_version: value.versioning.release,
        }
    }
}

/// - https://maven.apache.org/repository/layout.html
/// - https://maven.apache.org/ref/3.9.4/maven-repository-metadata/
#[derive(Deserialize, PartialEq)]
struct Metadata {
    #[serde(rename = "groupId")]
    group_id: String,

    #[serde(rename = "artifactId")]
    artifact_id: String,

    version: Option<String>,
    versioning: Versioning,
}

#[derive(Deserialize, PartialEq)]
struct Versioning {
    latest: Option<String>,
    release: Option<String>,
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    #[test]
    #[ignore]
    fn quick_xml_playground() {
        // https://github.com/tafia/quick-xml/tree/master#serde

        #[derive(Debug, Deserialize, Serialize, Eq, PartialEq)]
        struct Layer1 {
            #[serde(rename = "@f1")]
            attr1: String,

            #[serde(rename = "@f2")]
            attr2: String,

            #[serde(rename = "Bar")]
            bar: Layer2,

            #[serde(rename = "Piyo")]
            piyo: Layer2_2,
        }

        #[derive(Debug, Deserialize, Serialize, Eq, PartialEq)]
        struct Layer2 {
            #[serde(rename = "@b1")]
            attr1: String,

            #[serde(rename = "@b2")]
            attr2: String,

            #[serde(rename = "$text")]
            field: String,
        }

        #[derive(Debug, Deserialize, Serialize, Eq, PartialEq)]
        struct Layer2_2 {
            #[serde(rename = "$text")]
            field: String,
        }

        let source = r#"
<Foo f1="val 1" f2="val 2">
  <Bar b1="val 3" b2="val 4">
    baz
  </Bar>
  <Piyo>
    hoge
  </Piyo>
</Foo>
"#;

        let expected = Layer1 {
            attr1: "val 1".into(),
            attr2: "val 2".into(),
            bar: Layer2 {
                attr1: "val 3".into(),
                attr2: "val 4".into(),
                field: "baz".into(),
            },
            piyo: Layer2_2 {
                field: "hoge".into(),
            },
        };

        let actual = quick_xml::de::from_str::<Layer1>(source).unwrap();

        assert_eq!(expected, actual);
    }
}
