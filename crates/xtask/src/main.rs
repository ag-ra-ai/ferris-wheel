//! Project tasks. Run from anywhere in the repo:
//!
//! ```text
//! cargo xtask web <example> [--release] [--no-serve] [--port <port>]
//! ```
//!
//! `web` builds an example for the browser and serves it on localhost. The
//! JavaScript glue is generated with `wasm-bindgen-cli-support` as a library,
//! pinned to the same version as the `wasm-bindgen` crate, so no separately
//! installed tool can drift out of sync.

use std::error::Error;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

const USAGE: &str = "\
usage: cargo xtask web <example> [--release] [--no-serve] [--port <port>]

  Builds an example for the browser into target/web/<example>/ and serves it
  at http://127.0.0.1:<port>/ (default port 8000).

  --release   optimized build (smaller and faster; slower to compile)
  --no-serve  build only
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("web") => web(&args[1..]),
        _ => Err(USAGE.into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

struct WebArgs {
    example: String,
    release: bool,
    serve: bool,
    port: u16,
}

fn parse_web_args(args: &[String]) -> Result<WebArgs> {
    let mut parsed = WebArgs {
        example: String::new(),
        release: false,
        serve: true,
        port: 8000,
    };
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--release" => parsed.release = true,
            "--no-serve" => parsed.serve = false,
            "--port" => {
                let port = args.next().ok_or("--port needs a value")?;
                parsed.port = port.parse().map_err(|_| format!("bad port: {port}"))?;
            }
            flag if flag.starts_with('-') => {
                return Err(format!("unknown flag {flag}\n\n{USAGE}").into());
            }
            name if parsed.example.is_empty() => parsed.example = name.to_string(),
            extra => return Err(format!("unexpected argument {extra}\n\n{USAGE}").into()),
        }
    }
    Ok(parsed)
}

fn web(args: &[String]) -> Result<()> {
    let root = workspace_root();
    let examples = list_examples(&root.join("crates/ferris_wheel/examples"))?;
    let args = parse_web_args(args)?;
    if !examples.contains(&args.example) {
        return Err(format!("{USAGE}\nexamples: {}", examples.join(", ")).into());
    }
    let example = args.example.as_str();

    // 1. Compile the example to WebAssembly.
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let mut build = Command::new(cargo);
    build.current_dir(&root).args([
        "build",
        "--locked",
        "--package",
        "ferris_wheel",
        "--target",
        "wasm32-unknown-unknown",
        "--example",
        example,
    ]);
    if args.release {
        build.arg("--release");
    }
    if !build.status()?.success() {
        return Err("cargo build failed".into());
    }

    // 2. Generate the JavaScript glue that loads it.
    let target_dir = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target"));
    let profile = if args.release { "release" } else { "debug" };
    let wasm = target_dir
        .join("wasm32-unknown-unknown")
        .join(profile)
        .join("examples")
        .join(format!("{example}.wasm"));
    let out_dir = target_dir.join("web").join(example);
    if out_dir.exists() {
        fs::remove_dir_all(&out_dir)?;
    }
    fs::create_dir_all(&out_dir)?;
    wasm_bindgen_cli_support::Bindgen::new()
        .input_path(&wasm)
        .web(true)?
        .typescript(false)
        // Let `init()` find `<example>_bg.wasm` next to the JS file, as the
        // wasm-bindgen CLI does (the library's default leaves this out).
        .omit_default_module_path(false)
        .debug(!args.release)
        .generate(&out_dir)?;

    // 3. A page that loads it.
    fs::write(out_dir.join("index.html"), index_html(example))?;
    println!("built {}", out_dir.display());

    if args.serve {
        serve(&out_dir, args.port)?;
    }
    Ok(())
}

fn workspace_root() -> PathBuf {
    // This crate lives in <root>/crates/xtask.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("xtask is two levels below the workspace root")
        .to_path_buf()
}

/// Example names: the `.rs` files directly inside `dir`.
fn list_examples(dir: &Path) -> Result<Vec<String>> {
    let mut names = Vec::new();
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().is_some_and(|e| e == "rs")
            && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
        {
            names.push(stem.to_string());
        }
    }
    names.sort();
    Ok(names)
}

fn index_html(example: &str) -> String {
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{example}</title>
<style>
  /* The engine fills the canvas and letterboxes the game view inside it. */
  html, body {{ margin: 0; height: 100%; background: #000; overflow: hidden; }}
  canvas {{ display: block; width: 100%; height: 100%; outline: none; }}
</style>
</head>
<body>
<script type="module">
  import init from "./{example}.js";
  // Runs the example's main(); errors also appear in the browser console.
  init().catch((e) => console.error(e));
</script>
</body>
</html>
"#
    )
}

/// Serves `dir` over HTTP on localhost until stopped with Ctrl+C.
/// Deliberately minimal: GET only, one request at a time, local use only.
fn serve(dir: &Path, port: u16) -> Result<()> {
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    println!("serving at http://127.0.0.1:{port}/ (Ctrl+C to stop)");
    for stream in listener.incoming() {
        if let Err(e) = stream.map_err(Box::from).and_then(|s| handle(s, dir)) {
            eprintln!("request failed: {e}");
        }
    }
    Ok(())
}

fn handle(stream: TcpStream, dir: &Path) -> Result<()> {
    let mut reader = BufReader::new(&stream);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    // Skip the headers; we don't need them.
    let mut header = String::new();
    while reader.read_line(&mut header)? > 2 {
        header.clear();
    }

    let mut parts = request_line.split_whitespace();
    let (method, target) = (parts.next().unwrap_or(""), parts.next().unwrap_or("/"));
    let path = target.split(['?', '#']).next().unwrap_or("/");
    let path = if path == "/" { "/index.html" } else { path };

    let file = resolve(dir, path);
    let (status, content_type, body) =
        match (method, file.and_then(|f| fs::read(&f).ok().map(|b| (f, b)))) {
            ("GET", Some((file, body))) => ("200 OK", content_type(&file), body),
            ("GET", None) => ("404 Not Found", "text/plain", b"not found".to_vec()),
            _ => ("405 Method Not Allowed", "text/plain", b"GET only".to_vec()),
        };
    let mut stream = stream;
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\
         Cache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(&body)?;
    Ok(())
}

/// Maps a URL path to a file inside `dir`, refusing anything that could
/// reach outside it.
fn resolve(dir: &Path, url_path: &str) -> Option<PathBuf> {
    let relative = url_path.strip_prefix('/')?;
    let safe = relative
        .split('/')
        .all(|part| !part.is_empty() && part != "." && part != ".." && !part.contains('\\'));
    safe.then(|| dir.join(relative))
}

fn content_type(file: &Path) -> &'static str {
    match file.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript",
        // Browsers only stream-compile wasm served with this exact type.
        Some("wasm") => "application/wasm",
        Some("png") => "image/png",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_flags() {
        let a = parse_web_args(&args(&["bounce", "--release", "--port", "9000"])).unwrap();
        assert_eq!(
            (a.example.as_str(), a.release, a.serve, a.port),
            ("bounce", true, true, 9000)
        );
        let a = parse_web_args(&args(&["--no-serve", "clear"])).unwrap();
        assert_eq!((a.example.as_str(), a.serve), ("clear", false));
        assert!(parse_web_args(&args(&["clear", "--bogus"])).is_err());
        assert!(parse_web_args(&args(&["clear", "--port", "x"])).is_err());
    }

    #[test]
    fn resolve_stays_inside_dir() {
        let dir = Path::new("/srv/web");
        assert_eq!(resolve(dir, "/a.js"), Some(dir.join("a.js")));
        assert_eq!(resolve(dir, "/sub/a.js"), Some(dir.join("sub/a.js")));
        assert_eq!(resolve(dir, "/../secret"), None);
        assert_eq!(resolve(dir, "/a/../../secret"), None);
        assert_eq!(resolve(dir, "//etc/passwd"), None);
        assert_eq!(resolve(dir, "/a\\..\\b"), None);
        assert_eq!(resolve(dir, "relative"), None);
    }

    #[test]
    fn wasm_gets_streaming_content_type() {
        assert_eq!(content_type(Path::new("x_bg.wasm")), "application/wasm");
        assert_eq!(content_type(Path::new("x.js")), "text/javascript");
    }

    #[test]
    fn lists_only_rs_files() {
        let names = list_examples(&workspace_root().join("crates/ferris_wheel/examples")).unwrap();
        assert!(names.contains(&"bounce".to_string()));
        assert!(!names.iter().any(|n| n == "support" || n == "assets"));
    }
}
