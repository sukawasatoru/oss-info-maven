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

mod parse_dependencies_string;
mod parse_prettied_dependencies_string;

pub use parse_dependencies_string::parse_dependencies_string;
pub use parse_prettied_dependencies_string::parse_prettied_dependencies_string;

fn pretty_version(line: &str) -> String {
    let segments = line.split(':').collect::<Vec<_>>();
    let group_id = segments.first().expect("missing group id");
    let artifact_name = segments.get(1).expect("missing artifact name");

    match segments.len() {
        3 => {
            // - org.jetbrains.kotlin:kotlin-stdlib-jdk8:1.6.21
            // - org.jetbrains.kotlin:kotlin-stdlib:1.6.21 -> 1.7.10
            // - org.jetbrains.kotlin:kotlin-stdlib:1.6.21 -> 1.7.10 (*)
            // - androidx.profileinstaller:profileinstaller:1.3.0 (*)

            let version = segments.get(2).expect("missing version");
            let version_segments = version.split(' ').collect::<Vec<_>>();
            let version = match version_segments.len() {
                4 | 3 => {
                    // |0     |1 |2     |3  |
                    // `1.6.21 -> 1.7.10 (*)`
                    // `1.6.21 -> 1.7.10`
                    version_segments
                        .get(2)
                        .expect("unexpected format (v_seg.len == 3)")
                }
                2 | 1 => {
                    // |0     |1  |
                    // `1.6.21 (*)`
                    // `1.6.21`
                    version_segments
                        .first()
                        .expect("unexpected format (v_seg.len 2 or 1)")
                }
                _ => todo!("3-{}: {}", segments.len(), line),
            };

            format!("{}:{}:{}", group_id, artifact_name, version)
        }
        2 => {
            // no version by bom. e.g:
            // - `androidx.compose.ui:ui-tooling -> 1.3.3`
            // - `androidx.compose.material:material -> 1.3.1 (*)`

            // |0       |1 |2    |3  |
            // `material -> 1.3.1 (*)`
            let mut segments = artifact_name.split(' ');
            let artifact_name = segments.next().expect("missing artifact name (by bom)");
            segments.next();
            let version = segments.next().expect("missing version (by bom)");

            format!("{}:{}:{}", group_id, artifact_name, version)
        }
        _ => todo!("{}: {}", segments.len(), line),
    }
}
