#[macro_use]
extern crate lazy_static;

extern crate reqwest;

use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

mod utils;
use utils::*;

lazy_static! {
    static ref CARGO_WEB: PathBuf = get_var( "CARGO_WEB" ).into();
    static ref REPOSITORY_ROOT: PathBuf = get_var( "REPOSITORY_ROOT" ).into();
    static ref NODEJS: PathBuf = {
        use utils::find_cmd;
        find_cmd( &[ "nodejs", "node", "nodejs.exe", "node.exe" ] ).expect( "nodejs not found" ).into()
    };
}

fn each_target< F: FnMut( &'static str ) >( mut callback: F ) {
    callback( "asmjs-unknown-emscripten" );
    callback( "wasm32-unknown-emscripten" );
    if *IS_NIGHTLY {
        callback( "wasm32-unknown-unknown" );
    }
}

fn main() {
    eprintln!( "Running on nightly: {}", *IS_NIGHTLY );

    cd( &*REPOSITORY_ROOT );

    for name in &[
        "rlib",
        "dev-depends-on-dylib",
        "staticlib",
        "workspace",
        "conflicting-versions",
        "requires-old-cargo-web",
        "req-future-cargo-web-disabled-dep",
        "req-future-cargo-web-dev-dep",
        "req-future-cargo-web-dep-dev-dep",
        "req-future-cargo-web-build-dep",
        "compiling-under-cargo-web-env-var",
        "depends-on-default-target-invalid"
    ] {
        in_directory( &format!( "test-crates/{}", name ), || {
            each_target( |target| {
                run( &*CARGO_WEB, &["build", "--target", target] ).assert_success();
            });
        });
    }

    for name in &[
        "crate-with-tests",
        "crate-with-integration-tests"
    ] {
        in_directory( &format!( "test-crates/{}", name ), || {
            each_target( |target| {
                for _ in 0..2 { // Make sure they can be ran multiple times.
                    run( &*CARGO_WEB, &["test", "--nodejs", "--target", target] ).assert_success();
                }
            });
        });
    }

    in_directory( "test-crates/req-future-cargo-web-target-dep", || {
        run( &*CARGO_WEB, &["build", "--target", "asmjs-unknown-emscripten"] ).assert_success();
        run( &*CARGO_WEB, &["build", "--target", "wasm32-unknown-emscripten"] ).assert_failure();
    });

    for name in &[
        "requires-future-cargo-web",
        "req-future-cargo-web-dep",
        "req-future-cargo-web-dep-dep",
        "req-future-cargo-web-dep-and-dev-dep"
    ] {
        in_directory( &format!( "test-crates/{}", name ), || {
            each_target( |target| {
                run( &*CARGO_WEB, &["build", "--target", target] ).assert_failure();
            });
        });
    }

    in_directory( "test-crates/req-future-cargo-web-dev-dep", || {
        each_target( |target| {
            run( &*CARGO_WEB, &["test", "--target", target] ).assert_failure();
        });
    });

    in_directory( "test-crates/link-args-per-target", || {
        // In Web.toml of the test crate we set a different `EXPORT_NAME` link-arg
        // for each target and we check if it's actually used by Emscripten.
        run( &*CARGO_WEB, &["build", "--target", "asmjs-unknown-emscripten"] ).assert_success();
        assert_file_contains( "target/asmjs-unknown-emscripten/debug/link-args-per-target.js", "CustomExportNameAsmJs" );

        run( &*CARGO_WEB, &["build", "--target", "wasm32-unknown-emscripten"] ).assert_success();
        assert_file_contains( "target/wasm32-unknown-emscripten/debug/link-args-per-target.js", "CustomExportNameWasm" );

        if *IS_NIGHTLY {
            // This has no flags set, but still should compile.
            run( &*CARGO_WEB, &["build", "--target", "wasm32-unknown-unknown"] ).assert_success();
        }
    });

    in_directory( "test-crates/link-args-for-emscripten", || {
        // Here we set the same flag for both targets in a single target section.
        run( &*CARGO_WEB, &["build", "--target", "asmjs-unknown-emscripten"] ).assert_success();
        assert_file_contains( "target/asmjs-unknown-emscripten/debug/link-args-for-emscripten.js", "CustomExportNameEmscripten" );

        run( &*CARGO_WEB, &["build", "--target", "wasm32-unknown-emscripten"] ).assert_success();
        assert_file_contains( "target/wasm32-unknown-emscripten/debug/link-args-for-emscripten.js", "CustomExportNameEmscripten" );

        if *IS_NIGHTLY {
            // This has no flags set, but still should compile.
            run( &*CARGO_WEB, &["build", "--target", "wasm32-unknown-unknown"] ).assert_success();
        }
    });

    in_directory( "test-crates/prepend-js", || {
        each_target( |target| {
            run( &*CARGO_WEB, &["build", "--target", target] ).assert_success();
            // TODO: We should run cargo-web with `--message-format=json` and grab this path automatically.
            let build_dir = if target == "wasm32-unknown-unknown" { "release" } else { "debug" };
            assert_file_contains( &format!( "target/{}/{}/prepend-js.js", target, build_dir ), "alert('THIS IS A TEST');" );
        });
    });

    in_directory( "test-crates/depends-on-prepend-js-two-targets" , || {
        run( &*CARGO_WEB, &["build", "--target", "asmjs-unknown-emscripten"] ).assert_success();
        assert_file_contains( &format!( "target/asmjs-unknown-emscripten/debug/depends-on-prepend-js-two-targets.js" ), "alert('THIS IS A TEST');" );

        run( &*CARGO_WEB, &["build", "--target", "wasm32-unknown-emscripten"] ).assert_success();
        assert_file_contains( &format!( "target/wasm32-unknown-emscripten/debug/depends-on-prepend-js-two-targets.js" ), "alert('THIS IS A TEST');" );
    });

    in_directory( "test-crates/default-target-asmjs-unknown-emscripten", || {
        run( &*CARGO_WEB, &["build"] ).assert_success();
        assert_file_exists( "target/asmjs-unknown-emscripten/debug/default-target-asmjs-unknown-emscripten.js" );
        run( &*CARGO_WEB, &["test"] ).assert_success();
        run( &*CARGO_WEB, &["deploy"] ).assert_success();
    });

    in_directory( "test-crates/default-target-wasm32-unknown-emscripten", || {
        run( &*CARGO_WEB, &["build"] ).assert_success();
        assert_file_exists( "target/wasm32-unknown-emscripten/debug/default-target-wasm32-unknown-emscripten.js" );
        run( &*CARGO_WEB, &["test"] ).assert_success();
        run( &*CARGO_WEB, &["deploy"] ).assert_success();
    });

    in_directory( "test-crates/default-target-invalid", || {
        run( &*CARGO_WEB, &["build"] ).assert_failure();
        run( &*CARGO_WEB, &["test"] ).assert_failure();
        run( &*CARGO_WEB, &["deploy"] ).assert_failure();
    });

    if *IS_NIGHTLY {
        in_directory( "test-crates/native-webasm", || {
            run( &*CARGO_WEB, &["build", "--target", "wasm32-unknown-unknown"] ).assert_success();
            run( &*NODEJS, &["run.js"] ).assert_success();
        });

        in_directory( "test-crates/cdylib", || {
            run( &*CARGO_WEB, &["build", "--target", "wasm32-unknown-unknown"] ).assert_success();
            run( &*CARGO_WEB, &["deploy", "--target", "wasm32-unknown-unknown"] ).assert_success();
            run( &*NODEJS, &["target/wasm32-unknown-unknown/release/cdylib.js"] ).assert_success();
        });

        in_directory( "test-crates/default-target-wasm32-unknown-unknown", || {
            run( &*CARGO_WEB, &["build"] ).assert_success();
            assert_file_exists( "target/wasm32-unknown-unknown/release/default-target-wasm32-unknown-unknown.js" );
            run( &*CARGO_WEB, &["deploy"] ).assert_success();
        });

        in_directory( "test-crates/prepend-js-includable-only-once", || {
            run( &*CARGO_WEB, &["build", "--release", "--target", "wasm32-unknown-unknown"] ).assert_success();
            run( &*NODEJS, &["target/wasm32-unknown-unknown/release/prepend-js-includable-only-once.js"] ).assert_success();
        });
    }

    in_directory( "test-crates/static-files", || {
        use std::str::FromStr;
        use reqwest::header::ContentType;
        use reqwest::StatusCode;
        use reqwest::mime::Mime;

        run( &*CARGO_WEB, &["build"] ).assert_success();
        let _child = run_in_the_background( &*CARGO_WEB, &["start"] );
        let start = Instant::now();
        let mut response = None;
        while start.elapsed() < Duration::from_secs( 10 ) && response.is_none() {
            thread::sleep( Duration::from_millis( 100 ) );
            response = reqwest::get( "http://localhost:8000" ).ok();
        }

        let response = response.unwrap();
        assert_eq!( response.status(), StatusCode::Ok );
        assert_eq!( *response.headers().get::< ContentType >().unwrap(), ContentType::html() );

        let mut response = reqwest::get( "http://localhost:8000/subdirectory/dummy.json" ).unwrap();
        assert_eq!( response.status(), StatusCode::Ok );
        assert_eq!( *response.headers().get::< ContentType >().unwrap(), ContentType::json() );
        assert_eq!( response.text().unwrap(), "{}" );

        let mut response = reqwest::get( "http://localhost:8000/static-files.js" ).unwrap();
        assert_eq!( response.status(), StatusCode::Ok );
        assert_eq!( *response.headers().get::< ContentType >().unwrap(), ContentType( Mime::from_str( "application/javascript" ).unwrap() ) );
        assert_eq!( response.text().unwrap(), read_to_string( "target/asmjs-unknown-emscripten/debug/static-files.js" ) );

        // TODO: Move this to its own test?
        let mut response = reqwest::get( "http://localhost:8000/__cargo-web__/build_hash" ).unwrap();
        assert_eq!( response.status(), StatusCode::Ok );
        let build_hash = response.text().unwrap();

        let mut response = reqwest::get( "http://localhost:8000/__cargo-web__/build_hash" ).unwrap();
        assert_eq!( response.status(), StatusCode::Ok );
        assert_eq!( response.text().unwrap(), build_hash ); // Hash didn't change.

        touch_file( "src/main.rs" );

        let start = Instant::now();
        let mut found = false;
        while start.elapsed() < Duration::from_secs( 10 ) && !found {
            thread::sleep( Duration::from_millis( 100 ) );
            let mut response = reqwest::get( "http://localhost:8000" ).unwrap();
            assert_eq!( response.status(), StatusCode::Ok );

            let new_build_hash = response.text().unwrap();
            found = new_build_hash != build_hash;
        }

        assert!( found, "Touching a source file didn't change the build hash!" );
    });

    in_directory( "test-crates/virtual-manifest", || {
        each_target( |target| {
            run( &*CARGO_WEB, &["build"] ).assert_failure();
            run( &*CARGO_WEB, &["build", "-p", "child"] ).assert_success();

            run( &*CARGO_WEB, &["test"] ).assert_failure();
            run( &*CARGO_WEB, &["test", "-p", "child"] ).assert_success();

            run( &*CARGO_WEB, &["deploy"] ).assert_failure();
            run( &*CARGO_WEB, &["deploy", "-p", "child"] ).assert_success();

            assert_file_missing( "child/target/deploy" );
            assert_file_exists( "target/deploy" );
        });
    });

    in_directory( "test-crates/req-future-cargo-web-cfg-dep", || {
        run( &*CARGO_WEB, &["build", "--target", "wasm32-unknown-unknown"] ).assert_success();
        run( &*CARGO_WEB, &["build", "--target", "wasm32-unknown-emscripten"] ).assert_failure();
    });

    in_directory( "test-crates/req-future-cargo-web-cfg-not-dep", || {
        run( &*CARGO_WEB, &["build", "--target", "wasm32-unknown-unknown"] ).assert_failure();
        run( &*CARGO_WEB, &["build", "--target", "wasm32-unknown-emscripten"] ).assert_success();
    });

    in_directory( "test-crates/failing-test", || {
        each_target( |target| {
            run( &*CARGO_WEB, &["test", "--target", target, "--nodejs"] ).assert_failure();
        });
    });

    in_directory( "test-crates/failing-integration-test", || {
        each_target( |target| {
            run( &*CARGO_WEB, &["test", "--target", target, "--nodejs"] ).assert_failure();
        });
    });

    if *IS_NIGHTLY {
        in_directory( "test-crates/failing-integration-test-crate-types", || {
            run( &*CARGO_WEB, &["test", "--target", "wasm32-unknown-unknown", "--nodejs"] ).assert_failure();
        });
    }
}
