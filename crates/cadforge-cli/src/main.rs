//! `cadforge-cli`: CadForge drawings from the command line.
use cadforge_cli::{mcp, Session};
use std::process::ExitCode;

const USAGE: &str = "cadforge-cli: CadForge drawings from the command line

Usage:
  cadforge-cli info <drawing>                        report what a drawing holds (JSON)
  cadforge-cli convert <in> <out>                    convert between .dwg, .dxf and .json, or plot to .svg / .pdf
  cadforge-cli run [<drawing>] -s <script.scr> [-o <out>]
  cadforge-cli run [<drawing>] -c <line> [-c <line>...] [-o <out>]
                                                     run command-line input (as a SCRIPT types it) and save
  cadforge-cli commands [<word>]                     list the commands (JSON)
  cadforge-cli mcp                                   serve the Model Context Protocol on stdin / stdout

In a script, each line is typed at the command line: a command name, then the answers to its prompts
(10,20 for a point, a number, an option); an empty line is Enter. Lines starting with ; are comments.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("cadforge-cli: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &[String]) -> Result<(), String> {
    let Some(cmd) = args.first() else {
        println!("{USAGE}");
        return Ok(());
    };
    let rest = &args[1..];
    match cmd.as_str() {
        "-h" | "--help" | "help" => println!("{USAGE}"),
        "-V" | "--version" => println!("cadforge-cli {}", env!("CARGO_PKG_VERSION")),
        "info" => {
            let path = rest.first().ok_or("info needs a drawing file")?;
            let mut s = Session::new();
            s.open(path)?;
            println!("{}", serde_json::to_string_pretty(&s.info()).unwrap_or_default());
        }
        "convert" => {
            let (Some(inp), Some(out)) = (rest.first(), rest.get(1)) else { return Err("convert needs an input and an output file".into()) };
            let mut s = Session::new();
            s.open(inp)?;
            let n = s.save(out)?;
            eprintln!("Wrote {out} ({n} bytes).");
        }
        "run" => {
            let mut s = Session::new();
            let mut lines: Vec<String> = vec![];
            let mut out: Option<String> = None;
            let mut i = 0;
            while i < rest.len() {
                let a = &rest[i];
                let next = || rest.get(i + 1).cloned().ok_or_else(|| format!("{a} needs a value"));
                match a.as_str() {
                    "-s" | "--script" => {
                        let p = next()?;
                        let text = std::fs::read_to_string(&p).map_err(|e| format!("{p}: {e}"))?;
                        lines.extend(text.lines().map(str::to_string));
                        i += 1;
                    }
                    "-c" | "--command" => {
                        lines.push(next()?);
                        i += 1;
                    }
                    "-o" | "--out" => {
                        out = Some(next()?);
                        i += 1;
                    }
                    p if !p.starts_with('-') => s.open(p)?,
                    other => return Err(format!("unknown option {other}")),
                }
                i += 1;
            }
            let r = s.run(&lines);
            if let Some(log) = r["log"].as_str().filter(|l| !l.is_empty()) {
                eprintln!("{log}");
            }
            if let Some(w) = r["waiting_for"].as_str() {
                eprintln!("(the last command is still waiting: {w})");
            }
            if let Some(o) = out {
                let n = s.save(&o)?;
                eprintln!("Wrote {o} ({n} bytes).");
            }
            if r["errors"].as_array().is_some_and(|e| !e.is_empty()) {
                return Err("the script reported errors".into());
            }
        }
        "commands" => {
            let s = Session::new();
            println!("{}", serde_json::to_string_pretty(&s.commands(rest.first().map(String::as_str))).unwrap_or_default());
        }
        "mcp" => {
            let stdin = std::io::stdin();
            mcp::serve(stdin.lock(), std::io::stdout().lock()).map_err(|e| e.to_string())?;
        }
        other => return Err(format!("unknown command {other}; try cadforge-cli --help")),
    }
    Ok(())
}
