Collect OSS information for maven artifacts
===========================================

https://github.com/sukawasatoru/oss-info-maven/assets/12950393/8bc9b7d5-37e6-4d90-88e8-37bf1a8f3a6a

Usage
-----

```
Collect OSS information from server

Usage: oss-info-maven [OPTIONS]

Options:
      --format <FORMAT>          Output format type [default: csv] [possible values: csv]
      --skip-pretty              Parse stdin as manually formatted Gradle output
      --completion <COMPLETION>  Generate shell completions [possible values: bash, elvish, fish, powershell, zsh]
  -h, --help                     Print help
```

### e.g. ###

```shell
cd path/to/android-repo
./gradlew :app:dependencies --configuration releaseRuntimeClasspath | oss-info-maven | tee out.csv
```

LICENSE
-------

```
   Copyright 2023 sukawasatoru

   Licensed under the Apache License, Version 2.0 (the "License");
   you may not use this file except in compliance with the License.
   You may obtain a copy of the License at 

       http://www.apache.org/licenses/LICENSE-2.0

   Unless required by applicable law or agreed to in writing, software
   distributed under the License is distributed on an "AS IS" BASIS,
   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
   See the License for the specific language governing permissions and
   limitations under the License.
```
