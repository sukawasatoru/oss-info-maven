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

//! https://spdx.org/licenses/

use std::convert::Infallible;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

#[derive(Debug, Eq, PartialEq)]
pub enum SPDX {
    Apache20,
    BSD2,
    BSD3,
    ISC,
    MIT,
    Other(String),
}

impl Display for SPDX {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Apache20 => f.write_str("Apache-2.0"),
            Self::BSD2 => f.write_str("BSD-2-Clause"),
            Self::BSD3 => f.write_str("BSD-3-Clause"),
            Self::MIT => f.write_str("MIT"),
            Self::ISC => f.write_str("ISC"),
            Self::Other(data) => f.write_str(data),
        }
    }
}

impl FromStr for SPDX {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "The Apache Software License, Version 2.0"
            | "The Apache License, Version 2.0"
            | "Apache 2.0" => Self::Apache20,
            "Simplified BSD License" => Self::BSD2,
            "ISC License" => Self::ISC,
            "MIT License" => Self::MIT,
            _ => Self::Other(s.into()),
        })
    }
}
