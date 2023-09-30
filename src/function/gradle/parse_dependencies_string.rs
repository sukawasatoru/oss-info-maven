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

use crate::prelude::*;
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashSet;
use tracing::debug_span;

/// https://docs.gradle.org/current/userguide/viewing_debugging_dependencies.html
pub fn parse_dependencies_string(gradle_output: &str) -> Fallible<Vec<String>> {
    let mut list = HashSet::new();
    let mut found_start = false;
    let mut end = false;
    let mut current_level = 0usize;
    for line in gradle_output.lines() {
        let line_span = debug_span!("", %line);
        let _enter = line_span.enter();

        let line_level = calculate_level(line)?;
        debug!(?line_level);

        if !found_start || end {
            match line_level {
                Some(0) => {
                    ensure!(
                        !end,
                        "Please specify `--configuration` option. e.g: `--configuration releaseRuntimeClasspath`",
                    );
                    found_start = true;
                }
                Some(_) => bail!("unexpected indent"),
                _ => continue,
            }
        }

        let line_level = match line_level {
            Some(data) => data,
            None => {
                end = true;
                continue;
            }
        };

        if line.contains("--- project ") {
            // \--- project :hoge
            //      \--- xxx:yyy:zzz
            current_level = line_level + 1;
            continue;
        }

        if current_level < line_level {
            // \--- xxx:yyy:zzz
            //      \--- xxx:yyy:zzz
            continue;
        }

        // update level for leave project. e.g.:
        // +--- project :hoge
        // |    \--- xxx:yyy:zzz
        // \--- xxx:yyy:zzz
        current_level = line_level;

        list.insert(pretty_name(line).context("unexpected format")?);
    }

    let mut list = Vec::from_iter(list);
    list.sort();

    Ok(list)
}

fn calculate_level(line: &str) -> Fallible<Option<usize>> {
    line.find("--- ")
        .map(|data| {
            // -1 for `+--- ` or `\--- `.
            let data = data - 1;
            ensure!(data % 5 == 0, "unexpected indent: {}", data);
            Ok(data / 5)
        })
        .map_or(Ok(None), |v| v.map(Some))
}

fn pretty_name(line: &str) -> Option<String> {
    static REG: Lazy<Regex> = Lazy::new(|| Regex::new(r"[+\\]--- (.*)$").expect("invalid pattern"));

    REG.captures(line)
        .and_then(|data| data.get(1).map(|data| data.as_str().to_owned()))
        .map(|data| {
            let segments = data.split(':').collect::<Vec<_>>();
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
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pretty_name_annotation() {
        let actual =
            pretty_name(r#"|    |    |    +--- androidx.annotation:annotation:1.2.0 -> 1.5.0 (*)"#)
                .unwrap();
        assert_eq!(actual, "androidx.annotation:annotation:1.5.0");
    }

    #[test]
    fn pretty_name_glide() {
        let actual =
            pretty_name(r#"|         \--- com.github.bumptech.glide:glide:4.15.1"#).unwrap();
        assert_eq!(actual, "com.github.bumptech.glide:glide:4.15.1");
    }

    #[test]
    fn pretty_name_ui_tooling() {
        let actual = pretty_name("+--- androidx.compose.ui:ui-tooling -> 1.3.3").unwrap();
        assert_eq!(actual, "androidx.compose.ui:ui-tooling:1.3.3");
    }

    #[test]
    fn parse_dependencies_string_app_release_runtime_classpath() {
        let gradle_output = r#"
Starting a Gradle Daemon (subsequent builds will be faster)
Type-safe project accessors is an incubating feature.
Project accessors enabled, but root project name not explicitly set for 'android-template'. Checking out the project in different folders will impact the generated code and implicitly the buildscript classpath, breaking caching.

> Task :app:dependencies

------------------------------------------------------------
Project ':app'
------------------------------------------------------------

releaseRuntimeClasspath - Runtime classpath of compilation 'release' (target  (androidJvm)).
+--- org.jetbrains.kotlin:kotlin-stdlib-jdk8:1.6.21
|    +--- org.jetbrains.kotlin:kotlin-stdlib:1.6.21 -> 1.7.10
|    |    +--- org.jetbrains.kotlin:kotlin-stdlib-common:1.7.10
|    |    \--- org.jetbrains:annotations:13.0
|    \--- org.jetbrains.kotlin:kotlin-stdlib-jdk7:1.6.21
|         \--- org.jetbrains.kotlin:kotlin-stdlib:1.6.21 -> 1.7.10 (*)
+--- project :lib
|    +--- org.jetbrains.kotlin:kotlin-stdlib-jdk8:1.6.21 (*)
|    +--- androidx.core:core-ktx:1.9.0
|    |    +--- androidx.annotation:annotation:1.1.0 -> 1.5.0
|    |    |    \--- org.jetbrains.kotlin:kotlin-stdlib:1.7.10 (*)
|    |    +--- androidx.core:core:1.9.0
|    |    |    +--- androidx.annotation:annotation:1.2.0 -> 1.5.0 (*)
|    |    |    +--- androidx.annotation:annotation-experimental:1.3.0
|    |    |    |    \--- org.jetbrains.kotlin:kotlin-stdlib:1.7.10 (*)
|    |    |    +--- androidx.collection:collection:1.0.0 -> 1.1.0
|    |    |    |    \--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|    |    |    +--- androidx.concurrent:concurrent-futures:1.0.0 -> 1.1.0
|    |    |    |    +--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|    |    |    |    \--- com.google.guava:listenablefuture:1.0
|    |    |    +--- androidx.lifecycle:lifecycle-runtime:2.3.1 -> 2.5.1
|    |    |    |    +--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|    |    |    |    +--- androidx.arch.core:core-common:2.1.0
|    |    |    |    |    \--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|    |    |    |    +--- androidx.arch.core:core-runtime:2.1.0
|    |    |    |    |    +--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|    |    |    |    |    \--- androidx.arch.core:core-common:2.1.0 (*)
|    |    |    |    \--- androidx.lifecycle:lifecycle-common:2.5.1
|    |    |    |         +--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|    |    |    |         \--- androidx.lifecycle:lifecycle-common-java8:2.5.1 (c)
|    |    |    +--- androidx.versionedparcelable:versionedparcelable:1.1.1
|    |    |    |    +--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|    |    |    |    \--- androidx.collection:collection:1.0.0 -> 1.1.0 (*)
|    |    |    \--- androidx.core:core-ktx:1.9.0 (c)
|    |    +--- org.jetbrains.kotlin:kotlin-stdlib:1.7.10 (*)
|    |    \--- androidx.core:core:1.9.0 (c)
|    \--- project :liblib
|         +--- org.jetbrains.kotlin:kotlin-stdlib-jdk8:1.6.21 (*)
|         +--- com.squareup.okhttp3:okhttp:4.9.3
|         |    +--- com.squareup.okio:okio:2.8.0
|         |    |    +--- org.jetbrains.kotlin:kotlin-stdlib:1.4.0 -> 1.7.10 (*)
|         |    |    \--- org.jetbrains.kotlin:kotlin-stdlib-common:1.4.0 -> 1.7.10
|         |    \--- org.jetbrains.kotlin:kotlin-stdlib:1.4.10 -> 1.7.10 (*)
|         \--- com.github.bumptech.glide:glide:4.15.1
|              +--- com.github.bumptech.glide:gifdecoder:4.15.1
|              |    \--- androidx.annotation:annotation:1.3.0 -> 1.5.0 (*)
|              +--- com.github.bumptech.glide:disklrucache:4.15.1
|              +--- com.github.bumptech.glide:annotations:4.15.1
|              +--- androidx.fragment:fragment:1.3.6
|              |    +--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|              |    +--- androidx.core:core:1.2.0 -> 1.9.0 (*)
|              |    +--- androidx.collection:collection:1.1.0 (*)
|              |    +--- androidx.viewpager:viewpager:1.0.0
|              |    |    +--- androidx.annotation:annotation:1.0.0 -> 1.5.0 (*)
|              |    |    +--- androidx.core:core:1.0.0 -> 1.9.0 (*)
|              |    |    \--- androidx.customview:customview:1.0.0
|              |    |         +--- androidx.annotation:annotation:1.0.0 -> 1.5.0 (*)
|              |    |         \--- androidx.core:core:1.0.0 -> 1.9.0 (*)
|              |    +--- androidx.loader:loader:1.0.0
|              |    |    +--- androidx.annotation:annotation:1.0.0 -> 1.5.0 (*)
|              |    |    +--- androidx.core:core:1.0.0 -> 1.9.0 (*)
|              |    |    +--- androidx.lifecycle:lifecycle-livedata:2.0.0
|              |    |    |    +--- androidx.arch.core:core-runtime:2.0.0 -> 2.1.0 (*)
|              |    |    |    +--- androidx.lifecycle:lifecycle-livedata-core:2.0.0 -> 2.5.1
|              |    |    |    |    +--- androidx.arch.core:core-common:2.1.0 (*)
|              |    |    |    |    +--- androidx.arch.core:core-runtime:2.1.0 (*)
|              |    |    |    |    \--- androidx.lifecycle:lifecycle-common:2.5.1 (*)
|              |    |    |    \--- androidx.arch.core:core-common:2.0.0 -> 2.1.0 (*)
|              |    |    \--- androidx.lifecycle:lifecycle-viewmodel:2.0.0 -> 2.5.1
|              |    |         +--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|              |    |         +--- org.jetbrains.kotlin:kotlin-stdlib:1.6.21 -> 1.7.10 (*)
|              |    |         \--- androidx.lifecycle:lifecycle-viewmodel-savedstate:2.5.1 (c)
|              |    +--- androidx.activity:activity:1.2.4 -> 1.6.1
|              |    |    +--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|              |    |    +--- androidx.collection:collection:1.0.0 -> 1.1.0 (*)
|              |    |    +--- androidx.core:core:1.8.0 -> 1.9.0 (*)
|              |    |    +--- androidx.lifecycle:lifecycle-runtime:2.5.1 (*)
|              |    |    +--- androidx.lifecycle:lifecycle-viewmodel:2.5.1 (*)
|              |    |    +--- androidx.lifecycle:lifecycle-viewmodel-savedstate:2.5.1
|              |    |    |    +--- androidx.annotation:annotation:1.0.0 -> 1.5.0 (*)
|              |    |    |    +--- androidx.core:core-ktx:1.2.0 -> 1.9.0 (*)
|              |    |    |    +--- androidx.lifecycle:lifecycle-livedata-core:2.5.1 (*)
|              |    |    |    +--- androidx.lifecycle:lifecycle-viewmodel:2.5.1 (*)
|              |    |    |    +--- androidx.savedstate:savedstate:1.2.0
|              |    |    |    |    +--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|              |    |    |    |    +--- androidx.arch.core:core-common:2.1.0 (*)
|              |    |    |    |    +--- androidx.lifecycle:lifecycle-common:2.4.0 -> 2.5.1 (*)
|              |    |    |    |    \--- org.jetbrains.kotlin:kotlin-stdlib:1.6.20 -> 1.7.10 (*)
|              |    |    |    +--- org.jetbrains.kotlin:kotlin-stdlib:1.6.21 -> 1.7.10 (*)
|              |    |    |    \--- org.jetbrains.kotlinx:kotlinx-coroutines-android:1.6.1 -> 1.6.4
|              |    |    |         +--- org.jetbrains.kotlinx:kotlinx-coroutines-core:1.6.4
|              |    |    |         |    \--- org.jetbrains.kotlinx:kotlinx-coroutines-core-jvm:1.6.4
|              |    |    |         |         +--- org.jetbrains.kotlinx:kotlinx-coroutines-bom:1.6.4
|              |    |    |         |         |    +--- org.jetbrains.kotlinx:kotlinx-coroutines-android:1.6.4 (c)
|              |    |    |         |         |    +--- org.jetbrains.kotlinx:kotlinx-coroutines-core-jvm:1.6.4 (c)
|              |    |    |         |         |    \--- org.jetbrains.kotlinx:kotlinx-coroutines-core:1.6.4 (c)
|              |    |    |         |         +--- org.jetbrains.kotlin:kotlin-stdlib-jdk8:1.6.21 (*)
|              |    |    |         |         \--- org.jetbrains.kotlin:kotlin-stdlib-common:1.6.21 -> 1.7.10
|              |    |    |         +--- org.jetbrains.kotlinx:kotlinx-coroutines-bom:1.6.4 (*)
|              |    |    |         \--- org.jetbrains.kotlin:kotlin-stdlib-jdk8:1.6.21 (*)
|              |    |    +--- androidx.savedstate:savedstate:1.2.0 (*)
|              |    |    +--- androidx.tracing:tracing:1.0.0
|              |    |    |    \--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|              |    |    +--- org.jetbrains.kotlin:kotlin-stdlib:1.7.10 (*)
|              |    |    \--- androidx.activity:activity-ktx:1.6.1 (c)
|              |    +--- androidx.lifecycle:lifecycle-livedata-core:2.3.1 -> 2.5.1 (*)
|              |    +--- androidx.lifecycle:lifecycle-viewmodel:2.3.1 -> 2.5.1 (*)
|              |    +--- androidx.lifecycle:lifecycle-viewmodel-savedstate:2.3.1 -> 2.5.1 (*)
|              |    +--- androidx.savedstate:savedstate:1.1.0 -> 1.2.0 (*)
|              |    \--- androidx.annotation:annotation-experimental:1.0.0 -> 1.3.0 (*)
|              +--- androidx.vectordrawable:vectordrawable-animated:1.1.0
|              |    +--- androidx.vectordrawable:vectordrawable:1.1.0
|              |    |    +--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|              |    |    +--- androidx.core:core:1.1.0 -> 1.9.0 (*)
|              |    |    \--- androidx.collection:collection:1.1.0 (*)
|              |    +--- androidx.interpolator:interpolator:1.0.0
|              |    |    \--- androidx.annotation:annotation:1.0.0 -> 1.5.0 (*)
|              |    \--- androidx.collection:collection:1.1.0 (*)
|              +--- androidx.exifinterface:exifinterface:1.3.3
|              |    \--- androidx.annotation:annotation:1.2.0 -> 1.5.0 (*)
|              \--- androidx.tracing:tracing:1.0.0 (*)
+--- androidx.activity:activity-compose:1.6.1
|    +--- androidx.activity:activity-ktx:1.6.1
|    |    +--- androidx.activity:activity:1.6.1 (*)
|    |    +--- androidx.core:core-ktx:1.1.0 -> 1.9.0 (*)
|    |    +--- androidx.lifecycle:lifecycle-runtime-ktx:2.5.1
|    |    |    +--- androidx.annotation:annotation:1.0.0 -> 1.5.0 (*)
|    |    |    +--- androidx.lifecycle:lifecycle-runtime:2.5.1 (*)
|    |    |    +--- org.jetbrains.kotlin:kotlin-stdlib:1.6.21 -> 1.7.10 (*)
|    |    |    \--- org.jetbrains.kotlinx:kotlinx-coroutines-android:1.6.1 -> 1.6.4 (*)
|    |    +--- androidx.lifecycle:lifecycle-viewmodel-ktx:2.5.1
|    |    |    +--- androidx.lifecycle:lifecycle-viewmodel:2.5.1 (*)
|    |    |    +--- org.jetbrains.kotlin:kotlin-stdlib:1.6.21 -> 1.7.10 (*)
|    |    |    \--- org.jetbrains.kotlinx:kotlinx-coroutines-android:1.6.1 -> 1.6.4 (*)
|    |    +--- androidx.savedstate:savedstate-ktx:1.2.0
|    |    |    +--- androidx.savedstate:savedstate:1.2.0 (*)
|    |    |    \--- org.jetbrains.kotlin:kotlin-stdlib:1.6.20 -> 1.7.10 (*)
|    |    +--- org.jetbrains.kotlin:kotlin-stdlib:1.7.10 (*)
|    |    \--- androidx.activity:activity:1.6.1 (c)
|    +--- androidx.compose.runtime:runtime:1.0.1 -> 1.3.3
|    |    +--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|    |    +--- org.jetbrains.kotlin:kotlin-stdlib:1.7.10 (*)
|    |    \--- org.jetbrains.kotlinx:kotlinx-coroutines-android:1.6.4 (*)
|    +--- androidx.compose.runtime:runtime-saveable:1.0.1 -> 1.3.3
|    |    +--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|    |    +--- androidx.compose.runtime:runtime:1.3.3 (*)
|    |    \--- org.jetbrains.kotlin:kotlin-stdlib:1.7.10 (*)
|    +--- androidx.compose.ui:ui:1.0.1 -> 1.3.3
|    |    +--- androidx.activity:activity-ktx:1.5.1 -> 1.6.1 (*)
|    |    +--- androidx.annotation:annotation:1.5.0 (*)
|    |    +--- androidx.autofill:autofill:1.0.0
|    |    |    \--- androidx.core:core:1.1.0 -> 1.9.0 (*)
|    |    +--- androidx.collection:collection:1.0.0 -> 1.1.0 (*)
|    |    +--- androidx.compose.runtime:runtime:1.3.3 (*)
|    |    +--- androidx.compose.runtime:runtime-saveable:1.3.3 (*)
|    |    +--- androidx.compose.ui:ui-geometry:1.3.3
|    |    |    +--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|    |    |    +--- androidx.compose.runtime:runtime:1.1.1 -> 1.3.3 (*)
|    |    |    +--- androidx.compose.ui:ui-util:1.3.3
|    |    |    |    \--- org.jetbrains.kotlin:kotlin-stdlib:1.7.10 (*)
|    |    |    \--- org.jetbrains.kotlin:kotlin-stdlib:1.7.10 (*)
|    |    +--- androidx.compose.ui:ui-graphics:1.3.3
|    |    |    +--- androidx.annotation:annotation:1.2.0 -> 1.5.0 (*)
|    |    |    +--- androidx.compose.runtime:runtime:1.1.1 -> 1.3.3 (*)
|    |    |    +--- androidx.compose.ui:ui-unit:1.3.3
|    |    |    |    +--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|    |    |    |    +--- androidx.compose.runtime:runtime:1.1.1 -> 1.3.3 (*)
|    |    |    |    +--- androidx.compose.ui:ui-geometry:1.3.3 (*)
|    |    |    |    +--- androidx.compose.ui:ui-util:1.3.3 (*)
|    |    |    |    \--- org.jetbrains.kotlin:kotlin-stdlib:1.7.10 (*)
|    |    |    +--- androidx.compose.ui:ui-util:1.3.3 (*)
|    |    |    \--- org.jetbrains.kotlin:kotlin-stdlib-common:1.7.10
|    |    +--- androidx.compose.ui:ui-text:1.3.3
|    |    |    +--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|    |    |    +--- androidx.collection:collection:1.0.0 -> 1.1.0 (*)
|    |    |    +--- androidx.compose.runtime:runtime:1.2.0 -> 1.3.3 (*)
|    |    |    +--- androidx.compose.runtime:runtime-saveable:1.2.0 -> 1.3.3 (*)
|    |    |    +--- androidx.compose.ui:ui-graphics:1.3.3 (*)
|    |    |    +--- androidx.compose.ui:ui-unit:1.3.3 (*)
|    |    |    +--- androidx.compose.ui:ui-util:1.3.3 (*)
|    |    |    +--- androidx.core:core:1.7.0 -> 1.9.0 (*)
|    |    |    +--- org.jetbrains.kotlin:kotlin-stdlib:1.7.10 (*)
|    |    |    +--- org.jetbrains.kotlin:kotlin-stdlib-common:1.7.10
|    |    |    \--- org.jetbrains.kotlinx:kotlinx-coroutines-core:1.6.4 (*)
|    |    +--- androidx.compose.ui:ui-unit:1.3.3 (*)
|    |    +--- androidx.compose.ui:ui-util:1.3.3 (*)
|    |    +--- androidx.core:core:1.5.0 -> 1.9.0 (*)
|    |    +--- androidx.customview:customview-poolingcontainer:1.0.0
|    |    |    +--- androidx.core:core-ktx:1.5.0 -> 1.9.0 (*)
|    |    |    \--- org.jetbrains.kotlin:kotlin-stdlib:1.6.21 -> 1.7.10 (*)
|    |    +--- androidx.lifecycle:lifecycle-common-java8:2.3.0 -> 2.5.1
|    |    |    +--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|    |    |    \--- androidx.lifecycle:lifecycle-common:2.5.1 (*)
|    |    +--- androidx.lifecycle:lifecycle-runtime:2.3.0 -> 2.5.1 (*)
|    |    +--- androidx.lifecycle:lifecycle-viewmodel:2.3.0 -> 2.5.1 (*)
|    |    +--- androidx.profileinstaller:profileinstaller:1.2.0 -> 1.3.0
|    |    |    +--- androidx.annotation:annotation:1.2.0 -> 1.5.0 (*)
|    |    |    +--- androidx.concurrent:concurrent-futures:1.1.0 (*)
|    |    |    +--- androidx.startup:startup-runtime:1.1.1
|    |    |    |    +--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|    |    |    |    \--- androidx.tracing:tracing:1.0.0 (*)
|    |    |    \--- com.google.guava:listenablefuture:1.0
|    |    +--- androidx.savedstate:savedstate-ktx:1.2.0 (*)
|    |    +--- org.jetbrains.kotlin:kotlin-stdlib:1.7.10 (*)
|    |    +--- org.jetbrains.kotlin:kotlin-stdlib-common:1.7.10
|    |    +--- org.jetbrains.kotlinx:kotlinx-coroutines-android:1.6.4 (*)
|    |    \--- org.jetbrains.kotlinx:kotlinx-coroutines-core:1.6.4 (*)
|    +--- androidx.lifecycle:lifecycle-common-java8:2.5.1 (*)
|    \--- org.jetbrains.kotlin:kotlin-stdlib:1.7.10 (*)
+--- androidx.compose:compose-bom:2023.01.00
|    +--- androidx.compose.material:material:1.3.1 (c)
|    +--- androidx.compose.runtime:runtime:1.3.3 (c)
|    +--- androidx.compose.runtime:runtime-saveable:1.3.3 (c)
|    +--- androidx.compose.ui:ui:1.3.3 (c)
|    +--- androidx.compose.ui:ui-tooling:1.3.3 (c)
|    +--- androidx.compose.animation:animation:1.3.3 (c)
|    +--- androidx.compose.animation:animation-core:1.3.3 (c)
|    +--- androidx.compose.foundation:foundation:1.3.1 (c)
|    +--- androidx.compose.foundation:foundation-layout:1.3.1 (c)
|    +--- androidx.compose.material:material-icons-core:1.3.1 (c)
|    +--- androidx.compose.material:material-ripple:1.3.1 (c)
|    +--- androidx.compose.ui:ui-text:1.3.3 (c)
|    +--- androidx.compose.ui:ui-util:1.3.3 (c)
|    +--- androidx.compose.ui:ui-geometry:1.3.3 (c)
|    +--- androidx.compose.ui:ui-graphics:1.3.3 (c)
|    +--- androidx.compose.ui:ui-unit:1.3.3 (c)
|    +--- androidx.compose.ui:ui-tooling-data:1.3.3 (c)
|    \--- androidx.compose.ui:ui-tooling-preview:1.3.3 (c)
+--- androidx.compose.ui:ui-tooling -> 1.3.3
|    +--- androidx.activity:activity-compose:1.3.0 -> 1.6.1 (*)
|    +--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|    +--- androidx.compose.animation:animation:1.3.3
|    |    +--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|    |    +--- androidx.compose.animation:animation-core:1.3.3
|    |    |    +--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|    |    |    +--- androidx.compose.runtime:runtime:1.1.1 -> 1.3.3 (*)
|    |    |    +--- androidx.compose.ui:ui:1.2.0 -> 1.3.3 (*)
|    |    |    +--- androidx.compose.ui:ui-unit:1.0.0 -> 1.3.3 (*)
|    |    |    +--- androidx.compose.ui:ui-util:1.0.0 -> 1.3.3 (*)
|    |    |    +--- org.jetbrains.kotlin:kotlin-stdlib:1.7.10 (*)
|    |    |    \--- org.jetbrains.kotlinx:kotlinx-coroutines-core:1.6.4 (*)
|    |    +--- androidx.compose.foundation:foundation-layout:1.0.0 -> 1.3.1
|    |    |    +--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|    |    |    +--- androidx.compose.animation:animation-core:1.1.1 -> 1.3.3 (*)
|    |    |    +--- androidx.compose.runtime:runtime:1.2.0 -> 1.3.3 (*)
|    |    |    +--- androidx.compose.ui:ui:1.2.0 -> 1.3.3 (*)
|    |    |    +--- androidx.compose.ui:ui-unit:1.1.1 -> 1.3.3 (*)
|    |    |    +--- androidx.compose.ui:ui-util:1.0.0 -> 1.3.3 (*)
|    |    |    +--- androidx.core:core:1.7.0 -> 1.9.0 (*)
|    |    |    \--- org.jetbrains.kotlin:kotlin-stdlib-common:1.7.10
|    |    +--- androidx.compose.runtime:runtime:1.1.1 -> 1.3.3 (*)
|    |    +--- androidx.compose.ui:ui:1.0.0 -> 1.3.3 (*)
|    |    +--- androidx.compose.ui:ui-geometry:1.0.0 -> 1.3.3 (*)
|    |    +--- androidx.compose.ui:ui-util:1.0.0 -> 1.3.3 (*)
|    |    \--- org.jetbrains.kotlin:kotlin-stdlib-common:1.7.10
|    +--- androidx.compose.material:material:1.0.0 -> 1.3.1
|    |    +--- androidx.compose.animation:animation:1.0.0 -> 1.3.3 (*)
|    |    +--- androidx.compose.animation:animation-core:1.0.0 -> 1.3.3 (*)
|    |    +--- androidx.compose.foundation:foundation:1.2.0 -> 1.3.1
|    |    |    +--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|    |    |    +--- androidx.compose.animation:animation:1.1.1 -> 1.3.3 (*)
|    |    |    +--- androidx.compose.foundation:foundation-layout:1.3.1 (*)
|    |    |    +--- androidx.compose.runtime:runtime:1.3.1 -> 1.3.3 (*)
|    |    |    +--- androidx.compose.ui:ui:1.3.1 -> 1.3.3 (*)
|    |    |    +--- androidx.compose.ui:ui-graphics:1.1.1 -> 1.3.3 (*)
|    |    |    +--- androidx.compose.ui:ui-text:1.0.0 -> 1.3.3 (*)
|    |    |    +--- androidx.compose.ui:ui-util:1.0.0 -> 1.3.3 (*)
|    |    |    \--- org.jetbrains.kotlin:kotlin-stdlib-common:1.7.10
|    |    +--- androidx.compose.foundation:foundation-layout:1.1.1 -> 1.3.1 (*)
|    |    +--- androidx.compose.material:material-icons-core:1.3.1
|    |    |    +--- androidx.compose.runtime:runtime:1.1.1 -> 1.3.3 (*)
|    |    |    +--- androidx.compose.ui:ui:1.0.0 -> 1.3.3 (*)
|    |    |    \--- org.jetbrains.kotlin:kotlin-stdlib:1.7.10 (*)
|    |    +--- androidx.compose.material:material-ripple:1.3.1
|    |    |    +--- androidx.compose.animation:animation:1.0.0 -> 1.3.3 (*)
|    |    |    +--- androidx.compose.foundation:foundation:1.1.1 -> 1.3.1 (*)
|    |    |    +--- androidx.compose.runtime:runtime:1.1.1 -> 1.3.3 (*)
|    |    |    +--- androidx.compose.ui:ui-util:1.0.0 -> 1.3.3 (*)
|    |    |    \--- org.jetbrains.kotlin:kotlin-stdlib-common:1.7.10
|    |    +--- androidx.compose.runtime:runtime:1.2.0 -> 1.3.3 (*)
|    |    +--- androidx.compose.ui:ui:1.2.0 -> 1.3.3 (*)
|    |    +--- androidx.compose.ui:ui-text:1.2.0 -> 1.3.3 (*)
|    |    +--- androidx.compose.ui:ui-util:1.0.0 -> 1.3.3 (*)
|    |    +--- androidx.lifecycle:lifecycle-runtime:2.3.0 -> 2.5.1 (*)
|    |    +--- androidx.lifecycle:lifecycle-viewmodel:2.3.0 -> 2.5.1 (*)
|    |    +--- androidx.savedstate:savedstate:1.1.0 -> 1.2.0 (*)
|    |    \--- org.jetbrains.kotlin:kotlin-stdlib-common:1.7.10
|    +--- androidx.compose.runtime:runtime:1.1.1 -> 1.3.3 (*)
|    +--- androidx.compose.ui:ui:1.3.3 (*)
|    +--- androidx.compose.ui:ui-tooling-data:1.3.3
|    |    +--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|    |    +--- androidx.compose.runtime:runtime:1.2.0 -> 1.3.3 (*)
|    |    +--- androidx.compose.ui:ui:1.3.3 (*)
|    |    \--- org.jetbrains.kotlin:kotlin-stdlib:1.7.10 (*)
|    +--- androidx.compose.ui:ui-tooling-preview:1.3.3
|    |    +--- androidx.annotation:annotation:1.2.0 -> 1.5.0 (*)
|    |    +--- androidx.compose.runtime:runtime:1.1.1 -> 1.3.3 (*)
|    |    \--- org.jetbrains.kotlin:kotlin-stdlib:1.7.10 (*)
|    +--- androidx.savedstate:savedstate-ktx:1.2.0 (*)
|    \--- org.jetbrains.kotlin:kotlin-stdlib:1.7.10 (*)
+--- androidx.compose.material:material -> 1.3.1 (*)
\--- androidx.profileinstaller:profileinstaller:1.3.0 (*)

(c) - dependency constraint
(*) - dependencies omitted (listed previously)

A web-based, searchable dependency report is available by adding the --scan option.

Deprecated Gradle features were used in this build, making it incompatible with Gradle 8.0.

You can use '--warning-mode all' to show the individual deprecation warnings and determine if they come from your own scripts or plugins.

See https://docs.gradle.org/7.6/userguide/command_line_interface.html#sec:command_line_warnings

BUILD SUCCESSFUL in 4s
1 actionable task: 1 executed
"#;

        // tracing_subscriber::fmt()
        //     .with_max_level(tracing::Level::TRACE)
        //     .with_test_writer()
        //     .without_time()
        //     .init();
        let actual = parse_dependencies_string(gradle_output).unwrap();
        let expected = vec![
            "androidx.activity:activity-compose:1.6.1".to_owned(),
            "androidx.compose.material:material:1.3.1".into(),
            "androidx.compose.ui:ui-tooling:1.3.3".into(),
            "androidx.compose:compose-bom:2023.01.00".into(),
            "androidx.core:core-ktx:1.9.0".into(),
            "androidx.profileinstaller:profileinstaller:1.3.0".into(),
            "com.github.bumptech.glide:glide:4.15.1".into(),
            "com.squareup.okhttp3:okhttp:4.9.3".into(),
            "org.jetbrains.kotlin:kotlin-stdlib-jdk8:1.6.21".into(),
        ];

        assert_eq!(actual, expected);
    }

    #[test]
    fn parse_dependencies_string_app2_release_runtime_classpath() {
        let gradle_output = r#"
Type-safe project accessors is an incubating feature.
Project accessors enabled, but root project name not explicitly set for 'android-template'. Checking out the project in different folders will impact the generated code and implicitly the buildscript classpath, breaking caching.

> Task :app2:dependencies

------------------------------------------------------------
Project ':app2'
------------------------------------------------------------

releaseRuntimeClasspath - Runtime classpath of compilation 'release' (target  (androidJvm)).
+--- org.jetbrains.kotlin:kotlin-stdlib-jdk8:1.6.21
|    +--- org.jetbrains.kotlin:kotlin-stdlib:1.6.21 -> 1.7.10
|    |    +--- org.jetbrains.kotlin:kotlin-stdlib-common:1.7.10
|    |    \--- org.jetbrains:annotations:13.0
|    \--- org.jetbrains.kotlin:kotlin-stdlib-jdk7:1.6.21
|         \--- org.jetbrains.kotlin:kotlin-stdlib:1.6.21 -> 1.7.10 (*)
\--- project :lib
     +--- org.jetbrains.kotlin:kotlin-stdlib-jdk8:1.6.21 (*)
     +--- androidx.core:core-ktx:1.9.0
     |    +--- androidx.annotation:annotation:1.1.0 -> 1.3.0
     |    +--- androidx.core:core:1.9.0
     |    |    +--- androidx.annotation:annotation:1.2.0 -> 1.3.0
     |    |    +--- androidx.annotation:annotation-experimental:1.3.0
     |    |    |    \--- org.jetbrains.kotlin:kotlin-stdlib:1.7.10 (*)
     |    |    +--- androidx.collection:collection:1.0.0 -> 1.1.0
     |    |    |    \--- androidx.annotation:annotation:1.1.0 -> 1.3.0
     |    |    +--- androidx.concurrent:concurrent-futures:1.0.0
     |    |    |    +--- com.google.guava:listenablefuture:1.0
     |    |    |    \--- androidx.annotation:annotation:1.1.0 -> 1.3.0
     |    |    +--- androidx.lifecycle:lifecycle-runtime:2.3.1
     |    |    |    +--- androidx.arch.core:core-runtime:2.1.0
     |    |    |    |    +--- androidx.annotation:annotation:1.1.0 -> 1.3.0
     |    |    |    |    \--- androidx.arch.core:core-common:2.1.0
     |    |    |    |         \--- androidx.annotation:annotation:1.1.0 -> 1.3.0
     |    |    |    +--- androidx.lifecycle:lifecycle-common:2.3.1
     |    |    |    |    \--- androidx.annotation:annotation:1.1.0 -> 1.3.0
     |    |    |    +--- androidx.arch.core:core-common:2.1.0 (*)
     |    |    |    \--- androidx.annotation:annotation:1.1.0 -> 1.3.0
     |    |    +--- androidx.versionedparcelable:versionedparcelable:1.1.1
     |    |    |    +--- androidx.annotation:annotation:1.1.0 -> 1.3.0
     |    |    |    \--- androidx.collection:collection:1.0.0 -> 1.1.0 (*)
     |    |    \--- androidx.core:core-ktx:1.9.0 (c)
     |    +--- org.jetbrains.kotlin:kotlin-stdlib:1.7.10 (*)
     |    \--- androidx.core:core:1.9.0 (c)
     \--- project :liblib
          +--- org.jetbrains.kotlin:kotlin-stdlib-jdk8:1.6.21 (*)
          +--- com.squareup.okhttp3:okhttp:4.9.3
          |    +--- com.squareup.okio:okio:2.8.0
          |    |    +--- org.jetbrains.kotlin:kotlin-stdlib:1.4.0 -> 1.7.10 (*)
          |    |    \--- org.jetbrains.kotlin:kotlin-stdlib-common:1.4.0 -> 1.7.10
          |    \--- org.jetbrains.kotlin:kotlin-stdlib:1.4.10 -> 1.7.10 (*)
          \--- com.github.bumptech.glide:glide:4.15.1
               +--- com.github.bumptech.glide:gifdecoder:4.15.1
               |    \--- androidx.annotation:annotation:1.3.0
               +--- com.github.bumptech.glide:disklrucache:4.15.1
               +--- com.github.bumptech.glide:annotations:4.15.1
               +--- androidx.fragment:fragment:1.3.6
               |    +--- androidx.annotation:annotation:1.1.0 -> 1.3.0
               |    +--- androidx.core:core:1.2.0 -> 1.9.0 (*)
               |    +--- androidx.collection:collection:1.1.0 (*)
               |    +--- androidx.viewpager:viewpager:1.0.0
               |    |    +--- androidx.annotation:annotation:1.0.0 -> 1.3.0
               |    |    +--- androidx.core:core:1.0.0 -> 1.9.0 (*)
               |    |    \--- androidx.customview:customview:1.0.0
               |    |         +--- androidx.annotation:annotation:1.0.0 -> 1.3.0
               |    |         \--- androidx.core:core:1.0.0 -> 1.9.0 (*)
               |    +--- androidx.loader:loader:1.0.0
               |    |    +--- androidx.annotation:annotation:1.0.0 -> 1.3.0
               |    |    +--- androidx.core:core:1.0.0 -> 1.9.0 (*)
               |    |    +--- androidx.lifecycle:lifecycle-livedata:2.0.0
               |    |    |    +--- androidx.arch.core:core-runtime:2.0.0 -> 2.1.0 (*)
               |    |    |    +--- androidx.lifecycle:lifecycle-livedata-core:2.0.0 -> 2.3.1
               |    |    |    |    +--- androidx.arch.core:core-common:2.1.0 (*)
               |    |    |    |    +--- androidx.arch.core:core-runtime:2.1.0 (*)
               |    |    |    |    \--- androidx.lifecycle:lifecycle-common:2.3.1 (*)
               |    |    |    \--- androidx.arch.core:core-common:2.0.0 -> 2.1.0 (*)
               |    |    \--- androidx.lifecycle:lifecycle-viewmodel:2.0.0 -> 2.3.1
               |    |         \--- androidx.annotation:annotation:1.1.0 -> 1.3.0
               |    +--- androidx.activity:activity:1.2.4
               |    |    +--- androidx.annotation:annotation:1.1.0 -> 1.3.0
               |    |    +--- androidx.core:core:1.1.0 -> 1.9.0 (*)
               |    |    +--- androidx.lifecycle:lifecycle-runtime:2.3.1 (*)
               |    |    +--- androidx.lifecycle:lifecycle-viewmodel:2.3.1 (*)
               |    |    +--- androidx.savedstate:savedstate:1.1.0
               |    |    |    +--- androidx.annotation:annotation:1.1.0 -> 1.3.0
               |    |    |    +--- androidx.arch.core:core-common:2.0.1 -> 2.1.0 (*)
               |    |    |    \--- androidx.lifecycle:lifecycle-common:2.0.0 -> 2.3.1 (*)
               |    |    +--- androidx.lifecycle:lifecycle-viewmodel-savedstate:2.3.1
               |    |    |    +--- androidx.annotation:annotation:1.0.0 -> 1.3.0
               |    |    |    +--- androidx.savedstate:savedstate:1.1.0 (*)
               |    |    |    +--- androidx.lifecycle:lifecycle-livedata-core:2.3.1 (*)
               |    |    |    \--- androidx.lifecycle:lifecycle-viewmodel:2.3.1 (*)
               |    |    +--- androidx.collection:collection:1.0.0 -> 1.1.0 (*)
               |    |    \--- androidx.tracing:tracing:1.0.0
               |    |         \--- androidx.annotation:annotation:1.1.0 -> 1.3.0
               |    +--- androidx.lifecycle:lifecycle-livedata-core:2.3.1 (*)
               |    +--- androidx.lifecycle:lifecycle-viewmodel:2.3.1 (*)
               |    +--- androidx.lifecycle:lifecycle-viewmodel-savedstate:2.3.1 (*)
               |    +--- androidx.savedstate:savedstate:1.1.0 (*)
               |    \--- androidx.annotation:annotation-experimental:1.0.0 -> 1.3.0 (*)
               +--- androidx.vectordrawable:vectordrawable-animated:1.1.0
               |    +--- androidx.vectordrawable:vectordrawable:1.1.0
               |    |    +--- androidx.annotation:annotation:1.1.0 -> 1.3.0
               |    |    +--- androidx.core:core:1.1.0 -> 1.9.0 (*)
               |    |    \--- androidx.collection:collection:1.1.0 (*)
               |    +--- androidx.interpolator:interpolator:1.0.0
               |    |    \--- androidx.annotation:annotation:1.0.0 -> 1.3.0
               |    \--- androidx.collection:collection:1.1.0 (*)
               +--- androidx.exifinterface:exifinterface:1.3.3
               |    \--- androidx.annotation:annotation:1.2.0 -> 1.3.0
               \--- androidx.tracing:tracing:1.0.0 (*)

(c) - dependency constraint
(*) - dependencies omitted (listed previously)

A web-based, searchable dependency report is available by adding the --scan option.

Deprecated Gradle features were used in this build, making it incompatible with Gradle 8.0.

You can use '--warning-mode all' to show the individual deprecation warnings and determine if they come from your own scripts or plugins.

See https://docs.gradle.org/7.6/userguide/command_line_interface.html#sec:command_line_warnings

BUILD SUCCESSFUL in 559ms
1 actionable task: 1 executed
"#;

        let actual = parse_dependencies_string(gradle_output).unwrap();
        let expected = vec![
            "androidx.core:core-ktx:1.9.0".into(),
            "com.github.bumptech.glide:glide:4.15.1".into(),
            "com.squareup.okhttp3:okhttp:4.9.3".into(),
            "org.jetbrains.kotlin:kotlin-stdlib-jdk8:1.6.21".to_owned(),
        ];

        assert_eq!(actual, expected);
    }

    #[test]
    fn parse_dependencies_string_release_runtime_classpath() {
        let gradle_output = r#"
releaseRuntimeClasspath - Runtime classpath of compilation 'release' (target  (androidJvm)).
+--- org.jetbrains.kotlin:kotlin-stdlib-jdk8:1.6.21
|    +--- org.jetbrains.kotlin:kotlin-stdlib:1.6.21 -> 1.7.10
|    |    +--- org.jetbrains.kotlin:kotlin-stdlib-common:1.7.10
|    |    \--- org.jetbrains:annotations:13.0
|    \--- org.jetbrains.kotlin:kotlin-stdlib-jdk7:1.6.21
|         \--- org.jetbrains.kotlin:kotlin-stdlib:1.6.21 -> 1.7.10 (*)
+--- androidx.activity:activity-compose:1.6.1
|    +--- androidx.activity:activity-ktx:1.6.1
|    |    +--- androidx.activity:activity:1.6.1
|    |    |    +--- androidx.annotation:annotation:1.1.0 -> 1.5.0
|    |    |    |    \--- org.jetbrains.kotlin:kotlin-stdlib:1.7.10 (*)
|    |    |    +--- androidx.collection:collection:1.0.0
|    |    |    |    \--- androidx.annotation:annotation:1.0.0 -> 1.5.0 (*)
|    |    |    +--- androidx.core:core:1.8.0
|    |    |    |    +--- androidx.annotation:annotation:1.2.0 -> 1.5.0 (*)
|    |    |    |    +--- androidx.annotation:annotation-experimental:1.1.0
|    |    |    |    +--- androidx.collection:collection:1.0.0 (*)
|    |    |    |    +--- androidx.concurrent:concurrent-futures:1.0.0 -> 1.1.0
|    |    |    |    |    +--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|    |    |    |    |    \--- com.google.guava:listenablefuture:1.0
|    |    |    |    +--- androidx.lifecycle:lifecycle-runtime:2.3.1 -> 2.5.1
|    |    |    |    |    +--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|    |    |    |    |    +--- androidx.arch.core:core-common:2.1.0
|    |    |    |    |    |    \--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|    |    |    |    |    +--- androidx.arch.core:core-runtime:2.1.0
|    |    |    |    |    |    +--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|    |    |    |    |    |    \--- androidx.arch.core:core-common:2.1.0 (*)
|    |    |    |    |    \--- androidx.lifecycle:lifecycle-common:2.5.1
|    |    |    |    |         +--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|    |    |    |    |         \--- androidx.lifecycle:lifecycle-common-java8:2.5.1 (c)
|    |    |    |    \--- androidx.versionedparcelable:versionedparcelable:1.1.1
|    |    |    |         +--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|    |    |    |         \--- androidx.collection:collection:1.0.0 (*)
|    |    |    +--- androidx.lifecycle:lifecycle-runtime:2.5.1 (*)
|    |    |    +--- androidx.lifecycle:lifecycle-viewmodel:2.5.1
|    |    |    |    +--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|    |    |    |    +--- org.jetbrains.kotlin:kotlin-stdlib:1.6.21 -> 1.7.10 (*)
|    |    |    |    \--- androidx.lifecycle:lifecycle-viewmodel-savedstate:2.5.1 (c)
|    |    |    +--- androidx.lifecycle:lifecycle-viewmodel-savedstate:2.5.1
|    |    |    |    +--- androidx.annotation:annotation:1.0.0 -> 1.5.0 (*)
|    |    |    |    +--- androidx.core:core-ktx:1.2.0 -> 1.5.0
|    |    |    |    |    +--- org.jetbrains.kotlin:kotlin-stdlib:1.4.31 -> 1.7.10 (*)
|    |    |    |    |    +--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|    |    |    |    |    \--- androidx.core:core:1.5.0 -> 1.8.0 (*)
|    |    |    |    +--- androidx.lifecycle:lifecycle-livedata-core:2.5.1
|    |    |    |    |    +--- androidx.arch.core:core-common:2.1.0 (*)
|    |    |    |    |    +--- androidx.arch.core:core-runtime:2.1.0 (*)
|    |    |    |    |    \--- androidx.lifecycle:lifecycle-common:2.5.1 (*)
|    |    |    |    +--- androidx.lifecycle:lifecycle-viewmodel:2.5.1 (*)
|    |    |    |    +--- androidx.savedstate:savedstate:1.2.0
|    |    |    |    |    +--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|    |    |    |    |    +--- androidx.arch.core:core-common:2.1.0 (*)
|    |    |    |    |    +--- androidx.lifecycle:lifecycle-common:2.4.0 -> 2.5.1 (*)
|    |    |    |    |    \--- org.jetbrains.kotlin:kotlin-stdlib:1.6.20 -> 1.7.10 (*)
|    |    |    |    +--- org.jetbrains.kotlin:kotlin-stdlib:1.6.21 -> 1.7.10 (*)
|    |    |    |    \--- org.jetbrains.kotlinx:kotlinx-coroutines-android:1.6.1 -> 1.6.4
|    |    |    |         +--- org.jetbrains.kotlinx:kotlinx-coroutines-core:1.6.4
|    |    |    |         |    \--- org.jetbrains.kotlinx:kotlinx-coroutines-core-jvm:1.6.4
|    |    |    |         |         +--- org.jetbrains.kotlinx:kotlinx-coroutines-bom:1.6.4
|    |    |    |         |         |    +--- org.jetbrains.kotlinx:kotlinx-coroutines-android:1.6.4 (c)
|    |    |    |         |         |    +--- org.jetbrains.kotlinx:kotlinx-coroutines-core-jvm:1.6.4 (c)
|    |    |    |         |         |    \--- org.jetbrains.kotlinx:kotlinx-coroutines-core:1.6.4 (c)
|    |    |    |         |         +--- org.jetbrains.kotlin:kotlin-stdlib-jdk8:1.6.21 (*)
|    |    |    |         |         \--- org.jetbrains.kotlin:kotlin-stdlib-common:1.6.21 -> 1.7.10
|    |    |    |         +--- org.jetbrains.kotlinx:kotlinx-coroutines-bom:1.6.4 (*)
|    |    |    |         \--- org.jetbrains.kotlin:kotlin-stdlib-jdk8:1.6.21 (*)
|    |    |    +--- androidx.savedstate:savedstate:1.2.0 (*)
|    |    |    +--- androidx.tracing:tracing:1.0.0
|    |    |    |    \--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|    |    |    +--- org.jetbrains.kotlin:kotlin-stdlib:1.7.10 (*)
|    |    |    \--- androidx.activity:activity-ktx:1.6.1 (c)
|    |    +--- androidx.core:core-ktx:1.1.0 -> 1.5.0 (*)
|    |    +--- androidx.lifecycle:lifecycle-runtime-ktx:2.5.1
|    |    |    +--- androidx.annotation:annotation:1.0.0 -> 1.5.0 (*)
|    |    |    +--- androidx.lifecycle:lifecycle-runtime:2.5.1 (*)
|    |    |    +--- org.jetbrains.kotlin:kotlin-stdlib:1.6.21 -> 1.7.10 (*)
|    |    |    \--- org.jetbrains.kotlinx:kotlinx-coroutines-android:1.6.1 -> 1.6.4 (*)
|    |    +--- androidx.lifecycle:lifecycle-viewmodel-ktx:2.5.1
|    |    |    +--- androidx.lifecycle:lifecycle-viewmodel:2.5.1 (*)
|    |    |    +--- org.jetbrains.kotlin:kotlin-stdlib:1.6.21 -> 1.7.10 (*)
|    |    |    \--- org.jetbrains.kotlinx:kotlinx-coroutines-android:1.6.1 -> 1.6.4 (*)
|    |    +--- androidx.savedstate:savedstate-ktx:1.2.0
|    |    |    +--- androidx.savedstate:savedstate:1.2.0 (*)
|    |    |    \--- org.jetbrains.kotlin:kotlin-stdlib:1.6.20 -> 1.7.10 (*)
|    |    +--- org.jetbrains.kotlin:kotlin-stdlib:1.7.10 (*)
|    |    \--- androidx.activity:activity:1.6.1 (c)
|    +--- androidx.compose.runtime:runtime:1.0.1 -> 1.3.3
|    |    +--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|    |    +--- org.jetbrains.kotlin:kotlin-stdlib:1.7.10 (*)
|    |    \--- org.jetbrains.kotlinx:kotlinx-coroutines-android:1.6.4 (*)
|    +--- androidx.compose.runtime:runtime-saveable:1.0.1 -> 1.3.3
|    |    +--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|    |    +--- androidx.compose.runtime:runtime:1.3.3 (*)
|    |    \--- org.jetbrains.kotlin:kotlin-stdlib:1.7.10 (*)
|    +--- androidx.compose.ui:ui:1.0.1 -> 1.3.3
|    |    +--- androidx.activity:activity-ktx:1.5.1 -> 1.6.1 (*)
|    |    +--- androidx.annotation:annotation:1.5.0 (*)
|    |    +--- androidx.autofill:autofill:1.0.0
|    |    |    \--- androidx.core:core:1.1.0 -> 1.8.0 (*)
|    |    +--- androidx.collection:collection:1.0.0 (*)
|    |    +--- androidx.compose.runtime:runtime:1.3.3 (*)
|    |    +--- androidx.compose.runtime:runtime-saveable:1.3.3 (*)
|    |    +--- androidx.compose.ui:ui-geometry:1.3.3
|    |    |    +--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|    |    |    +--- androidx.compose.runtime:runtime:1.1.1 -> 1.3.3 (*)
|    |    |    +--- androidx.compose.ui:ui-util:1.3.3
|    |    |    |    \--- org.jetbrains.kotlin:kotlin-stdlib:1.7.10 (*)
|    |    |    \--- org.jetbrains.kotlin:kotlin-stdlib:1.7.10 (*)
|    |    +--- androidx.compose.ui:ui-graphics:1.3.3
|    |    |    +--- androidx.annotation:annotation:1.2.0 -> 1.5.0 (*)
|    |    |    +--- androidx.compose.runtime:runtime:1.1.1 -> 1.3.3 (*)
|    |    |    +--- androidx.compose.ui:ui-unit:1.3.3
|    |    |    |    +--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|    |    |    |    +--- androidx.compose.runtime:runtime:1.1.1 -> 1.3.3 (*)
|    |    |    |    +--- androidx.compose.ui:ui-geometry:1.3.3 (*)
|    |    |    |    +--- androidx.compose.ui:ui-util:1.3.3 (*)
|    |    |    |    \--- org.jetbrains.kotlin:kotlin-stdlib:1.7.10 (*)
|    |    |    +--- androidx.compose.ui:ui-util:1.3.3 (*)
|    |    |    \--- org.jetbrains.kotlin:kotlin-stdlib-common:1.7.10
|    |    +--- androidx.compose.ui:ui-text:1.3.3
|    |    |    +--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|    |    |    +--- androidx.collection:collection:1.0.0 (*)
|    |    |    +--- androidx.compose.runtime:runtime:1.2.0 -> 1.3.3 (*)
|    |    |    +--- androidx.compose.runtime:runtime-saveable:1.2.0 -> 1.3.3 (*)
|    |    |    +--- androidx.compose.ui:ui-graphics:1.3.3 (*)
|    |    |    +--- androidx.compose.ui:ui-unit:1.3.3 (*)
|    |    |    +--- androidx.compose.ui:ui-util:1.3.3 (*)
|    |    |    +--- androidx.core:core:1.7.0 -> 1.8.0 (*)
|    |    |    +--- org.jetbrains.kotlin:kotlin-stdlib:1.7.10 (*)
|    |    |    +--- org.jetbrains.kotlin:kotlin-stdlib-common:1.7.10
|    |    |    \--- org.jetbrains.kotlinx:kotlinx-coroutines-core:1.6.4 (*)
|    |    +--- androidx.compose.ui:ui-unit:1.3.3 (*)
|    |    +--- androidx.compose.ui:ui-util:1.3.3 (*)
|    |    +--- androidx.core:core:1.5.0 -> 1.8.0 (*)
|    |    +--- androidx.customview:customview-poolingcontainer:1.0.0
|    |    |    +--- androidx.core:core-ktx:1.5.0 (*)
|    |    |    \--- org.jetbrains.kotlin:kotlin-stdlib:1.6.21 -> 1.7.10 (*)
|    |    +--- androidx.lifecycle:lifecycle-common-java8:2.3.0 -> 2.5.1
|    |    |    +--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|    |    |    \--- androidx.lifecycle:lifecycle-common:2.5.1 (*)
|    |    +--- androidx.lifecycle:lifecycle-runtime:2.3.0 -> 2.5.1 (*)
|    |    +--- androidx.lifecycle:lifecycle-viewmodel:2.3.0 -> 2.5.1 (*)
|    |    +--- androidx.profileinstaller:profileinstaller:1.2.0 -> 1.3.0
|    |    |    +--- androidx.annotation:annotation:1.2.0 -> 1.5.0 (*)
|    |    |    +--- androidx.concurrent:concurrent-futures:1.1.0 (*)
|    |    |    +--- androidx.startup:startup-runtime:1.1.1
|    |    |    |    +--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|    |    |    |    \--- androidx.tracing:tracing:1.0.0 (*)
|    |    |    \--- com.google.guava:listenablefuture:1.0
|    |    +--- androidx.savedstate:savedstate-ktx:1.2.0 (*)
|    |    +--- org.jetbrains.kotlin:kotlin-stdlib:1.7.10 (*)
|    |    +--- org.jetbrains.kotlin:kotlin-stdlib-common:1.7.10
|    |    +--- org.jetbrains.kotlinx:kotlinx-coroutines-android:1.6.4 (*)
|    |    \--- org.jetbrains.kotlinx:kotlinx-coroutines-core:1.6.4 (*)
|    +--- androidx.lifecycle:lifecycle-common-java8:2.5.1 (*)
|    \--- org.jetbrains.kotlin:kotlin-stdlib:1.7.10 (*)
+--- androidx.compose:compose-bom:2023.01.00
|    +--- androidx.compose.material:material:1.3.1 (c)
|    +--- androidx.compose.runtime:runtime:1.3.3 (c)
|    +--- androidx.compose.runtime:runtime-saveable:1.3.3 (c)
|    +--- androidx.compose.ui:ui:1.3.3 (c)
|    +--- androidx.compose.ui:ui-tooling:1.3.3 (c)
|    +--- androidx.compose.animation:animation:1.3.3 (c)
|    +--- androidx.compose.animation:animation-core:1.3.3 (c)
|    +--- androidx.compose.foundation:foundation:1.3.1 (c)
|    +--- androidx.compose.foundation:foundation-layout:1.3.1 (c)
|    +--- androidx.compose.material:material-icons-core:1.3.1 (c)
|    +--- androidx.compose.material:material-ripple:1.3.1 (c)
|    +--- androidx.compose.ui:ui-text:1.3.3 (c)
|    +--- androidx.compose.ui:ui-util:1.3.3 (c)
|    +--- androidx.compose.ui:ui-geometry:1.3.3 (c)
|    +--- androidx.compose.ui:ui-graphics:1.3.3 (c)
|    +--- androidx.compose.ui:ui-unit:1.3.3 (c)
|    +--- androidx.compose.ui:ui-tooling-data:1.3.3 (c)
|    \--- androidx.compose.ui:ui-tooling-preview:1.3.3 (c)
+--- androidx.compose.ui:ui-tooling -> 1.3.3
|    +--- androidx.activity:activity-compose:1.3.0 -> 1.6.1 (*)
|    +--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|    +--- androidx.compose.animation:animation:1.3.3
|    |    +--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|    |    +--- androidx.compose.animation:animation-core:1.3.3
|    |    |    +--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|    |    |    +--- androidx.compose.runtime:runtime:1.1.1 -> 1.3.3 (*)
|    |    |    +--- androidx.compose.ui:ui:1.2.0 -> 1.3.3 (*)
|    |    |    +--- androidx.compose.ui:ui-unit:1.0.0 -> 1.3.3 (*)
|    |    |    +--- androidx.compose.ui:ui-util:1.0.0 -> 1.3.3 (*)
|    |    |    +--- org.jetbrains.kotlin:kotlin-stdlib:1.7.10 (*)
|    |    |    \--- org.jetbrains.kotlinx:kotlinx-coroutines-core:1.6.4 (*)
|    |    +--- androidx.compose.foundation:foundation-layout:1.0.0 -> 1.3.1
|    |    |    +--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|    |    |    +--- androidx.compose.animation:animation-core:1.1.1 -> 1.3.3 (*)
|    |    |    +--- androidx.compose.runtime:runtime:1.2.0 -> 1.3.3 (*)
|    |    |    +--- androidx.compose.ui:ui:1.2.0 -> 1.3.3 (*)
|    |    |    +--- androidx.compose.ui:ui-unit:1.1.1 -> 1.3.3 (*)
|    |    |    +--- androidx.compose.ui:ui-util:1.0.0 -> 1.3.3 (*)
|    |    |    +--- androidx.core:core:1.7.0 -> 1.8.0 (*)
|    |    |    \--- org.jetbrains.kotlin:kotlin-stdlib-common:1.7.10
|    |    +--- androidx.compose.runtime:runtime:1.1.1 -> 1.3.3 (*)
|    |    +--- androidx.compose.ui:ui:1.0.0 -> 1.3.3 (*)
|    |    +--- androidx.compose.ui:ui-geometry:1.0.0 -> 1.3.3 (*)
|    |    +--- androidx.compose.ui:ui-util:1.0.0 -> 1.3.3 (*)
|    |    \--- org.jetbrains.kotlin:kotlin-stdlib-common:1.7.10
|    +--- androidx.compose.material:material:1.0.0 -> 1.3.1
|    |    +--- androidx.compose.animation:animation:1.0.0 -> 1.3.3 (*)
|    |    +--- androidx.compose.animation:animation-core:1.0.0 -> 1.3.3 (*)
|    |    +--- androidx.compose.foundation:foundation:1.2.0 -> 1.3.1
|    |    |    +--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|    |    |    +--- androidx.compose.animation:animation:1.1.1 -> 1.3.3 (*)
|    |    |    +--- androidx.compose.foundation:foundation-layout:1.3.1 (*)
|    |    |    +--- androidx.compose.runtime:runtime:1.3.1 -> 1.3.3 (*)
|    |    |    +--- androidx.compose.ui:ui:1.3.1 -> 1.3.3 (*)
|    |    |    +--- androidx.compose.ui:ui-graphics:1.1.1 -> 1.3.3 (*)
|    |    |    +--- androidx.compose.ui:ui-text:1.0.0 -> 1.3.3 (*)
|    |    |    +--- androidx.compose.ui:ui-util:1.0.0 -> 1.3.3 (*)
|    |    |    \--- org.jetbrains.kotlin:kotlin-stdlib-common:1.7.10
|    |    +--- androidx.compose.foundation:foundation-layout:1.1.1 -> 1.3.1 (*)
|    |    +--- androidx.compose.material:material-icons-core:1.3.1
|    |    |    +--- androidx.compose.runtime:runtime:1.1.1 -> 1.3.3 (*)
|    |    |    +--- androidx.compose.ui:ui:1.0.0 -> 1.3.3 (*)
|    |    |    \--- org.jetbrains.kotlin:kotlin-stdlib:1.7.10 (*)
|    |    +--- androidx.compose.material:material-ripple:1.3.1
|    |    |    +--- androidx.compose.animation:animation:1.0.0 -> 1.3.3 (*)
|    |    |    +--- androidx.compose.foundation:foundation:1.1.1 -> 1.3.1 (*)
|    |    |    +--- androidx.compose.runtime:runtime:1.1.1 -> 1.3.3 (*)
|    |    |    +--- androidx.compose.ui:ui-util:1.0.0 -> 1.3.3 (*)
|    |    |    \--- org.jetbrains.kotlin:kotlin-stdlib-common:1.7.10
|    |    +--- androidx.compose.runtime:runtime:1.2.0 -> 1.3.3 (*)
|    |    +--- androidx.compose.ui:ui:1.2.0 -> 1.3.3 (*)
|    |    +--- androidx.compose.ui:ui-text:1.2.0 -> 1.3.3 (*)
|    |    +--- androidx.compose.ui:ui-util:1.0.0 -> 1.3.3 (*)
|    |    +--- androidx.lifecycle:lifecycle-runtime:2.3.0 -> 2.5.1 (*)
|    |    +--- androidx.lifecycle:lifecycle-viewmodel:2.3.0 -> 2.5.1 (*)
|    |    +--- androidx.savedstate:savedstate:1.1.0 -> 1.2.0 (*)
|    |    \--- org.jetbrains.kotlin:kotlin-stdlib-common:1.7.10
|    +--- androidx.compose.runtime:runtime:1.1.1 -> 1.3.3 (*)
|    +--- androidx.compose.ui:ui:1.3.3 (*)
|    +--- androidx.compose.ui:ui-tooling-data:1.3.3
|    |    +--- androidx.annotation:annotation:1.1.0 -> 1.5.0 (*)
|    |    +--- androidx.compose.runtime:runtime:1.2.0 -> 1.3.3 (*)
|    |    +--- androidx.compose.ui:ui:1.3.3 (*)
|    |    \--- org.jetbrains.kotlin:kotlin-stdlib:1.7.10 (*)
|    +--- androidx.compose.ui:ui-tooling-preview:1.3.3
|    |    +--- androidx.annotation:annotation:1.2.0 -> 1.5.0 (*)
|    |    +--- androidx.compose.runtime:runtime:1.1.1 -> 1.3.3 (*)
|    |    \--- org.jetbrains.kotlin:kotlin-stdlib:1.7.10 (*)
|    +--- androidx.savedstate:savedstate-ktx:1.2.0 (*)
|    \--- org.jetbrains.kotlin:kotlin-stdlib:1.7.10 (*)
+--- androidx.compose.material:material -> 1.3.1 (*)
\--- androidx.profileinstaller:profileinstaller:1.3.0 (*)
"#;

        let actual = parse_dependencies_string(gradle_output).unwrap();
        let expected = vec![
            "androidx.activity:activity-compose:1.6.1".to_owned(),
            "androidx.compose.material:material:1.3.1".into(),
            "androidx.compose.ui:ui-tooling:1.3.3".into(),
            "androidx.compose:compose-bom:2023.01.00".into(),
            "androidx.profileinstaller:profileinstaller:1.3.0".into(),
            "org.jetbrains.kotlin:kotlin-stdlib-jdk8:1.6.21".into(),
        ];

        assert_eq!(actual, expected);
    }
}
