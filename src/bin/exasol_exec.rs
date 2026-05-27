use std::env;
use std::fs;

use std::time::Instant;

use exa_postgres_interface::config::{DEFAULT_TRANSPORT, ExasolConfig};
use exa_postgres_interface::exasol::{ExasolOutcome, ExasolSession};

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut dsn = String::new();
    let mut user = String::new();
    let mut password = String::new();
    let mut schema = String::new();
    let mut certificate_fingerprint = String::new();
    let mut validate_certificate = true;
    let mut sql = None;
    let mut file = None;
    let mut transport = DEFAULT_TRANSPORT.to_owned();
    let mut repeat: usize = 1;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--dsn" => dsn = required_value(&mut args, "--dsn")?,
            "--user" => user = required_value(&mut args, "--user")?,
            "--password" => password = required_value(&mut args, "--password")?,
            "--schema" => schema = required_value(&mut args, "--schema")?,
            "--transport" => transport = required_value(&mut args, "--transport")?,
            "--repeat" => repeat = required_value(&mut args, "--repeat")?.parse()?,
            "--fingerprint" => {
                certificate_fingerprint = required_value(&mut args, "--fingerprint")?
            }
            "--no-verify" => validate_certificate = false,
            "--sql" => sql = Some(required_value(&mut args, "--sql")?),
            "--file" => file = Some(required_value(&mut args, "--file")?),
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            other => return Err(format!("unknown argument: {other}").into()),
        }
    }

    if dsn.is_empty() || user.is_empty() || password.is_empty() {
        return Err("required: --dsn --user --password".into());
    }

    let sql = match (sql, file) {
        (Some(sql), None) => sql,
        (None, Some(path)) => fs::read_to_string(path)?,
        (Some(_), Some(_)) => return Err("pass either --sql or --file, not both".into()),
        (None, None) => return Err("required: --sql or --file".into()),
    };

    let config = ExasolConfig {
        dsn,
        encryption: true,
        certificate_fingerprint,
        validate_certificate,
        pass_client_credentials: true,
        schema,
        transport,
    };

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    if repeat > 1 {
        return rt.block_on(run_benchmark(&config, &user, &password, &sql, repeat));
    }

    rt.block_on(async {
        let mut session = ExasolSession::connect(&config, &user, &password).await?;
        match session.execute(&sql).await? {
            ExasolOutcome::RowCount(count) => {
                println!("row_count={count}");
            }
            ExasolOutcome::ArrowRows(batches) => {
                if let Some(first) = batches.first() {
                    let schema = first.schema();
                    let headers: Vec<&str> =
                        schema.fields().iter().map(|f| f.name().as_str()).collect();
                    println!("{}", headers.join("\t"));
                }
                for batch in &batches {
                    for row_idx in 0..batch.num_rows() {
                        let row: Vec<String> = (0..batch.num_columns())
                            .map(|col_idx| {
                                let col = batch.column(col_idx);
                                if col.is_null(row_idx) {
                                    String::new()
                                } else {
                                    arrow::util::display::array_value_to_string(col, row_idx)
                                        .unwrap_or_default()
                                }
                            })
                            .collect();
                        println!("{}", row.join("\t"));
                    }
                }
            }
            ExasolOutcome::TypedRows { columns, rows } => {
                let headers: Vec<&str> = columns.iter().map(|c| c.name.as_str()).collect();
                println!("{}", headers.join("\t"));
                for row in &rows {
                    let display: Vec<String> = row
                        .iter()
                        .map(|cell| cell.clone().unwrap_or_default())
                        .collect();
                    println!("{}", display.join("\t"));
                }
            }
        }
        session.close().await?;
        Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
    })?;

    Ok(())
}

/// Number of rows materialized by an outcome (0 for a bare row-count DML reply).
fn rows_in(outcome: &ExasolOutcome) -> usize {
    match outcome {
        ExasolOutcome::RowCount(_) => 0,
        ExasolOutcome::ArrowRows(batches) => batches.iter().map(|b| b.num_rows()).sum(),
        ExasolOutcome::TypedRows { rows, .. } => rows.len(),
    }
}

/// Connect once, then run `sql` `repeat` times, fully consuming each result and
/// reporting timing. The first iteration is reported but excluded from the
/// summary as warm-up.
async fn run_benchmark(
    config: &ExasolConfig,
    user: &str,
    password: &str,
    sql: &str,
    repeat: usize,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let connect_start = Instant::now();
    let mut session = ExasolSession::connect(config, user, password).await?;
    let connect_ms = connect_start.elapsed().as_secs_f64() * 1000.0;

    let mut samples_ms: Vec<f64> = Vec::with_capacity(repeat);
    let mut rows = 0;
    for _ in 0..repeat {
        let start = Instant::now();
        let outcome = session.execute(sql).await?;
        rows = rows_in(&outcome);
        // Drop the outcome before stopping the clock so columnar decode /
        // row materialization is included in the measured cost.
        drop(outcome);
        samples_ms.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    session.close().await?;

    // Exclude the first run (connection warm-up, server-side plan cache) from
    // the summary when we have more than one sample.
    let measured = if samples_ms.len() > 1 {
        &samples_ms[1..]
    } else {
        &samples_ms[..]
    };
    let mut sorted = measured.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let min = sorted.first().copied().unwrap_or(0.0);
    let max = sorted.last().copied().unwrap_or(0.0);
    let median = sorted[sorted.len() / 2];
    let mean = sorted.iter().sum::<f64>() / sorted.len() as f64;

    println!("transport={}", config.transport);
    println!("rows={rows}");
    println!("connect_ms={connect_ms:.1}");
    println!("iterations={repeat} (first excluded as warm-up)");
    println!("first_ms={:.2}", samples_ms[0]);
    println!("min_ms={min:.2}  median_ms={median:.2}  mean_ms={mean:.2}  max_ms={max:.2}");
    Ok(())
}

fn required_value(
    args: &mut impl Iterator<Item = String>,
    flag: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    args.next()
        .ok_or_else(|| format!("missing value for {flag}").into())
}

fn print_help() {
    eprintln!(
        "Usage: cargo run --bin exasol_exec -- --dsn <host:port> --user <user> --password <pwd> [--schema <schema>] [--transport websocket|arrow] [--repeat <n>] [--no-verify] [--fingerprint <sha256>] (--sql <sql> | --file <path>)\n\nWith --repeat <n> > 1, runs in benchmark mode: connects once, executes the query n times, fully consumes each result, and prints timing stats (first run excluded as warm-up)."
    );
}
