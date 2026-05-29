//! Minimal reproducer for exarrow-rs 0.12.2 parameter-binding bug.
//!
//! `Statement::build_sql` scans the SQL for `?` characters and substitutes
//! them with bound parameters, but the scan is not aware of string literals,
//! comments, or identifiers. Any literal `?` inside a SQL string causes
//! `QueryError::ParameterBindingError { message: "Not enough parameters
//! bound" }` even when no placeholders are intended.
//!
//! Run with:
//!     DSN=host:port USER=sys PASSWORD=exasol \
//!         cargo run --example question_mark_repro
//!
//! Compares three queries:
//!   1. SELECT 'no question mark here'           -- expected to succeed
//!   2. SELECT 'this has a ?'                    -- triggers the bug
//!   3. SELECT 1 WHERE 'a' LIKE 'a?' ESCAPE '\\' -- triggers the bug (comment path)

use std::env;

use exarrow_rs::Connection;
use exarrow_rs::connection::ConnectionBuilder;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let dsn = env::var("DSN").unwrap_or_else(|_| "127.0.0.1:9564".to_owned());
    let user = env::var("USER").unwrap_or_else(|_| "sys".to_owned());
    let password = env::var("PASSWORD").unwrap_or_else(|_| "exasol".to_owned());

    let (host, port) = dsn
        .rsplit_once(':')
        .map(|(h, p)| (h.to_owned(), p.parse::<u16>().expect("port number")))
        .expect("DSN must be host:port");

    let params = ConnectionBuilder::new()
        .host(&host)
        .port(port)
        .username(&user)
        .password(&password)
        .use_tls(true)
        .validate_server_certificate(false)
        .build()
        .expect("build ConnectionParams");

    println!("--- exarrow-rs 0.12.2: ? in SQL literal reproducer ---");
    println!("Connecting to {host}:{port} as {user} ...");
    let mut conn = Connection::from_params(params)
        .await
        .expect("Connection::from_params");
    println!("Connected.\n");

    run_case(
        &mut conn,
        "1. control (no question mark)",
        "SELECT 'no question mark here'",
    )
    .await;
    run_case(
        &mut conn,
        "2. literal ? inside string",
        "SELECT 'this has a ?'",
    )
    .await;
    run_case(
        &mut conn,
        "3. literal ? inside LIKE pattern",
        "SELECT 1 FROM DUAL WHERE 'a?b' LIKE '%?%'",
    )
    .await;

    let _ = conn.close().await;
}

async fn run_case(conn: &mut Connection, label: &str, sql: &str) {
    println!("[{label}]");
    println!("  SQL: {sql}");
    match conn.execute(sql).await {
        Ok(rs) => {
            if let Some(rows) = rs.row_count() {
                println!("  OK  row_count={rows}\n");
            } else {
                match rs.fetch_all().await {
                    Ok(batches) => {
                        let total: usize = batches.iter().map(|b| b.num_rows()).sum();
                        println!("  OK  {total} row(s) returned");
                        if let Some(batch) = batches.first()
                            && batch.num_rows() > 0
                        {
                            let value =
                                arrow::util::display::array_value_to_string(batch.column(0), 0)
                                    .unwrap_or_else(|_| "<format error>".to_owned());
                            println!("  first cell: {value}");
                        }
                        println!();
                    }
                    Err(err) => println!("  ERR fetch_all: {err:?}\n"),
                }
            }
        }
        Err(err) => {
            println!("  ERR {err:?}");
            println!("  Display: {err}\n");
        }
    }
}
