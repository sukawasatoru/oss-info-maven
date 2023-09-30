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

use std::ops::Deref;
use std::sync::Mutex;
use tokio::sync::Semaphore;
use tokio::sync::SemaphorePermit;

const DEFAULT_PORTS_LEN: usize = 9;
const DEFAULT_PORTS: [u16; DEFAULT_PORTS_LEN] = [
    38092, 38093, 38094, 38095, 38096, 38097, 38098, 38099, 38100,
];

static POOL: Pool = Pool::new();

pub async fn acquire_port() -> PortGuard {
    POOL.acquire().await
}

pub struct PortGuard {
    _permit: SemaphorePermit<'static>,
    pub port: u16,
}

impl Deref for PortGuard {
    type Target = u16;

    fn deref(&self) -> &Self::Target {
        &self.port
    }
}

impl Drop for PortGuard {
    fn drop(&mut self) {
        let mut pool = POOL.internal.lock().unwrap();
        pool.ports.push(self.port);
    }
}

struct Pool {
    internal: Mutex<PoolInternal>,
    semaphore: Semaphore,
}

impl Pool {
    const fn new() -> Self {
        Self {
            internal: Mutex::new(PoolInternal::new()),
            semaphore: Semaphore::const_new(DEFAULT_PORTS_LEN),
        }
    }

    async fn acquire(&'static self) -> PortGuard {
        let permit = self.semaphore.acquire().await.unwrap();
        let mut pool = self.internal.lock().unwrap();
        if !pool.initialized {
            pool.initialized = true;
            pool.ports.extend_from_slice(&DEFAULT_PORTS[..]);
        }

        let port = pool.ports.pop().unwrap();

        drop(pool);

        PortGuard {
            _permit: permit,
            port,
        }
    }
}

struct PoolInternal {
    initialized: bool,
    ports: Vec<u16>,
}

impl PoolInternal {
    const fn new() -> Self {
        Self {
            initialized: false,
            ports: vec![],
        }
    }
}

#[tokio::test]
async fn acquire_release() {
    let port1 = acquire_port().await;
    assert_eq!(38100, *port1);

    let port2 = acquire_port().await;
    assert_eq!(38099, *port2);

    let port3 = acquire_port().await;
    assert_eq!(38098, *port3);

    let port4 = acquire_port().await;
    assert_eq!(38097, *port4);

    let port5 = acquire_port().await;
    assert_eq!(38096, *port5);

    let port6 = acquire_port().await;
    assert_eq!(38095, *port6);

    let port7 = acquire_port().await;
    assert_eq!(38094, *port7);

    let port8 = acquire_port().await;
    assert_eq!(38093, *port8);

    let port9 = acquire_port().await;
    assert_eq!(38092, *port9);

    let actual = POOL.semaphore.try_acquire();
    assert!(actual.is_err());

    drop(port1);

    let port10 = acquire_port().await;
    assert_eq!(38100, *port10);
}
