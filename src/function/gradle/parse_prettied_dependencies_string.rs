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
use std::io::prelude::*;
use std::io::BufReader;

pub fn parse_prettied_dependencies_string() -> Fallible<Vec<String>> {
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
