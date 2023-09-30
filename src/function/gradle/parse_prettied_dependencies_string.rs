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

use crate::function::gradle::pretty_version;
use crate::prelude::*;
use std::collections::HashSet;
use std::io::prelude::*;

pub fn parse_prettied_dependencies_string<R>(mut reader: R) -> Fallible<Vec<String>>
where
    R: BufRead,
{
    let mut list = HashSet::new();
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let line = if line.split(':').collect::<Vec<_>>().len() == 3 {
                    pretty_version(line)
                } else {
                    line.to_owned()
                };

                list.insert(line);
            }
            Err(e) => {
                debug!(?e);
                bail!("failed to read lines: {}", e);
            }
        }
    }

    let mut list = Vec::from_iter(list);
    list.sort();

    Ok(list)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_prettied_dependencies_string_without_version() {
        let lines = r#"
androidx.activity:activity
androidx.activity:activity-compose
androidx.activity:activity-ktx
androidx.annotation:annotation
androidx.annotation:annotation-experimental
androidx.appcompat:appcompat
androidx.appcompat:appcompat-resources
"#;
        let actual = parse_prettied_dependencies_string(&mut lines.as_bytes()).unwrap();
        let expected = vec![
            "androidx.activity:activity".to_owned(),
            "androidx.activity:activity-compose".into(),
            "androidx.activity:activity-ktx".into(),
            "androidx.annotation:annotation".into(),
            "androidx.annotation:annotation-experimental".into(),
            "androidx.appcompat:appcompat".into(),
            "androidx.appcompat:appcompat-resources".into(),
        ];

        assert_eq!(actual, expected);
    }

    #[test]
    fn parse_prettied_dependencies_string_with_version() {
        let lines = r#"
androidx.activity:activity-compose:1.3.0 -> 1.4.0 (*)
androidx.activity:activity-compose:1.3.1 -> 1.4.0 (*)
androidx.activity:activity-compose:1.4.0
androidx.activity:activity-ktx:1.2.3 -> 1.4.0 (*)
androidx.activity:activity-ktx:1.4.0
androidx.activity:activity:1.2.4 -> 1.4.0 (*)
androidx.activity:activity:1.3.1 -> 1.4.0
androidx.activity:activity:1.4.0 (*)
androidx.annotation:annotation-experimental:1.0.0 -> 1.1.0
androidx.annotation:annotation-experimental:1.1.0
androidx.annotation:annotation-experimental:1.1.0-rc01 -> 1.1.0
androidx.annotation:annotation:1.0.0 -> 1.3.0
androidx.annotation:annotation:1.0.1 -> 1.3.0
androidx.annotation:annotation:1.1.0 -> 1.3.0
androidx.annotation:annotation:1.2.0 -> 1.3.0
androidx.annotation:annotation:1.3.0
androidx.appcompat:appcompat-resources:1.2.0
androidx.appcompat:appcompat:1.1.0 -> 1.2.0 (*)
androidx.appcompat:appcompat:1.2.0
"#;
        let actual = parse_prettied_dependencies_string(&mut lines.as_bytes()).unwrap();
        let expected = vec![
            "androidx.activity:activity-compose:1.4.0".to_owned(),
            "androidx.activity:activity-ktx:1.4.0".into(),
            "androidx.activity:activity:1.4.0".into(),
            "androidx.annotation:annotation-experimental:1.1.0".into(),
            "androidx.annotation:annotation:1.3.0".into(),
            "androidx.appcompat:appcompat-resources:1.2.0".into(),
            "androidx.appcompat:appcompat:1.2.0".into(),
        ];

        assert_eq!(actual, expected);
    }
}
