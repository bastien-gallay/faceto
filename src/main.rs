//! faceto — a typed file → a visual workshop board you think through with an LLM.
//!
//!   faceto render [MODEL]            write board.svg + index.html next to MODEL
//!   faceto serve  [MODEL] [-p PORT]  serve the live board + comment sidecar
//!
//! MODEL defaults to ./model.json. Zero dependencies, offline.

mod json;
mod model;
mod render;
mod serve;

use std::path::Path;
use std::process::exit;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(String::as_str).unwrap_or("help");
    match cmd {
        "render" => {
            let model = args.get(2).map(String::as_str).unwrap_or("model.json");
            cmd_render(model);
        }
        "serve" => {
            let (model, port) = parse_serve(&args[2..]);
            if let Err(e) = serve::serve(Path::new(&model), port) {
                eprintln!("error: {e}");
                exit(1);
            }
        }
        "help" | "-h" | "--help" => print_help(),
        "version" | "-V" | "--version" => println!("faceto {}", env!("CARGO_PKG_VERSION")),
        other => {
            eprintln!("unknown command: {other}\n");
            print_help();
            exit(2);
        }
    }
}

fn cmd_render(model_path: &str) {
    let path = Path::new(model_path);
    let model = match model::load(path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("error: {e}");
            exit(1);
        }
    };
    let svg = render::render_svg(&model);
    let html = render::render_html(&svg, &model.title);
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    if let Err(e) = std::fs::write(dir.join("board.svg"), format!("{svg}\n")) {
        eprintln!("error writing board.svg: {e}");
        exit(1);
    }
    if let Err(e) = std::fs::write(dir.join("index.html"), html) {
        eprintln!("error writing index.html: {e}");
        exit(1);
    }
    println!(
        "rendered {} elements → {} + {}",
        model.elements.len(),
        dir.join("board.svg").display(),
        dir.join("index.html").display()
    );
}

fn parse_serve(args: &[String]) -> (String, u16) {
    let mut model = "model.json".to_string();
    let mut port: u16 = 8753;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-p" | "--port" => {
                if let Some(v) = args.get(i + 1).and_then(|s| s.parse().ok()) {
                    port = v;
                }
                i += 2;
            }
            other => {
                model = other.to_string();
                i += 1;
            }
        }
    }
    (model, port)
}

fn print_help() {
    println!(
        "faceto {} — a typed file → a visual workshop board you think through with an LLM\n\
         \n\
         USAGE:\n\
         \x20 faceto render [MODEL]             write board.svg + index.html next to MODEL\n\
         \x20 faceto serve  [MODEL] [-p PORT]   serve the live board + comment sidecar (default :8753)\n\
         \x20 faceto help | version\n\
         \n\
         MODEL defaults to ./model.json.",
        env!("CARGO_PKG_VERSION")
    );
}
