extern crate bindgen;
extern crate cc;
extern crate walkdir;

use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

use walkdir::WalkDir;

const CPAL_ASIO_DIR: &str = "CPAL_ASIO_DIR";

const ASIO_HEADER: &str = "asio.h";
const ASIO_SYS_HEADER: &str = "asiosys.h";
const ASIO_DRIVERS_HEADER: &str = "asiodrivers.h";

/// Where `cargo run -p xtask -- asio-sdk` (Cantus) unpacks the SDK, under `%LOCALAPPDATA%`.
/// CANTUS PATCH: a known place to look when `CPAL_ASIO_DIR` is not set.
const CANTUS_SDK_CACHE: &str = r"cantus-build-tools\asiosdk-2.3.4\ASIOSDK";
/// Set to `1` to fail the build instead of building without the SDK (release builds).
const REQUIRE_SDK: &str = "ASIO_SYS_REQUIRE_SDK";

fn main() {
    // CANTUS PATCH: declare the cfg the library reads (`SDK_LINKED`).
    println!("cargo::rustc-check-cfg=cfg(asio_sdk)");
    // ASIO is Windows-only. Skip build on non-Windows platforms and on docs.rs.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows")
        || std::env::var("DOCS_RS").is_ok()
    {
        let out_dir = PathBuf::from(env::var("OUT_DIR").expect("bad path"));
        create_stub_bindings(&out_dir);
        return;
    }

    let is_msvc = std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");

    println!("cargo:rerun-if-env-changed={}", CPAL_ASIO_DIR);
    println!("cargo:rerun-if-env-changed=LIBCLANG_PATH");
    println!("cargo:rerun-if-env-changed={REQUIRE_SDK}");
    println!("cargo:rerun-if-changed=build.rs");
    // Directory where bindings and library are created
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("bad path"));

    // CANTUS PATCH: the SDK and libclang are looked for, never downloaded by the build
    // (a proxy that answers with its own page broke the download). Without them the crate
    // builds stub bindings — no driver ever loads, `SDK_LINKED` is false — and says so,
    // unless `ASIO_SYS_REQUIRE_SDK=1` asks for a hard error (release builds).
    let sdk = find_asio_dir();
    let clang = find_libclang();
    let (cpal_asio_dir, clang) = match (sdk, clang) {
        (Some(sdk), Some(clang)) => (sdk, clang),
        (sdk, clang) => {
            let mut why = Vec::new();
            if sdk.is_none() {
                why.push(format!(
                    "the Steinberg ASIO SDK was not found ({CPAL_ASIO_DIR} is not set and \
                     %LOCALAPPDATA%\\{CANTUS_SDK_CACHE} does not exist)"
                ));
            }
            if clang.is_none() {
                why.push(
                    r"LLVM's libclang.dll was not found (LIBCLANG_PATH, C:\Program Files\LLVM\bin)"
                        .to_string(),
                );
            }
            let why = why.join("; ");
            if env::var(REQUIRE_SDK).is_ok_and(|v| v == "1") {
                panic!("ASIO is required for this build, but {why}");
            }
            println!("cargo:warning=building WITHOUT ASIO: {why}.");
            println!(
                "cargo:warning=run `cargo run -p xtask -- asio-sdk` to fetch the SDK (and install LLVM: winget install LLVM.LLVM), then build again."
            );
            if let Some(root) = cache_root() {
                // Rebuild by itself once the SDK appears there.
                if root.is_dir() {
                    println!("cargo:rerun-if-changed={}", root.display());
                }
            }
            create_stub_bindings(&out_dir);
            return;
        }
    };
    println!("cargo:rerun-if-changed={}", cpal_asio_dir.display());
    // bindgen loads libclang at run time from LIBCLANG_PATH.
    env::set_var("LIBCLANG_PATH", &clang);

    // Check if library exists,
    // if it doesn't create it
    let mut lib_path = out_dir.clone();
    lib_path.push("libasio.a");
    if !lib_path.exists() {
        if is_msvc {
            invoke_vcvars_if_not_set();
        }
        create_lib(&cpal_asio_dir);
    }

    // Print out links to needed libraries
    println!("cargo:rustc-link-lib=dylib=advapi32");
    println!("cargo:rustc-link-lib=dylib=ole32");
    println!("cargo:rustc-link-lib=dylib=user32");
    println!("cargo:rustc-link-search={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=asio");
    println!("cargo:rustc-cfg=asio");
    println!("cargo:rustc-cfg=asio_sdk");
    // Dependents that link this crate directly read `DEP_ASIO_SDK`.
    println!("cargo:sdk=1");

    // Check if bindings exist
    // If they don't create them
    let mut binding_path = out_dir.clone();
    binding_path.push("asio_bindings.rs");
    if !binding_path.exists() {
        if is_msvc {
            invoke_vcvars_if_not_set();
        }
        create_bindings(&cpal_asio_dir);
    }
}

/// `%LOCALAPPDATA%\cantus-build-tools` (CANTUS PATCH).
fn cache_root() -> Option<PathBuf> {
    env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("cantus-build-tools"))
}

/// The SDK: `CPAL_ASIO_DIR` when set (it must hold the SDK), else Cantus's cache.
fn find_asio_dir() -> Option<PathBuf> {
    if let Ok(path) = env::var(CPAL_ASIO_DIR) {
        let dir = PathBuf::from(&path);
        if !dir.join("common").join("asio.h").is_file() {
            panic!("{CPAL_ASIO_DIR}={path} does not hold the ASIO SDK (common\\asio.h is missing)");
        }
        println!("CPAL_ASIO_DIR is set at {path}");
        return Some(dir);
    }
    let dir = env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join(CANTUS_SDK_CACHE))?;
    dir.join("common").join("asio.h").is_file().then(|| {
        println!("ASIO SDK found at {}", dir.display());
        dir
    })
}

/// The folder holding `libclang.dll`: `LIBCLANG_PATH`, else LLVM's default install.
fn find_libclang() -> Option<PathBuf> {
    let candidates = env::var_os("LIBCLANG_PATH")
        .map(PathBuf::from)
        .into_iter()
        .chain([PathBuf::from(r"C:\Program Files\LLVM\bin")]);
    for dir in candidates {
        if dir.join("libclang.dll").is_file() {
            return Some(dir);
        }
    }
    None
}

fn create_lib(cpal_asio_dir: &Path) {
    let mut cpp_paths: Vec<PathBuf> = Vec::new();
    let mut host_dir = cpal_asio_dir.to_path_buf();
    let mut pc_dir = cpal_asio_dir.to_path_buf();
    let mut common_dir = cpal_asio_dir.to_path_buf();
    host_dir.push("host");
    common_dir.push("common");
    pc_dir.push("host/pc");

    // Gathers cpp files from directories
    let walk_a_dir = |dir_to_walk, paths: &mut Vec<PathBuf>| {
        for entry in WalkDir::new(dir_to_walk).max_depth(1) {
            let entry = match entry {
                Err(e) => {
                    println!("error: {}", e);
                    continue;
                }
                Ok(entry) => entry,
            };
            match entry.path().extension().and_then(|s| s.to_str()) {
                None => continue,
                Some("cpp") => {
                    // Skip macos bindings
                    if entry.path().file_name().unwrap().to_str() == Some("asiodrvr.cpp") {
                        continue;
                    }
                    paths.push(entry.path().to_path_buf())
                }
                Some(_) => continue,
            };
        }
    };

    // Get all cpp files for building SDK library
    walk_a_dir(host_dir, &mut cpp_paths);
    walk_a_dir(pc_dir, &mut cpp_paths);
    walk_a_dir(common_dir, &mut cpp_paths);

    // build the asio lib
    cc::Build::new()
        .include(format!("{}/{}", cpal_asio_dir.display(), "host"))
        .include(format!("{}/{}", cpal_asio_dir.display(), "common"))
        .include(format!("{}/{}", cpal_asio_dir.display(), "host/pc"))
        .include("asio-link/helpers.hpp")
        .file("asio-link/helpers.cpp")
        .files(cpp_paths)
        .cpp(true)
        .compile("libasio.a");
}

/// Creates stub bindings for docs.rs
///
/// Since docs.rs builds in a sandboxed environment without network access
/// and cannot cross-compile Windows MSVC targets with C++ dependencies,
/// we generate minimal stub bindings that allow documentation to be built.
fn create_stub_bindings(out_dir: &Path) {
    let stub_content = include_str!("asio_stub_bindings.rs");
    let binding_path = out_dir.join("asio_bindings.rs");
    std::fs::write(&binding_path, stub_content).expect("Failed to write stub bindings");
}

fn create_bindings(cpal_asio_dir: &PathBuf) {
    let mut asio_header = None;
    let mut asio_sys_header = None;
    let mut asio_drivers_header = None;

    // Recursively walk given cpal dir to find required headers
    for entry in WalkDir::new(cpal_asio_dir) {
        let entry = match entry {
            Err(_) => continue,
            Ok(entry) => entry,
        };
        let file_name = match entry.path().file_name().and_then(|s| s.to_str()) {
            None => continue,
            Some(file_name) => file_name,
        };

        match file_name {
            ASIO_HEADER => asio_header = Some(entry.path().to_path_buf()),
            ASIO_SYS_HEADER => asio_sys_header = Some(entry.path().to_path_buf()),
            ASIO_DRIVERS_HEADER => asio_drivers_header = Some(entry.path().to_path_buf()),
            _ => (),
        }
    }

    macro_rules! header_or_panic {
        ($opt_header:expr, $FILE_NAME:expr) => {
            match $opt_header.as_ref() {
                None => {
                    panic!(
                        "Could not find {} in {}: {}",
                        $FILE_NAME,
                        CPAL_ASIO_DIR,
                        cpal_asio_dir.display()
                    );
                }
                Some(path) => path.to_str().expect("Could not convert path to str"),
            }
        };
    }

    // Only continue if found all headers that we need
    let asio_header = header_or_panic!(asio_header, ASIO_HEADER);
    let asio_sys_header = header_or_panic!(asio_sys_header, ASIO_SYS_HEADER);
    let asio_drivers_header = header_or_panic!(asio_drivers_header, ASIO_DRIVERS_HEADER);

    // The bindgen::Builder is the main entry point
    // to bindgen, and lets you build up options for
    // the resulting bindings.
    let bindings = bindgen::Builder::default()
        // The input header we would like to generate
        // bindings for.
        .header(asio_header)
        .header(asio_sys_header)
        .header(asio_drivers_header)
        .header("asio-link/helpers.hpp")
        .clang_arg("-x")
        .clang_arg("c++")
        .clang_arg("-std=c++14")
        .clang_arg(format!("-I{}/{}", cpal_asio_dir.display(), "host/pc"))
        .clang_arg(format!("-I{}/{}", cpal_asio_dir.display(), "host"))
        .clang_arg(format!("-I{}/{}", cpal_asio_dir.display(), "common"))
        // Need to whitelist to avoid binding tp c++ std::*
        .allowlist_type("AsioDrivers")
        .allowlist_type("AsioDriver")
        .allowlist_type("ASIOTime")
        .allowlist_type("ASIOTimeInfo")
        .allowlist_type("ASIODriverInfo")
        .allowlist_type("ASIOBufferInfo")
        .allowlist_type("ASIOCallbacks")
        .allowlist_type("ASIOSamples")
        .allowlist_type("ASIOSampleType")
        .allowlist_type("ASIOSampleRate")
        .allowlist_type("ASIOChannelInfo")
        .allowlist_type("AsioTimeInfoFlags")
        .allowlist_type("ASIOTimeCodeFlags")
        .allowlist_function("ASIOGetChannels")
        .allowlist_function("ASIOGetChannelInfo")
        .allowlist_function("ASIOGetBufferSize")
        .allowlist_function("ASIOGetLatencies")
        .allowlist_function("ASIOGetSamplePosition")
        .allowlist_function("ASIOOutputReady")
        .allowlist_function("get_sample_rate")
        .allowlist_function("set_sample_rate")
        .allowlist_function("can_sample_rate")
        .allowlist_function("ASIOInit")
        .allowlist_function("ASIOCreateBuffers")
        .allowlist_function("ASIOStart")
        .allowlist_function("ASIOStop")
        .allowlist_function("ASIODisposeBuffers")
        .allowlist_function("ASIOExit")
        .allowlist_function("load_asio_driver")
        .allowlist_function("remove_current_driver")
        .allowlist_function("get_driver_names")
        .bitfield_enum("AsioTimeInfoFlags")
        .bitfield_enum("ASIOTimeCodeFlags")
        // Finish the builder and generate the bindings.
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").expect("bad path"));

    bindings
        .write_to_file(out_path.join("asio_bindings.rs"))
        .expect("Couldn't write bindings!");
}

/// Invokes `vcvarsall.bat` to initialize the environment for building with MSVC
///
/// This function is only meant to be called when the host OS is Windows.
fn invoke_vcvars_if_not_set() {
    if vcvars_set() {
        return;
    }
    println!("VCINSTALLDIR is not set. Attempting to invoke vcvarsall.bat..");

    println!("Invoking vcvarsall.bat..");
    println!("Determining system architecture..");

    let arch_arg = determine_vcvarsall_bat_arch_arg();
    println!(
        "Host architecture is detected as {}.",
        std::env::consts::ARCH
    );
    println!("Architecture argument for vcvarsall.bat will be used as: {arch_arg}.");

    let vcvars_all_bat_path = search_vcvars_all_bat();

    println!(
        "Found vcvarsall.bat at {}. Initializing environment..",
        vcvars_all_bat_path.display()
    );

    // Invoke vcvarsall.bat
    let output = Command::new("cmd")
        .args([
            "/c",
            vcvars_all_bat_path.to_str().unwrap(),
            &arch_arg,
            "&&",
            "set",
        ])
        .output()
        .expect("Failed to execute command");

    for line in String::from_utf8_lossy(&output.stdout).lines() {
        // Filters the output of vcvarsall.bat to only include lines of the form "VARNAME=VALUE"
        let parts: Vec<&str> = line.splitn(2, '=').collect();
        if parts.len() == 2 {
            env::set_var(parts[0], parts[1]);
            println!("{}={}", parts[0], parts[1]);
        }
    }
}

/// Checks if vcvarsall.bat has been invoked
/// Assumes that it is very unlikely that the user would set `VCINSTALLDIR` manually
fn vcvars_set() -> bool {
    env::var("VCINSTALLDIR").is_ok()
}

/// Searches for vcvarsall.bat in the default installation directories
///
/// If it is not found, it will search for it in the Program Files directories
///
/// If it is still not found, it will panic.
fn search_vcvars_all_bat() -> PathBuf {
    if let Some(path) = guess_vcvars_all_bat() {
        return path;
    }

    // Define search paths for vcvarsall.bat based on architecture
    let paths = &[
        // Visual Studio 2022+
        "C:\\Program Files\\Microsoft Visual Studio\\",
        // <= Visual Studio 2019
        "C:\\Program Files (x86)\\Microsoft Visual Studio\\",
    ];

    // Search for vcvarsall.bat using walkdir
    println!("Searching for vcvarsall.bat in {paths:?}");

    let mut found = None;

    for path in paths.iter() {
        for entry in WalkDir::new(path)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| !e.file_type().is_dir())
        {
            if entry.path().ends_with("vcvarsall.bat") {
                found.replace(entry.path().to_path_buf());
            }
        }
    }

    match found {
        Some(path) => path,
        None => panic!(
            "Could not find vcvarsall.bat. Please install the latest version of Visual Studio."
        ),
    }
}

/// Guesses the location of vcvarsall.bat by searching it with certain heuristics.
///
/// It is meant to be executed before a top level search over Microsoft Visual Studio directories
/// to ensure faster execution in CI environments.
fn guess_vcvars_all_bat() -> Option<PathBuf> {
    /// Checks if a string is a year
    fn is_year(s: Option<&str>) -> Option<String> {
        let Some(s) = s else {
            return None;
        };

        if s.len() == 4 && s.chars().all(|c| c.is_ascii_digit()) {
            Some(s.to_string())
        } else {
            None
        }
    }

    /// Checks if a string is an edition of Visual Studio
    fn is_edition(s: Option<&str>) -> Option<String> {
        let Some(s) = s else {
            return None;
        };

        let editions = ["Enterprise", "Professional", "Community", "Express"];
        if editions.contains(&s) {
            Some(s.to_string())
        } else {
            None
        }
    }

    /// Constructs a path to vcvarsall.bat based on a base path
    fn construct_path(base: &Path) -> Option<PathBuf> {
        let mut constructed = base.to_path_buf();
        for entry in WalkDir::new(&constructed).max_depth(1) {
            let entry = match entry {
                Err(_) => continue,
                Ok(entry) => entry,
            };
            if let Some(year) = is_year(entry.path().file_name().and_then(|s| s.to_str())) {
                constructed = constructed.join(year);
                for entry in WalkDir::new(&constructed).max_depth(1) {
                    let entry = match entry {
                        Err(_) => continue,
                        Ok(entry) => entry,
                    };
                    if let Some(edition) =
                        is_edition(entry.path().file_name().and_then(|s| s.to_str()))
                    {
                        constructed = constructed
                            .join(edition)
                            .join("VC")
                            .join("Auxiliary")
                            .join("Build")
                            .join("vcvarsall.bat");

                        return Some(constructed);
                    }
                }
            }
        }
        None
    }

    let vs_2022_and_onwards_base = PathBuf::from("C:\\Program Files\\Microsoft Visual Studio\\");
    let vs_2019_and_2017_base = PathBuf::from("C:\\Program Files (x86)\\Microsoft Visual Studio\\");

    construct_path(&vs_2022_and_onwards_base).map_or_else(
        || construct_path(&vs_2019_and_2017_base).map_or_else(|| None, Some),
        Some,
    )
}

/// Determines the right argument to pass to `vcvarsall.bat` based on the host and target architectures.
///
/// Windows on ARM is not supporting 32 bit arm processors.
/// Because of this there is no native or cross compilation is supported for 32 bit arm processors.
fn determine_vcvarsall_bat_arch_arg() -> String {
    let host_architecture = std::env::consts::ARCH;
    let target_architecture = std::env::var("CARGO_CFG_TARGET_ARCH").expect("Target not set.");

    let arch_arg = if target_architecture == "x86_64" {
        if host_architecture == "x86" {
            // Arg for cross compilation from x86 to x64
            "x86_amd64"
        } else if host_architecture == "x86_64" {
            // Arg for native compilation from x64 to x64
            "amd64"
        } else if host_architecture == "aarch64" {
            // Arg for cross compilation from arm64 to amd64
            "arm64_amd64"
        } else {
            panic!("Unsupported host architecture {}", host_architecture);
        }
    } else if target_architecture == "x86" {
        if host_architecture == "x86" {
            // Arg for native compilation from x86 to x86
            "x86"
        } else if host_architecture == "x86_64" {
            // Arg for cross compilation from x64 to x86
            "amd64_x86"
        } else if host_architecture == "aarch64" {
            // Arg for cross compilation from arm64 to x86
            "arm64_x86"
        } else {
            panic!("Unsupported host architecture {}", host_architecture);
        }
    } else if target_architecture == "arm" {
        if host_architecture == "x86" {
            // Arg for cross compilation from x86 to arm
            "x86_arm"
        } else if host_architecture == "x86_64" {
            // Arg for cross compilation from x64 to arm
            "amd64_arm"
        } else if host_architecture == "aarch64" {
            // Arg for cross compilation from arm64 to arm
            "arm64_arm"
        } else {
            panic!("Unsupported host architecture {}", host_architecture);
        }
    } else if target_architecture == "aarch64" {
        if host_architecture == "x86" {
            // Arg for cross compilation from x86 to arm
            "x86_arm64"
        } else if host_architecture == "x86_64" {
            // Arg for cross compilation from x64 to arm
            "amd64_arm64"
        } else if host_architecture == "aarch64" {
            // Arg for native compilation from arm64 to arm64
            "arm64"
        } else {
            panic!("Unsupported host architecture {}", host_architecture);
        }
    } else {
        panic!("Unsupported target architecture.");
    };

    arch_arg.to_owned()
}
