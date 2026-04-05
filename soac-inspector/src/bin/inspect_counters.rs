use soac_inspector::CounterDumpFile;
use std::path::PathBuf;

struct Args {
    path: PathBuf,
}

fn parse_args() -> Result<Args, String> {
    let mut positionals = Vec::new();
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            _ if arg.starts_with('-') => {
                return Err(format!("unknown option: {arg}"));
            }
            _ => positionals.push(arg),
        }
    }
    if positionals.len() != 1 {
        return Err("expected <counter-dump-file>".to_string());
    }
    Ok(Args {
        path: PathBuf::from(&positionals[0]),
    })
}

fn print_usage() {
    eprintln!("usage: inspect_counters <counter-dump-file>");
}

fn main() -> Result<(), String> {
    let args = parse_args().inspect_err(|_| print_usage())?;
    let dump = CounterDumpFile::open(args.path.as_path())?;
    let records = dump.records()?;
    for (record_index, record) in records.iter().enumerate() {
        println!(
            "record={} module={} package={} rows={}",
            record_index,
            record.module_name()?,
            record.package_name()?.unwrap_or("-"),
            record.row_count()
        );
        for row_index in 0..record.row_count() {
            let row = record.row(row_index)?;
            println!(
                "  counter={} scope={} kind={} site={} function_id={} function={} block={} value={}",
                row.counter_id,
                row.scope,
                row.kind,
                row.site_kind,
                row.function_id
                    .map(|function_id| function_id.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                row.function_qualname.unwrap_or("-"),
                row.block_label.unwrap_or("-"),
                row.value,
            );
        }
    }
    Ok(())
}
