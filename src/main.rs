use std::{env, fs, path::Path, process::ExitCode};

const USAGE: &str = "Usage: pepper <parse|check|emit-ir> <file.pepper>\n       pepper build <file.pepper> -o <executable>";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
        println!(
            "Pepper compiler (LLVM backend)\n\n{USAGE}\n\n  parse     Print the syntax tree\n  check     Check syntax only\n  emit-ir   Validate the executable subset and print LLVM IR\n  build     Compile the executable subset to a native executable\n\nExecutable subset: typed functions, scalar values, local arrays and print.\nRUSTC may select the Rust compiler executable (default: rustc)."
        );
        return Ok(());
    }
    if args.len() == 1 && args[0] == "--version" {
        println!("pepper {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let building = args.first().is_some_and(|arg| arg == "build");
    if (building && (args.len() != 4 || args[2] != "-o"))
        || (!building
            && (args.len() != 2
                || (args[0] != "parse" && args[0] != "check" && args[0] != "emit-ir")))
    {
        return Err(format!("{USAGE}\nSee --help."));
    }
    let filename = Path::new(&args[1]);
    let source =
        fs::read_to_string(filename).map_err(|e| format!("{}: {e}", filename.display()))?;
    let module =
        pepper::parse(&source).map_err(|e| e.render(&filename.display().to_string(), &source))?;
    if args[0] == "parse" {
        println!("{module:#?}");
    } else if args[0] == "check" {
        println!("{}: syntax OK", filename.display());
    } else {
        let program = pepper::semantic::analyze(&module)
            .map_err(|e| e.render(&filename.display().to_string(), &source))?;
        let name = filename.to_string_lossy();
        if building {
            let output = Path::new(&args[3]);
            if fs::canonicalize(output).ok().as_ref()
                == Some(&fs::canonicalize(filename).map_err(|e| e.to_string())?)
            {
                return Err("the executable output must not overwrite the source file".into());
            }
            pepper::backend::build(&program, &name, output)?;
            println!("Built {}", output.display());
        } else {
            print!("{}", pepper::backend::emit_ir(&program, &name)?);
        }
    }
    Ok(())
}
