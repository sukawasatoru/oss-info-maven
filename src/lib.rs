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

use crate::function::maven::{parse_maven_metadata, parse_pom, POM};
pub use crate::prelude::*;

pub mod function;
pub mod model;
pub mod prelude;

#[tracing::instrument(skip_all)]
pub async fn retrieve_maven_lib(client: reqwest::Client, dependency_name: &str) -> Fallible<POM> {
    let repo_root = match dependency_name {
        data if data.starts_with("androidx") => get_google_maven_repo(),
        data if data.starts_with("com.google.android") => get_google_maven_repo(),
        _ => get_maven_central_repo(),
    };

    retrieve_maven_lib_impl(client, dependency_name, repo_root).await
}

/// https://maven.google.com/web/index.html
fn get_google_maven_repo() -> &'static str {
    #[cfg(not(test))]
    let repo_root = "https://dl.google.com/android/maven2";

    #[cfg(test)]
    let repo_root = "http://127.0.0.1";

    repo_root
}

/// https://central.sonatype.com/
fn get_maven_central_repo() -> &'static str {
    #[cfg(not(test))]
    let repo_root = "https://repo1.maven.org/maven2";

    #[cfg(test)]
    let repo_root = "http://127.0.0.1";

    repo_root
}

/// https://maven.apache.org/repository/layout.html
#[tracing::instrument(skip(client, dependency_name))]
async fn retrieve_maven_lib_impl(
    client: reqwest::Client,
    dependency_name: &str,
    repo_root: &str,
) -> Fallible<POM> {
    let artifact_root_path = format!(
        "{}/{}",
        repo_root,
        split_dependency_name_to_path(dependency_name)?,
    );

    let artifact_metadata_path = format!("{}/{}", artifact_root_path, "maven-metadata.xml");
    let res = client
        .get(&artifact_metadata_path)
        .header(reqwest::header::ACCEPT, "application/xml,text/xml")
        .send()
        .await
        .with_context(|| {
            format!(
                "failed to request maven-metadata.xml. url: {}",
                artifact_metadata_path,
            )
        })?;
    let maven_metadata_xml = res
        .error_for_status()
        .context("server returned an error for maven-metadata.xml")?
        .text()
        .await
        .context("failed to parse response to maven-metadata.xml's string")?;
    trace!(%maven_metadata_xml);

    let maven_metadata =
        parse_maven_metadata(&maven_metadata_xml).context("failed to parse maven-metadata.xml")?;
    debug!(?maven_metadata);

    let pom_path = format!(
        "{base}/{version}/{artifact}-{version}.pom",
        base = artifact_root_path,
        version = maven_metadata
            .release_version
            .or(maven_metadata.latest_version)
            .or_else(|| {
                info!("use version tag");
                maven_metadata.version
            })
            .with_context(|| format!(
                "missing release, latest and version: {}",
                artifact_metadata_path
            ))?,
        artifact = maven_metadata.artifact_id,
    );

    let res = client
        .get(&pom_path)
        .header(reqwest::header::ACCEPT, "application/xml,text/xml")
        .send()
        .await
        .with_context(|| format!("failed to request pom.xml. url: {}", pom_path))?;
    let pom_xml = res
        .error_for_status()
        .context("server returned an error for pom.xml")?
        .text()
        .await
        .context("failed to parse response to pom.xml's string")?;
    trace!(%pom_xml);

    parse_pom(&pom_xml).context("failed to parse pom.xml")
}

fn split_dependency_name_to_path(dependency_name: &str) -> Fallible<String> {
    let mut segments = dependency_name.split(":");
    let group_id = segments
        .next()
        .expect("unexpected format?")
        .trim()
        .replace('.', "/");
    ensure!(
        !group_id.is_empty(),
        "missing group id: {}",
        dependency_name
    );

    let artifact_id = segments.next().context("missing artifact id")?.trim();
    ensure!(
        !artifact_id.is_empty(),
        "missing artifact id: {}",
        dependency_name
    );

    if segments.next().is_some() {
        info!("ignore version of {}", dependency_name);
    }

    Ok(format!("{}/{}", group_id, artifact_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::function::mock_server::{acquire_port, PortGuard};
    use crate::model::SPDX;
    use axum::response::Html;
    use axum::routing::{get, IntoMakeService, Router};
    use std::net::SocketAddr;
    use tokio::task::JoinHandle;

    #[tokio::test]
    async fn retrieve_maven_lib_impl_core_ktx_1_12_0() {
        async fn get_maven_metadata() -> Html<&'static str> {
            Html(
                r#"<?xml version='1.0' encoding='UTF-8'?>
<metadata>
  <groupId>androidx.core</groupId>
  <artifactId>core-ktx</artifactId>
  <versioning>
    <latest>1.12.0</latest>
    <release>1.12.0</release>
    <versions>
      <version>0.1</version>
      <version>0.2</version>
      <version>0.3</version>
      <version>1.0.0-alpha1</version>
      <version>1.0.0-alpha3</version>
      <version>1.0.0-beta01</version>
      <version>1.0.0-rc01</version>
      <version>1.0.0-rc02</version>
      <version>1.0.0</version>
      <version>1.0.1</version>
      <version>1.0.2</version>
      <version>1.1.0-alpha02</version>
      <version>1.1.0-alpha03</version>
      <version>1.1.0-alpha04</version>
      <version>1.1.0-alpha05</version>
      <version>1.1.0-beta01</version>
      <version>1.1.0-rc01</version>
      <version>1.1.0-rc02</version>
      <version>1.1.0-rc03</version>
      <version>1.1.0</version>
      <version>1.2.0-alpha01</version>
      <version>1.2.0-alpha02</version>
      <version>1.2.0-alpha03</version>
      <version>1.2.0-alpha04</version>
      <version>1.2.0-beta01</version>
      <version>1.2.0-beta02</version>
      <version>1.2.0-rc01</version>
      <version>1.2.0</version>
      <version>1.3.0-alpha01</version>
      <version>1.3.0-alpha02</version>
      <version>1.3.0-beta01</version>
      <version>1.3.0-rc01</version>
      <version>1.3.0</version>
      <version>1.3.1</version>
      <version>1.3.2</version>
      <version>1.4.0-alpha01</version>
      <version>1.5.0-alpha01</version>
      <version>1.5.0-alpha02</version>
      <version>1.5.0-alpha03</version>
      <version>1.5.0-alpha04</version>
      <version>1.5.0-alpha05</version>
      <version>1.5.0-beta01</version>
      <version>1.5.0-beta02</version>
      <version>1.5.0-beta03</version>
      <version>1.5.0-rc01</version>
      <version>1.5.0-rc02</version>
      <version>1.5.0</version>
      <version>1.6.0-alpha01</version>
      <version>1.6.0-alpha02</version>
      <version>1.6.0-alpha03</version>
      <version>1.6.0-beta01</version>
      <version>1.6.0-beta02</version>
      <version>1.6.0-rc01</version>
      <version>1.6.0</version>
      <version>1.7.0-alpha01</version>
      <version>1.7.0-alpha02</version>
      <version>1.7.0-beta01</version>
      <version>1.7.0-beta02</version>
      <version>1.7.0-rc01</version>
      <version>1.7.0</version>
      <version>1.8.0-alpha01</version>
      <version>1.8.0-alpha02</version>
      <version>1.8.0-alpha03</version>
      <version>1.8.0-alpha04</version>
      <version>1.8.0-alpha05</version>
      <version>1.8.0-alpha06</version>
      <version>1.8.0-alpha07</version>
      <version>1.8.0-beta01</version>
      <version>1.8.0-rc01</version>
      <version>1.8.0-rc02</version>
      <version>1.8.0</version>
      <version>1.9.0-alpha01</version>
      <version>1.9.0-alpha02</version>
      <version>1.9.0-alpha03</version>
      <version>1.9.0-alpha04</version>
      <version>1.9.0-alpha05</version>
      <version>1.9.0-beta01</version>
      <version>1.9.0-rc01</version>
      <version>1.9.0</version>
      <version>1.10.0-alpha01</version>
      <version>1.10.0-alpha02</version>
      <version>1.10.0-beta01</version>
      <version>1.10.0-rc01</version>
      <version>1.10.0</version>
      <version>1.10.1</version>
      <version>1.11.0-alpha01</version>
      <version>1.11.0-alpha02</version>
      <version>1.11.0-alpha03</version>
      <version>1.11.0-alpha04</version>
      <version>1.11.0-beta01</version>
      <version>1.11.0-beta02</version>
      <version>1.12.0-alpha01</version>
      <version>1.12.0-alpha03</version>
      <version>1.12.0-alpha04</version>
      <version>1.12.0-alpha05</version>
      <version>1.12.0-beta01</version>
      <version>1.12.0-rc01</version>
      <version>1.12.0</version>
    </versions>
    <lastUpdated>20230904154022</lastUpdated>
  </versioning>
</metadata>
"#,
            )
        }

        async fn get_pom() -> Html<&'static str> {
            Html(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:schemaLocation="http://maven.apache.org/POM/4.0.0 https://maven.apache.org/xsd/maven-4.0.0.xsd">
  <!-- This module was also published with a richer model, Gradle metadata,  -->
  <!-- which should be used instead. Do not delete the following line which  -->
  <!-- is to indicate to Gradle or any Gradle module metadata file consumer  -->
  <!-- that they should prefer consuming it instead. -->
  <!-- do_not_remove: published-with-gradle-metadata -->
  <modelVersion>4.0.0</modelVersion>
  <groupId>androidx.core</groupId>
  <artifactId>core-ktx</artifactId>
  <version>1.12.0</version>
  <packaging>aar</packaging>
  <name>Core Kotlin Extensions</name>
  <description>Kotlin extensions for 'core' artifact</description>
  <url>https://developer.android.com/jetpack/androidx/releases/core#1.12.0</url>
  <inceptionYear>2018</inceptionYear>
  <licenses>
    <license>
      <name>The Apache Software License, Version 2.0</name>
      <url>http://www.apache.org/licenses/LICENSE-2.0.txt</url>
      <distribution>repo</distribution>
    </license>
  </licenses>
  <developers>
    <developer>
      <name>The Android Open Source Project</name>
    </developer>
  </developers>
  <scm>
    <connection>scm:git:https://android.googlesource.com/platform/frameworks/support</connection>
    <url>https://cs.android.com/androidx/platform/frameworks/support</url>
  </scm>
  <dependencyManagement>
    <dependencies>
      <dependency>
        <groupId>androidx.core</groupId>
        <artifactId>core</artifactId>
        <version>1.12.0</version>
      </dependency>
    </dependencies>
  </dependencyManagement>
  <dependencies>
    <dependency>
      <groupId>androidx.annotation</groupId>
      <artifactId>annotation</artifactId>
      <version>1.1.0</version>
      <scope>compile</scope>
    </dependency>
    <dependency>
      <groupId>androidx.core</groupId>
      <artifactId>core</artifactId>
      <version>1.12.0</version>
      <scope>compile</scope>
      <type>aar</type>
    </dependency>
    <dependency>
      <groupId>org.jetbrains.kotlin</groupId>
      <artifactId>kotlin-stdlib</artifactId>
      <version>1.8.22</version>
      <scope>compile</scope>
    </dependency>
  </dependencies>
</project>
"#,
            )
        }

        let (handler, tx, port) = launch_web_server(
            Router::new()
                .route(
                    "/androidx/core/core-ktx/maven-metadata.xml",
                    get(get_maven_metadata),
                )
                .route(
                    "/androidx/core/core-ktx/1.12.0/core-ktx-1.12.0.pom",
                    get(get_pom),
                )
                .into_make_service(),
        )
        .await;

        let repo_root = format!("http://127.0.0.1:{}", *port);
        let actual =
            retrieve_maven_lib_impl(reqwest::Client::new(), "androidx.core:core-ktx", &repo_root)
                .await;

        tx.send(()).unwrap();
        handler.await.unwrap();

        let actual = actual.unwrap();
        let expected = POM {
            group_id: Some("androidx.core".into()),
            artifact_id: "core-ktx".into(),
            version: Some("1.12.0".into()),
            packaging: Some("aar".into()),
            name: Some("Core Kotlin Extensions".into()),
            description: Some("Kotlin extensions for 'core' artifact".into()),
            licenses: vec![SPDX::Apache20],
        };

        assert_eq!(expected, actual);
    }

    #[tokio::test]
    async fn retrieve_maven_lib_impl_glide_4_16_0() {
        async fn get_maven_metadata() -> Html<&'static str> {
            Html(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<metadata>
  <groupId>com.github.bumptech.glide</groupId>
  <artifactId>glide</artifactId>
  <versioning>
    <latest>4.16.0</latest>
    <release>4.16.0</release>
    <versions>
      <version>3.3.0</version>
      <version>3.3.1</version>
      <version>3.4.0</version>
      <version>3.5.0</version>
      <version>3.5.1</version>
      <version>3.5.2</version>
      <version>3.6.0</version>
      <version>3.6.1</version>
      <version>3.7.0</version>
      <version>3.8.0</version>
      <version>4.0.0-RC0</version>
      <version>4.0.0-RC1</version>
      <version>4.0.0</version>
      <version>4.1.0</version>
      <version>4.1.1</version>
      <version>4.2.0</version>
      <version>4.3.0</version>
      <version>4.3.1</version>
      <version>4.4.0</version>
      <version>4.5.0</version>
      <version>4.6.0</version>
      <version>4.6.1</version>
      <version>4.7.0</version>
      <version>4.7.1</version>
      <version>4.8.0</version>
      <version>4.9.0</version>
      <version>4.10.0</version>
      <version>4.11.0</version>
      <version>4.12.0</version>
      <version>4.13.0</version>
      <version>4.13.1</version>
      <version>4.13.2</version>
      <version>4.14.0</version>
      <version>4.14.1</version>
      <version>4.14.2</version>
      <version>4.15.0</version>
      <version>4.15.1</version>
      <version>4.16.0</version>
    </versions>
    <lastUpdated>20230821070349</lastUpdated>
  </versioning>
</metadata>
"#,
            )
        }

        async fn get_pom() -> Html<&'static str> {
            Html(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0" xsi:schemaLocation="http://maven.apache.org/POM/4.0.0 https://maven.apache.org/xsd/maven-4.0.0.xsd" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
  <modelVersion>4.0.0</modelVersion>
  <groupId>com.github.bumptech.glide</groupId>
  <artifactId>glide</artifactId>
  <version>4.16.0</version>
  <packaging>aar</packaging>
  <name>Glide</name>
  <description>A fast and efficient image loading library for Android focused on smooth scrolling.</description>
  <url>https://github.com/bumptech/glide</url>
  <licenses>
    <license>
      <name>Simplified BSD License</name>
      <url>http://www.opensource.org/licenses/bsd-license</url>
      <distribution>repo</distribution>
    </license>
    <license>
      <name>The Apache Software License, Version 2.0</name>
      <url>http://www.apache.org/licenses/LICENSE-2.0.txt</url>
      <distribution>repo</distribution>
    </license>
  </licenses>
  <developers>
    <developer>
      <id>sjudd</id>
      <name>Sam Judd</name>
      <email>judds@google.com</email>
    </developer>
  </developers>
  <scm>
    <connection>scm:git@github.com:bumptech/glide.git</connection>
    <developerConnection>scm:git@github.com:bumptech/glide.git</developerConnection>
    <url>https://github.com/bumptech/glide</url>
  </scm>
  <dependencies>
    <dependency>
      <groupId>com.github.bumptech.glide</groupId>
      <artifactId>gifdecoder</artifactId>
      <version>4.16.0</version>
      <scope>compile</scope>
    </dependency>
    <dependency>
      <groupId>com.github.bumptech.glide</groupId>
      <artifactId>disklrucache</artifactId>
      <version>4.16.0</version>
      <scope>compile</scope>
    </dependency>
    <dependency>
      <groupId>com.github.bumptech.glide</groupId>
      <artifactId>annotations</artifactId>
      <version>4.16.0</version>
      <scope>compile</scope>
    </dependency>
    <dependency>
      <groupId>androidx.fragment</groupId>
      <artifactId>fragment</artifactId>
      <version>1.3.6</version>
      <scope>compile</scope>
    </dependency>
    <dependency>
      <groupId>androidx.vectordrawable</groupId>
      <artifactId>vectordrawable-animated</artifactId>
      <version>1.1.0</version>
      <scope>compile</scope>
    </dependency>
    <dependency>
      <groupId>androidx.exifinterface</groupId>
      <artifactId>exifinterface</artifactId>
      <version>1.3.6</version>
      <scope>compile</scope>
    </dependency>
    <dependency>
      <groupId>androidx.tracing</groupId>
      <artifactId>tracing</artifactId>
      <version>1.0.0</version>
      <scope>compile</scope>
    </dependency>
  </dependencies>
</project>
"#,
            )
        }

        let (handler, tx, port) = launch_web_server(
            Router::new()
                .route(
                    "/com/github/bumptech/glide/glide/maven-metadata.xml",
                    get(get_maven_metadata),
                )
                .route(
                    "/com/github/bumptech/glide/glide/4.16.0/glide-4.16.0.pom",
                    get(get_pom),
                )
                .into_make_service(),
        )
        .await;

        let repo_root = format!("http://127.0.0.1:{}", *port);
        let actual = retrieve_maven_lib_impl(
            reqwest::Client::new(),
            "com.github.bumptech.glide:glide",
            &repo_root,
        )
        .await;

        tx.send(()).unwrap();
        handler.await.unwrap();

        let actual = actual.unwrap();
        let expected = POM {
            group_id: Some("com.github.bumptech.glide".into()),
            artifact_id: "glide".into(),
            version: Some("4.16.0".into()),
            packaging: Some("aar".into()),
            name: Some("Glide".into()),
            description: Some("A fast and efficient image loading library for Android focused on smooth scrolling.".into()),
            licenses: vec![SPDX::BSD2, SPDX::Apache20],
        };

        assert_eq!(expected, actual);
    }

    #[test]
    fn split_dependency_name_to_path_core_ktx() {
        let source = "androidx.core:core-ktx";
        let expected = "androidx/core/core-ktx";

        let actual = split_dependency_name_to_path(source).unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    fn split_dependency_name_to_path_core_ktx_version() {
        let source = "androidx.core:core-ktx:1.1.0";
        let expected = "androidx/core/core-ktx";

        let actual = split_dependency_name_to_path(source).unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    fn split_dependency_name_to_path_javax_inject() {
        let source = "javax.inject:javax.inject";
        let expected = "javax/inject/javax.inject";

        let actual = split_dependency_name_to_path(source).unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    fn split_dependency_name_to_path_unexpected_format() {
        let actual = split_dependency_name_to_path("aaa");
        assert!(actual.is_err());
    }

    async fn launch_web_server(
        make_service: IntoMakeService<Router>,
    ) -> (JoinHandle<()>, tokio::sync::oneshot::Sender<()>, PortGuard) {
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();

        let port_guard = acquire_port().await;
        let port = *port_guard;
        let handler = tokio::task::spawn(async move {
            axum::Server::bind(&SocketAddr::from(([127, 0, 0, 1], port)))
                .serve(make_service)
                .with_graceful_shutdown(async {
                    rx.await.ok();
                })
                .await
                .unwrap();
        });

        // yield for launching server.
        tokio::task::yield_now().await;

        (handler, tx, port_guard)
    }
}
