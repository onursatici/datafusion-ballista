// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.

use std::env;
use std::error::Error;
use std::path::PathBuf;

use ballista::prelude::BallistaConfig;
use ballista_core::serde::{
    protobuf::scheduler_grpc_client::SchedulerGrpcClient, BallistaCodec,
};

// /// Remote ballista cluster to be used for local testing.
// static BALLISTA_CLUSTER: tokio::sync::OnceCell<(String, u16)> =
//     tokio::sync::OnceCell::const_new();

/// Returns the parquet test data directory, which is by default
/// stored in a git submodule rooted at
/// `examples/testdata`.
///
/// The default can be overridden by the optional environment variable
/// `EXAMPLES_TEST_DATA`
///
/// panics when the directory can not be found.
///
/// Example:
/// ```
/// use ballista_examples::test_util;
/// let testdata = test_util::examples_test_data();
/// let filename = format!("{testdata}/aggregate_test_100.csv");
/// assert!(std::path::PathBuf::from(filename).exists());
/// ```
#[allow(dead_code)]
pub fn example_test_data() -> String {
    match get_data_dir("EXAMPLES_TEST_DATA", "testdata") {
        Ok(pb) => pb.display().to_string(),
        Err(err) => panic!("failed to get examples test data dir: {err}"),
    }
}

/// Returns a directory path for finding test data.
///
/// udf_env: name of an environment variable
///
/// submodule_dir: fallback path (relative to CARGO_MANIFEST_DIR)
///
///  Returns either:
/// The path referred to in `udf_env` if that variable is set and refers to a directory
/// The submodule_data directory relative to CARGO_MANIFEST_PATH
#[allow(dead_code)]
fn get_data_dir(udf_env: &str, submodule_data: &str) -> Result<PathBuf, Box<dyn Error>> {
    // Try user defined env.
    if let Ok(dir) = env::var(udf_env) {
        let trimmed = dir.trim().to_string();
        if !trimmed.is_empty() {
            let pb = PathBuf::from(trimmed);
            if pb.is_dir() {
                return Ok(pb);
            } else {
                return Err(format!(
                    "the data dir `{}` defined by env {udf_env} not found",
                    pb.display()
                )
                .into());
            }
        }
    }

    // The env is undefined or its value is trimmed to empty, let's try default dir.

    // env "CARGO_MANIFEST_DIR" is "the directory containing the manifest of your package",
    // set by `cargo run` or `cargo test`, see:
    // https://doc.rust-lang.org/cargo/reference/environment-variables.html
    let dir = env!("CARGO_MANIFEST_DIR");

    let pb = PathBuf::from(dir).join(submodule_data);
    if pb.is_dir() {
        Ok(pb)
    } else {
        Err(format!(
            "env `{udf_env}` is undefined or has empty value, and the pre-defined data dir `{}` not found\n\
             HINT: try running `git submodule update --init`",
            pb.display(),
        ).into())
    }
}

/// starts a ballista cluster for integration tests
#[allow(dead_code)]
pub async fn setup_test_cluster() -> (String, u16) {
    let config = BallistaConfig::builder().build().unwrap();
    let default_codec = BallistaCodec::default();

    let addr = ballista_scheduler::standalone::new_standalone_scheduler()
        .await
        .expect("scheduler to be created");

    let host = "localhost".to_string();

    let scheduler_url = format!("http://{}:{}", host, addr.port());

    let scheduler = loop {
        match SchedulerGrpcClient::connect(scheduler_url.clone()).await {
            Err(_) => {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                log::info!("Attempting to connect to test scheduler...");
            }
            Ok(scheduler) => break scheduler,
        }
    };

    ballista_executor::new_standalone_executor(
        scheduler,
        config.default_standalone_parallelism(),
        default_codec,
    )
    .await
    .expect("executor to be created");

    log::info!("test scheduler created at: {}:{}", host, addr.port());

    (host, addr.port())
}

#[ctor::ctor]
fn init() {
    // Enable RUST_LOG logging configuration for test
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .parse_filters("ballista=debug,ballista_scheduler-rs=debug,ballista_executor=debug,datafusion=debug")
        .is_test(true)
        .try_init();
}
