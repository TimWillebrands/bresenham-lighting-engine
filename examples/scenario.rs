//! Exploration-loop CLI: build a named scenario, print or render its output.
//!
//! Usage:
//!
//! ```text
//! cargo run --example scenario -- --list
//! cargo run --example scenario -- --name single_light
//! cargo run --example scenario -- --name object_shadow --output-format png --out /tmp/s.png
//! ```
//!
//! The scenarios themselves are defined in
//! [`bresenham_lighting_engine::scenarios`] and shared with the regression
//! tests in `tests/scenarios.rs`.

use std::process::ExitCode;

use bresenham_lighting_engine::engine::LightingEngine;
use bresenham_lighting_engine::lighting::Color;
use bresenham_lighting_engine::scenarios::{self, Scenario};

#[derive(Debug, Clone, Copy)]
enum OutputFormat {
    Text,
    Png,
}

struct Args {
    list: bool,
    name: Option<String>,
    format: OutputFormat,
    out: Option<String>,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        list: false,
        name: None,
        format: OutputFormat::Text,
        out: None,
    };
    let mut iter = std::env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--list" => args.list = true,
            "--name" => {
                args.name =
                    Some(iter.next().ok_or_else(|| "missing value for --name".to_string())?)
            }
            "--output-format" => {
                let v = iter
                    .next()
                    .ok_or_else(|| "missing value for --output-format".to_string())?;
                args.format = match v.as_str() {
                    "text" => OutputFormat::Text,
                    "png" => OutputFormat::Png,
                    other => return Err(format!("unknown --output-format {:?}", other)),
                };
            }
            "--out" => {
                args.out =
                    Some(iter.next().ok_or_else(|| "missing value for --out".to_string())?)
            }
            "-h" | "--help" => {
                print_usage();
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument {:?}", other)),
        }
    }
    Ok(args)
}

fn print_usage() {
    eprintln!(
        "Usage: scenario --list\n       scenario --name <NAME> [--output-format text|png] [--out PATH]\n\n\
         Build one of the named scenarios in src/scenarios and print or render its primary light."
    );
}

fn print_list() {
    println!("scenarios:");
    for s in scenarios::SCENARIOS {
        println!("  {:20} {}", s.name, s.description);
    }
}

fn run_text(scenario: &Scenario) -> Result<(), String> {
    let mut engine = LightingEngine::default();
    let light_id = (scenario.build)(&mut engine);
    let text = engine
        .render_canvas_text(light_id)
        .ok_or_else(|| format!("scenario {:?} produced no light {}", scenario.name, light_id))?;
    println!("scenario: {}", scenario.name);
    print!("{}", text);
    Ok(())
}

fn run_png(scenario: &Scenario, out: &str) -> Result<(), String> {
    let mut engine = LightingEngine::default();
    let light_id = (scenario.build)(&mut engine);
    let canvas = engine
        .light_canvas(light_id)
        .ok_or_else(|| format!("scenario {:?} produced no light {}", scenario.name, light_id))?;
    let size = engine.light_canvas_size(light_id).unwrap();
    let mut img = image::ImageBuffer::new(size as u32, size as u32);
    for (i, &Color(r, g, b, _a)) in canvas.iter().enumerate() {
        let x = (i % size) as u32;
        let y = (i / size) as u32;
        img.put_pixel(x, y, image::Rgb([r, g, b]));
    }
    img.save(out).map_err(|e| format!("save {}: {}", out, e))?;
    println!("wrote {}", out);
    Ok(())
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {}", e);
            print_usage();
            return ExitCode::from(2);
        }
    };

    if args.list {
        print_list();
        return ExitCode::SUCCESS;
    }

    let name = match &args.name {
        Some(n) => n,
        None => {
            eprintln!("error: --name or --list required");
            print_usage();
            return ExitCode::from(2);
        }
    };

    let scenario = match scenarios::find(name) {
        Some(s) => s,
        None => {
            eprintln!("error: unknown scenario {:?}", name);
            print_list();
            return ExitCode::from(2);
        }
    };

    let result = match args.format {
        OutputFormat::Text => run_text(scenario),
        OutputFormat::Png => {
            let out = match &args.out {
                Some(p) => p,
                None => {
                    eprintln!("error: --out is required when --output-format=png");
                    return ExitCode::from(2);
                }
            };
            run_png(scenario, out)
        }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {}", e);
            ExitCode::from(1)
        }
    }
}
